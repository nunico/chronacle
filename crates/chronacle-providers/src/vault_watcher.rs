//! `VaultWatcher` over the `notify` crate (ADR-008 / tranche 5).
//!
//! Dumb by design: maps fs events to vault keys, debounces bursts, and emits
//! `VaultEvent`s. It does NOT decide anything — self-write filtering happens in
//! the consumer via `VaultSyncService::is_own_write`/`is_own_delete`, and every
//! materialization happens in `reconcile()`. A dropped event degrades to
//! "handled on the next reconcile", never to wrong data.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

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
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<VaultEvent>();
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

        let native_event_tx = event_tx.clone();
        let native_root = root.clone();
        tokio::spawn(async move {
            // The watcher must stay alive for the task's lifetime; notify's
            // callback runs on its own thread, and unbounded_send is sync-safe.
            let mut watcher = match notify::recommended_watcher(move |res| {
                let _ = raw_tx.send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("vault: watcher init failed: {e}");
                    let _ = native_event_tx.send(VaultEvent::Rescan);
                    return;
                }
            };
            if let Err(e) = watcher.watch(&native_root, RecursiveMode::Recursive) {
                eprintln!("vault: watch of {} failed: {e}", native_root.display());
                let _ = native_event_tx.send(VaultEvent::Rescan);
                return;
            }

            let mut pending: Vec<VaultEvent> = Vec::new();
            loop {
                // Wait for the first event of a burst…
                let Some(first) = raw_rx.recv().await else {
                    break;
                };
                collect(&native_root, first, &mut pending);
                // …then absorb the burst until a quiet window elapses.
                loop {
                    match tokio::time::timeout(debounce, raw_rx.recv()).await {
                        Ok(Some(ev)) => collect(&native_root, ev, &mut pending),
                        Ok(None) => return, // channel closed
                        Err(_elapsed) => break,
                    }
                }
                pending.sort_unstable_by(event_order);
                pending.dedup();
                for ev in pending.drain(..) {
                    if native_event_tx.send(ev).is_err() {
                        return; // consumer dropped; stop watching
                    }
                }
            }
        });

        let poll_event_tx = event_tx.clone();
        let poll_root = root.clone();
        tokio::spawn(async move {
            let poll_interval = (debounce / 2).max(Duration::from_millis(50));
            let mut known = scan_markdown_files(&poll_root);
            loop {
                tokio::time::sleep(poll_interval).await;
                let current = scan_markdown_files(&poll_root);

                let mut events = Vec::new();
                for (key, meta) in &current {
                    if known.get(key) != Some(meta) {
                        events.push(VaultEvent::Upsert(key.clone()));
                    }
                }
                let current_keys: HashSet<&String> = current.keys().collect();
                for key in known.keys() {
                    if !current_keys.contains(key) {
                        events.push(VaultEvent::Remove(key.clone()));
                    }
                }

                known = current;
                events.sort_unstable_by(event_order);
                events.dedup();
                for ev in events {
                    if poll_event_tx.send(ev).is_err() {
                        return;
                    }
                }
            }
        });

        tokio::spawn(async move {
            let mut pending: Vec<VaultEvent> = Vec::new();
            while let Some(first) = event_rx.recv().await {
                pending.push(first);
                loop {
                    match tokio::time::timeout(debounce, event_rx.recv()).await {
                        Ok(Some(ev)) => pending.push(ev),
                        Ok(None) => return,
                        Err(_elapsed) => break,
                    }
                }
                pending.sort_unstable_by(event_order);
                pending.dedup();
                for ev in pending.drain(..) {
                    if tx.send(ev).await.is_err() {
                        return;
                    }
                }
            }
        });
        rx
    }
}

fn scan_markdown_files(root: &Path) -> HashMap<String, (SystemTime, u64)> {
    fn visit(root: &Path, dir: &Path, out: &mut HashMap<String, (SystemTime, u64)>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if metadata.is_dir() {
                visit(root, &path, out);
            } else if metadata.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                let Some(key) = NotifyWatcher::key_of(root, &path) else {
                    continue;
                };
                let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                out.insert(key, (mtime, metadata.len()));
            }
        }
    }

    let mut out = HashMap::new();
    visit(root, root, &mut out);
    out
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
        // A NEW DIRECTORY is the one event we must not drop for want of a `.md`
        // extension. Linux `inotify` watches one directory at a time: `notify`
        // adds a watch for a newly-created subdirectory only after it sees the
        // creation, so a file written into it inside that window produces NO
        // event at all — a GM who drags a whole folder of notes into the vault
        // would have it silently ignored until something else triggered a sync.
        // (macOS FSEvents watches the tree and does not have this hole, which is
        // exactly why this was invisible in local testing and only failed on CI.)
        //
        // Fall back to a Rescan: reconcile re-derives everything from disk, so it
        // finds whatever the missed events would have told us about. Restricted
        // to directories on purpose — emitting Rescan for any non-`.md` write
        // would have Obsidian's own `.obsidian/workspace.json` churn triggering a
        // reconcile storm.
        if matches!(event.kind, EventKind::Create(_)) && path.is_dir() {
            out.push(VaultEvent::Rescan);
            continue;
        }
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

    /// The key mapping itself: an edit to a file in a directory the watcher is
    /// already watching must arrive as an `Upsert` carrying a POSIX-style key.
    ///
    /// The directory is created BEFORE `subscribe()` on purpose. Creating it
    /// after would race Linux's per-directory `inotify` registration, which is a
    /// real hole — but it is a *different* hole, covered by the test below. This
    /// test is about the key, so it must not be able to fail for that reason.
    #[tokio::test]
    async fn an_edited_md_file_produces_an_upsert_with_a_posix_key() {
        let dir = tempfile::TempDir::new().unwrap();
        let sub = dir.path().join("campaigns/c/entities/npc");
        std::fs::create_dir_all(&sub).unwrap();

        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(100));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await; // watcher warm-up

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

    /// A GM dragging a whole FOLDER of notes into the vault must not be ignored.
    ///
    /// Linux `inotify` watches one directory at a time, and `notify` only adds a
    /// watch for a new subdirectory after it observes the creation — so a file
    /// written into it inside that window emits no event of its own. Dropping the
    /// directory event (it has no `.md` extension) therefore lost the whole
    /// folder until some unrelated change triggered a sync. The watcher falls back
    /// to `Rescan`, which is sufficient: reconcile re-derives everything from disk.
    ///
    /// This asserts the guarantee that matters — *some* event arrives, so a
    /// reconcile runs — rather than a specific one, because which event wins the
    /// race is genuinely platform-dependent.
    #[tokio::test]
    async fn a_file_created_in_a_brand_new_directory_still_wakes_the_sync() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(100));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await; // watcher warm-up

        // Created only now — the directory watch does not exist yet on Linux.
        let sub = dir.path().join("campaigns/c/entities/npc");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.md"), "hello").unwrap();

        let ev = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("a new folder must wake the sync, not be silently ignored")
            .expect("open channel");
        assert!(
            matches!(
                ev,
                VaultEvent::Rescan | VaultEvent::Upsert(_) | VaultEvent::Remove(_)
            ),
            "expected an event that triggers a reconcile, got {ev:?}"
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
