//! Filesystem-backed `VaultStore`.
//!
//! The only component in the vault stack that knows about `tokio::fs`. Keys are
//! POSIX-style and root-relative; this adapter is the sole place they become OS
//! paths. There is deliberately no `rename()` — S3 has none, and a re-key is
//! `write(new) + delete(old)`.

use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use chronacle_core::{VaultKey, VaultMetadata, VaultStore, VaultStoreError};

/// A `VaultStore` rooted at a directory on the local filesystem.
pub struct LocalFsVaultStore {
    root: PathBuf,
}

/// Wrap a `std::io::Error`, preserving its `kind()` for callers that need to
/// distinguish e.g. permission errors from a full disk.
fn io_err(e: std::io::Error) -> VaultStoreError {
    VaultStoreError::Io {
        kind: e.kind(),
        message: e.to_string(),
    }
}

impl LocalFsVaultStore {
    /// Create a store rooted at `root`. The directory need not exist yet.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve a key to an absolute path, refusing anything that escapes the root.
    fn resolve(&self, key: &str) -> Result<PathBuf, VaultStoreError> {
        let rel = Path::new(key);
        // `RootDir`/`Prefix` are checked explicitly, not just via `is_absolute()`:
        // on Windows a rooted-but-driveless path like `/etc/passwd` is NOT
        // `is_absolute()`, yet `root.join("/etc/passwd")` still discards the root
        // (→ `C:\etc\passwd`), escaping the vault. Rejecting `RootDir` closes that.
        if rel.is_absolute()
            || rel.components().any(|c| {
                matches!(
                    c,
                    Component::RootDir | Component::ParentDir | Component::Prefix(_)
                )
            })
        {
            return Err(VaultStoreError::InvalidKey(key.to_owned()));
        }
        Ok(self.root.join(rel))
    }
}

#[async_trait]
impl VaultStore for LocalFsVaultStore {
    async fn read(&self, key: &str) -> Result<String, VaultStoreError> {
        let path = self.resolve(key)?;
        match tokio::fs::read_to_string(&path).await {
            Ok(content) => Ok(content),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(VaultStoreError::NotFound(key.to_owned()))
            }
            Err(e) => Err(io_err(e)),
        }
    }

    async fn write(&self, key: &str, content: &str) -> Result<(), VaultStoreError> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(io_err)?;
        }
        tokio::fs::write(&path, content).await.map_err(io_err)
    }

    async fn delete(&self, key: &str) -> Result<(), VaultStoreError> {
        let path = self.resolve(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            // Reconcile may re-delete a key it already removed; that must be a no-op.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err(e)),
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<VaultKey>, VaultStoreError> {
        let start = self.resolve(prefix)?;

        // Explicit stack, not recursion via `path.is_dir()`: `Path::is_dir()`
        // follows symlinks, so a symlinked directory pointing at an ancestor
        // would recurse forever. `DirEntry::file_type()` does not follow
        // symlinks, so a symlinked directory is naturally skipped.
        let mut stack = vec![start];
        let mut keys = Vec::new();

        while let Some(dir) = stack.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io_err(e)),
            };

            while let Some(entry) = entries.next_entry().await.map_err(io_err)? {
                let ft = entry.file_type().await.map_err(io_err)?;
                let path = entry.path();

                if ft.is_dir() {
                    stack.push(path);
                } else if ft.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                    let rel = path
                        .strip_prefix(&self.root)
                        .map_err(|_| VaultStoreError::Io {
                            kind: std::io::ErrorKind::Other,
                            message: format!(
                                "path {} escaped vault root during list",
                                path.display()
                            ),
                        })?;
                    let key = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("/");
                    keys.push(key);
                }
            }
        }

        Ok(keys)
    }

    async fn metadata(&self, key: &str) -> Result<VaultMetadata, VaultStoreError> {
        let path = self.resolve(key)?;
        let meta = match tokio::fs::metadata(&path).await {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(VaultStoreError::NotFound(key.to_owned()));
            }
            Err(e) => return Err(io_err(e)),
        };
        let mtime = meta.modified().map_err(io_err)?;
        Ok(VaultMetadata { mtime })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{VaultStore, VaultStoreError};
    use tempfile::TempDir;

    fn store() -> (TempDir, LocalFsVaultStore) {
        let dir = TempDir::new().expect("tempdir");
        let store = LocalFsVaultStore::new(dir.path());
        (dir, store)
    }

    #[tokio::test]
    async fn write_then_read_round_trips() {
        let (_dir, store) = store();
        store
            .write("campaigns/c/entities/npc/a.md", "hello")
            .await
            .expect("write");
        assert_eq!(
            store
                .read("campaigns/c/entities/npc/a.md")
                .await
                .expect("read"),
            "hello"
        );
    }

    #[tokio::test]
    async fn write_creates_missing_parent_directories() {
        let (_dir, store) = store();
        // Nothing pre-creates `campaigns/c/entities/npc/`.
        store
            .write("campaigns/c/entities/npc/deep.md", "x")
            .await
            .expect("write must mkdir -p");
        assert_eq!(
            store
                .read("campaigns/c/entities/npc/deep.md")
                .await
                .expect("read"),
            "x"
        );
    }

    #[tokio::test]
    async fn read_of_a_missing_key_is_not_found() {
        let (_dir, store) = store();
        assert!(matches!(
            store.read("nope.md").await,
            Err(VaultStoreError::NotFound(_))
        ));
    }

    #[tokio::test]
    async fn delete_removes_the_file_and_is_idempotent() {
        let (_dir, store) = store();
        store.write("a.md", "x").await.expect("write");
        store.delete("a.md").await.expect("delete");
        assert!(matches!(
            store.read("a.md").await,
            Err(VaultStoreError::NotFound(_))
        ));
        store
            .delete("a.md")
            .await
            .expect("deleting an absent key must succeed");
    }

    #[tokio::test]
    async fn list_returns_posix_keys_recursively_and_only_md() {
        let (_dir, store) = store();
        store
            .write("campaigns/c/entities/npc/a.md", "x")
            .await
            .expect("write");
        store
            .write("campaigns/c/sessions/001-b.md", "x")
            .await
            .expect("write");
        store
            .write(".obsidian/workspace.json", "{}")
            .await
            .expect("write");

        let mut keys = store.list("").await.expect("list");
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "campaigns/c/entities/npc/a.md".to_string(),
                "campaigns/c/sessions/001-b.md".to_string(),
            ],
            "only .md files, POSIX separators, no OS paths"
        );
    }

    #[tokio::test]
    async fn list_honours_the_prefix() {
        let (_dir, store) = store();
        store
            .write("campaigns/c/entities/npc/a.md", "x")
            .await
            .expect("write");
        store
            .write("collections/k/rules/g.md", "x")
            .await
            .expect("write");
        let keys = store.list("collections").await.expect("list");
        assert_eq!(keys, vec!["collections/k/rules/g.md".to_string()]);
    }

    #[tokio::test]
    async fn list_of_a_missing_prefix_is_empty_not_an_error() {
        let (_dir, store) = store();
        assert_eq!(
            store.list("campaigns").await.expect("list"),
            Vec::<String>::new()
        );
    }

    #[tokio::test]
    async fn metadata_returns_a_monotonic_mtime() {
        let (_dir, store) = store();
        store.write("a.md", "x").await.expect("write");
        let m1 = store.metadata("a.md").await.expect("metadata");
        store.write("a.md", "y").await.expect("rewrite");
        let m2 = store.metadata("a.md").await.expect("metadata");
        assert!(m2.mtime >= m1.mtime);
    }

    #[tokio::test]
    async fn a_key_escaping_the_root_is_rejected() {
        let (_dir, store) = store();
        assert!(matches!(
            store.write("../escape.md", "x").await,
            Err(VaultStoreError::InvalidKey(_))
        ));
        assert!(matches!(
            store.read("campaigns/../../etc/passwd").await,
            Err(VaultStoreError::InvalidKey(_))
        ));
        // A rooted key must be rejected on EVERY platform. On Unix `is_absolute()`
        // catches it; on Windows a driveless `/…` is not `is_absolute()`, so the
        // explicit `Component::RootDir` guard is what stops `root.join()` from
        // escaping the vault. Pin the invariant, not the Unix-incidental path.
        assert!(matches!(
            store.read("/etc/passwd").await,
            Err(VaultStoreError::InvalidKey(_))
        ));
    }
}
