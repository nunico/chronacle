use async_trait::async_trait;

/// Errors that can arise from blob-store operations.
#[derive(Debug, thiserror::Error)]
pub enum BlobStoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

/// Trait abstracting file storage for PDFs and other binary assets.
///
/// Files are organised under a root directory with sub-directories per
/// source ID.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Persist a blob (e.g. a PDF) to the store.
    async fn store(
        &self,
        source_id: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<(), BlobStoreError>;

    /// Read a previously stored blob back into memory.
    async fn retrieve(&self, source_id: &str, filename: &str) -> Result<Vec<u8>, BlobStoreError>;

    /// Remove all blobs associated with `source_id`.
    async fn delete(&self, source_id: &str) -> Result<(), BlobStoreError>;
}

/// Filesystem-backed blob store.
///
/// Files are stored at `{root}/{source_id}/{filename}`.
pub struct LocalFileStore {
    root: std::path::PathBuf,
}

impl LocalFileStore {
    /// Create a new store rooted at `root`.
    ///
    /// The directory is created on first `store` call if it does not exist.
    pub fn new(root: std::path::PathBuf) -> Self {
        Self { root }
    }
}

#[async_trait]
impl BlobStore for LocalFileStore {
    async fn store(
        &self,
        source_id: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<(), BlobStoreError> {
        let dir = self.root.join(source_id);
        tokio::fs::create_dir_all(&dir).await?;

        let path = dir.join(filename);
        tokio::fs::write(&path, data).await?;

        Ok(())
    }

    async fn retrieve(&self, source_id: &str, filename: &str) -> Result<Vec<u8>, BlobStoreError> {
        let path = self.root.join(source_id).join(filename);

        if !path.exists() {
            return Err(BlobStoreError::NotFound(format!(
                "{} / {}",
                source_id, filename
            )));
        }

        let data = tokio::fs::read(&path).await?;
        Ok(data)
    }

    async fn delete(&self, source_id: &str) -> Result<(), BlobStoreError> {
        let dir = self.root.join(source_id);

        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFileStore::new(dir.path().to_path_buf());

        store
            .store("src-1", "test.pdf", b"hello world")
            .await
            .unwrap();

        let data = store.retrieve("src-1", "test.pdf").await.unwrap();
        assert_eq!(data, b"hello world");
    }

    #[tokio::test]
    async fn test_delete_removes_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFileStore::new(dir.path().to_path_buf());

        store.store("src-2", "doc.pdf", b"data").await.unwrap();
        store.delete("src-2").await.unwrap();

        let result = store.retrieve("src-2", "doc.pdf").await;
        assert!(matches!(result, Err(BlobStoreError::NotFound(_))));
    }
}
