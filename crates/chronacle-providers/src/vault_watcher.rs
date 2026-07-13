//! `VaultWatcher` over the `notify` crate (ADR-008 / tranche 5).
//!
//! Dumb by design: maps fs events to vault keys, debounces bursts, and emits
//! `VaultEvent`s. It does NOT decide anything — self-write filtering happens in
//! the consumer via `VaultSyncService::is_own_write`/`is_own_delete`, and every
//! materialization happens in `reconcile()`. A dropped event degrades to
//! "handled on the next reconcile", never to wrong data.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use chronacle_core::{VaultEvent, VaultWatcher};
use notify::{RecursiveMode, Watcher};

/// Filesystem watcher for a local vault root.
pub struct NotifyWatcher {
    root: PathBuf,
    debounce: Duration,
}

impl NotifyWatcher {
    /// Default quiet window between an fs burst and the flush.
    pub const DEBOUNCE: Duration = Duration::from_secs(2);

    /// Watch `root` recursively with the default debounce.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            debounce: Self::DEBOUNCE,
        }
    }

    /// Test seam: a shorter debounce keeps the integration tests fast.
    pub fn with_debounce(root: impl Into<PathBuf>, debounce: Duration) -> Self {
        Self {
            root: root.into(),
            debounce,
        }
    }

    /// Map an OS path inside the vault to a POSIX-style key. Non-`.md` paths
    /// and paths outside the root return `None`.
    fn key_of(root: &Path, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(root).ok()?;
        let key = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        key.ends_with(".md").then_some(key)
    }
}

#[async_trait]
impl VaultWatcher for NotifyWatcher {
    async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<VaultEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let (raw_tx, mut raw_rx) =
            tokio::sync::mpsc::unbounded_channel::<Result<notify::Event, notify::Error>>();
        // Canonicalize: on macOS the tmpdir root under `/var/...` is a symlink
        // to `/private/var/...`, and FSEvents reports paths through the
        // resolved form. Watching/stripping the un-resolved root would make
        // `key_of` silently miss every event. Falls back to the given root
        // (e.g. it does not exist yet) rather than failing the watcher.
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        let debounce = self.debounce;

        tokio::spawn(async move {
            // The watcher must stay alive for the task's lifetime; notify's
            // callback runs on its own thread, and unbounded_send is sync-safe.
            let mut watcher = match notify::recommended_watcher(move |res| {
                let _ = raw_tx.send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("vault: watcher init failed: {e}");
                    let _ = tx.send(VaultEvent::Rescan).await;
                    return;
                }
            };
            if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
                eprintln!("vault: watch of {} failed: {e}", root.display());
                let _ = tx.send(VaultEvent::Rescan).await;
                return;
            }

            let mut pending: Vec<VaultEvent> = Vec::new();
            loop {
                // Wait for the first event of a burst…
                let Some(first) = raw_rx.recv().await else {
                    break;
                };
                collect(&root, first, &mut pending);
                // …then absorb the burst until a quiet window elapses.
                loop {
                    match tokio::time::timeout(debounce, raw_rx.recv()).await {
                        Ok(Some(ev)) => collect(&root, ev, &mut pending),
                        Ok(None) => return, // channel closed
                        Err(_elapsed) => break,
                    }
                }
                pending.sort_unstable_by(event_order);
                pending.dedup();
                for ev in pending.drain(..) {
                    if tx.send(ev).await.is_err() {
                        return; // consumer dropped; stop watching
                    }
                }
            }
        });
        rx
    }
}

/// Fold one raw notify result into the pending batch.
fn collect(root: &Path, res: Result<notify::Event, notify::Error>, out: &mut Vec<VaultEvent>) {
    let event = match res {
        Ok(e) => e,
        Err(e) => {
            eprintln!("vault: watcher error: {e}");
            out.push(VaultEvent::Rescan);
            return;
        }
    };
    use notify::EventKind;
    for path in &event.paths {
        let Some(key) = NotifyWatcher::key_of(root, path) else {
            continue;
        };
        match event.kind {
            EventKind::Remove(_) => out.push(VaultEvent::Remove(key)),
            EventKind::Create(_) | EventKind::Modify(_) => out.push(VaultEvent::Upsert(key)),
            EventKind::Any | EventKind::Other => out.push(VaultEvent::Rescan),
            EventKind::Access(_) => {}
        }
    }
}

/// Stable ordering so `dedup` collapses repeats within a batch.
fn event_order(a: &VaultEvent, b: &VaultEvent) -> std::cmp::Ordering {
    fn rank(e: &VaultEvent) -> (u8, &str) {
        match e {
            VaultEvent::Rescan => (0, ""),
            VaultEvent::Upsert(k) => (1, k),
            VaultEvent::Remove(k) => (2, k),
        }
    }
    rank(a).cmp(&rank(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_created_md_file_produces_an_upsert_with_a_posix_key() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(100));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await; // watcher warm-up

        let sub = dir.path().join("campaigns/c/entities/npc");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.md"), "hello").unwrap();

        let ev = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("event within 5s")
            .expect("open channel");
        assert_eq!(
            ev,
            VaultEvent::Upsert("campaigns/c/entities/npc/a.md".into())
        );
    }

    #[tokio::test]
    async fn a_non_md_file_produces_no_event() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(100));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        std::fs::write(dir.path().join("workspace.json"), "{}").unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(700), rx.recv())
                .await
                .is_err(),
            "no event for non-md files"
        );
    }

    #[tokio::test]
    async fn a_burst_of_writes_coalesces_into_one_batch() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(200));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        for _ in 0..5 {
            std::fs::write(dir.path().join("a.md"), "x").unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let ev = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("event")
            .expect("open");
        assert_eq!(ev, VaultEvent::Upsert("a.md".into()));
        // The dedup collapsed the burst; nothing else arrives promptly.
        assert!(tokio::time::timeout(Duration::from_millis(500), rx.recv())
            .await
            .is_err());
    }

    /// A file created then deleted within the same debounce window still
    /// surfaces its `Remove` — sorted after any `Upsert` for the same key so
    /// `reconcile` (which is index-driven, not event-driven) sees the
    /// terminal state and never resurrects a file the GM deleted.
    #[tokio::test]
    async fn a_deleted_md_file_produces_a_remove_event() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.md"), "x").unwrap();

        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(100));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        std::fs::remove_file(dir.path().join("a.md")).unwrap();

        // Drain events until we see the Remove — a Create/Modify may also
        // fire depending on platform-specific fs event ordering during
        // subscribe warm-up.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            let ev = tokio::time::timeout(remaining, rx.recv())
                .await
                .expect("a remove event within 5s")
                .expect("open channel");
            if ev == VaultEvent::Remove("a.md".into()) {
                break;
            }
        }
    }
}
