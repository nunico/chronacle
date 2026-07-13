//! Non-blocking outbound queue and the write-loop guard.
//!
//! `enqueue` is a latency optimisation, never a correctness mechanism: a dropped
//! enqueue degrades to "the file updates on next reconcile". That is why the
//! producers depend on this one-method trait and nothing else vault-shaped.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chronacle_core::{VaultOutbound, VaultRef};

/// Producer handle. Fire-and-forget: a dropped receiver is not an error.
pub struct QueueOutbound {
    tx: tokio::sync::mpsc::UnboundedSender<VaultRef>,
}

impl QueueOutbound {
    /// Create the producer and its receiver.
    pub fn new() -> (Self, tokio::sync::mpsc::UnboundedReceiver<VaultRef>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

impl VaultOutbound for QueueOutbound {
    fn enqueue(&self, target: VaultRef) {
        // A closed channel means vault sync was turned off. Reconcile will
        // catch up if it is turned back on; never panic a producer for it.
        let _ = self.tx.send(target);
    }
}

/// Content-hash keyed loop guard with a TTL.
///
/// Deletes cannot be content-hash keyed — there is no content left to hash
/// once the file is gone — so they get a second, key-only guard (`deletes`)
/// with the same TTL semantics. Every compiler-driven delete in `reconcile()`
/// arms it before deleting; the watcher drops a `Remove` event that matches,
/// exactly as it drops an `Upsert` whose content matches `inner`. Without
/// this, the watcher would misread Chronacle's own orphan-sweep or
/// evaporated-conflict cleanup as the GM's sidecar-deletion resolution
/// signal.
///
/// Asymmetry with the write guard, deliberately: `matches` (write) is
/// content-keyed and non-consuming, because a stale write guard can never
/// mask a real later edit — a genuine GM edit produces different content, so
/// `matches` returns `false` for it even while the guard is still armed. The
/// delete guard has no content dimension to fall back on, so a stale,
/// non-consuming delete guard COULD mask a real later GM deletion (see
/// `take_delete`). The fix is to make the delete guard one-shot: consumed by
/// its first match. Masking a genuine GM resolution is far worse than the
/// alternative (a surplus `Remove` event slipping through and triggering one
/// harmless extra `reconcile()` — reconcile re-derives everything from real
/// on-disk and DB state, so a redundant pass just finds nothing to do).
#[derive(Default)]
pub struct PendingWrites {
    inner: Mutex<HashMap<String, (u64, Instant)>>,
    deletes: Mutex<HashMap<String, Instant>>,
}

impl PendingWrites {
    /// A guard whose event never arrives expires after this long.
    pub const TTL: Duration = Duration::from_secs(30);

    /// Arm a guard for a key we are about to write.
    pub fn arm(&self, key: &str, hash: u64) {
        self.arm_at(key, hash, Instant::now());
    }

    /// Arm with an explicit timestamp. Test seam for TTL expiry.
    pub fn arm_at(&self, key: &str, hash: u64, at: Instant) {
        self.inner
            .lock()
            .expect("poisoned")
            .insert(key.to_owned(), (hash, at));
    }

    /// Whether an inbound event on `key` with this content is our own write.
    ///
    /// Deliberately does **not** consume the guard: one `write()` emits several
    /// events. Content-keyed, so a stale guard cannot mask a real later edit.
    pub fn matches(&self, key: &str, hash: u64) -> bool {
        let guard = self.inner.lock().expect("poisoned");
        guard
            .get(key)
            .is_some_and(|(h, at)| *h == hash && at.elapsed() < Self::TTL)
    }

    /// Arm a guard for a key we are about to delete ourselves (compiler-driven:
    /// orphan sweep, evaporated-conflict sidecar cleanup). Key-only — a delete
    /// has no content to hash.
    pub fn arm_delete(&self, key: &str) {
        self.arm_delete_at(key, Instant::now());
    }

    /// Arm a delete guard with an explicit timestamp. Test seam for TTL expiry.
    pub fn arm_delete_at(&self, key: &str, at: Instant) {
        self.deletes
            .lock()
            .expect("poisoned")
            .insert(key.to_owned(), at);
    }

    /// Whether a `Remove` event on `key` is a delete we made ourselves.
    ///
    /// CONSUMING, unlike `matches`: removes the guard entry on a match so it
    /// suppresses exactly one `Remove` event. A single compiler delete can
    /// still surface more than one fs event, but that is the one case where
    /// letting a surplus event through is the safe failure mode — a masked
    /// GM deletion would leave a conflict frozen forever with the GM
    /// believing they resolved it; a redundant `reconcile()` costs a little
    /// work and changes nothing. See the type-level doc comment above for
    /// the full write-vs-delete-guard reasoning.
    pub fn take_delete(&self, key: &str) -> bool {
        let mut guard = self.deletes.lock().expect("poisoned");
        match guard.get(key) {
            Some(at) if at.elapsed() < Self::TTL => {
                guard.remove(key);
                true
            }
            Some(_) => false, // expired; leave it for `sweep` to reap
            None => false,
        }
    }

    /// Drop expired guards, both write and delete.
    pub fn sweep(&self) {
        self.inner
            .lock()
            .expect("poisoned")
            .retain(|_, (_, at)| at.elapsed() < Self::TTL);
        self.deletes
            .lock()
            .expect("poisoned")
            .retain(|_, at| at.elapsed() < Self::TTL);
    }

    /// Count of currently-armed write guards, expired or not. Test seam —
    /// proves a caller actually swept (vs. relying on `matches`, which
    /// already ignores expired entries and so cannot distinguish "swept"
    /// from "not swept").
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner.lock().expect("poisoned").len()
    }

    /// Count of currently-armed delete guards, expired or not. Test seam,
    /// mirroring `len()`.
    #[cfg(test)]
    pub(crate) fn deletes_len(&self) -> usize {
        self.deletes.lock().expect("poisoned").len()
    }
}

/// Drain the queue, coalescing repeats, calling `export` once per distinct ref.
pub async fn drain_loop_with<F>(mut rx: tokio::sync::mpsc::UnboundedReceiver<VaultRef>, export: F)
where
    F: Fn(VaultRef) -> Result<(), crate::VaultError> + Send + 'static,
{
    while let Some(first) = rx.recv().await {
        let mut batch = HashSet::new();
        batch.insert(first);
        while let Ok(next) = rx.try_recv() {
            batch.insert(next);
        }
        for vref in batch {
            if let Err(e) = export(vref.clone()) {
                // Reconcile is the correctness guarantee; a failed export is a
                // latency problem, not a data problem. Never abort the loop.
                eprintln!("vault: export of {} failed: {e}", vref.to_thing());
            }
        }
    }
}

/// The real drain loop: batches enqueued refs, arms the write guard, and asks
/// `svc` to export the batch in one index scan.
///
/// Index-aware via [`crate::reconcile::VaultSyncService::export_refs`] — a
/// file the GM renamed in the vault keeps its name; the loop never overwrites
/// the wrong key for a renamed record.
pub async fn drain_loop(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<VaultRef>,
    svc: Arc<crate::reconcile::VaultSyncService>,
) {
    while let Some(first) = rx.recv().await {
        let mut batch = HashSet::new();
        batch.insert(first);
        while let Ok(next) = rx.try_recv() {
            batch.insert(next);
        }
        // `export_refs` already logs and continues on a per-ref failure; its
        // only `Err` is a whole-batch `VaultIndex::scan` I/O failure. Log that
        // rather than swallowing it — the batch silently exported nothing.
        if let Err(e) = svc.export_refs(&batch).await {
            eprintln!("vault: drain batch failed to scan the vault index: {e}");
        }
        svc.sweep_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{VaultOutbound, VaultRef};

    fn vref(id: &str) -> VaultRef {
        VaultRef {
            table: "npc".into(),
            id: id.into(),
        }
    }

    #[tokio::test]
    async fn enqueue_delivers_the_ref_to_the_receiver() {
        let (out, mut rx) = QueueOutbound::new();
        out.enqueue(vref("a"));
        assert_eq!(rx.recv().await, Some(vref("a")));
    }

    #[tokio::test]
    async fn enqueue_never_blocks_and_never_panics_after_the_receiver_drops() {
        let (out, rx) = QueueOutbound::new();
        drop(rx);
        out.enqueue(vref("a")); // fire-and-forget: a dropped receiver is not an error
    }

    #[test]
    fn a_guard_matches_the_same_key_and_content() {
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        assert!(p.matches("k.md", 42));
    }

    #[test]
    fn a_guard_does_not_match_different_content_on_the_same_key() {
        // A genuine GM edit after our write must NOT be masked.
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        assert!(!p.matches("k.md", 99));
    }

    #[test]
    fn a_guard_survives_repeated_matches() {
        // One write emits several events (Create + Modify, Data + Metadata).
        // Consuming on first match would let the trailing events through.
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        assert!(p.matches("k.md", 42));
        assert!(p.matches("k.md", 42));
        assert!(p.matches("k.md", 42));
    }

    #[test]
    fn arming_the_same_key_twice_replaces_the_hash() {
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        p.arm("k.md", 43);
        assert!(!p.matches("k.md", 42));
        assert!(p.matches("k.md", 43));
    }

    #[test]
    fn sweep_expires_guards_older_than_the_ttl() {
        let p = PendingWrites::default();
        p.arm_at(
            "k.md",
            42,
            std::time::Instant::now() - PendingWrites::TTL - std::time::Duration::from_secs(1),
        );
        p.sweep();
        assert!(
            !p.matches("k.md", 42),
            "an event that never arrived must not pin a guard forever"
        );
    }

    #[test]
    fn a_delete_guard_matches_the_same_key() {
        let p = PendingWrites::default();
        p.arm_delete("sidecar.md");
        assert!(p.take_delete("sidecar.md"));
    }

    #[test]
    fn a_delete_guard_does_not_match_a_different_key() {
        let p = PendingWrites::default();
        p.arm_delete("sidecar.md");
        assert!(!p.take_delete("other.md"));
    }

    #[test]
    fn a_delete_guard_is_consumed_by_its_first_match() {
        // Unlike the write guard, the delete guard is one-shot: it suppresses
        // exactly one `Remove` event, never more.
        let p = PendingWrites::default();
        p.arm_delete("sidecar.md");
        assert!(p.take_delete("sidecar.md"));
        assert!(!p.take_delete("sidecar.md"));
    }

    /// THE MASKING BUG (Finding 1): a non-consuming delete guard would let a
    /// stale arm from an earlier compiler-driven delete suppress a LATER,
    /// genuine GM deletion of the same key (the record re-entered conflict
    /// and Chronacle rewrote the sidecar in between). Consuming the guard on
    /// its first match means a second, independent `Remove` at the same key
    /// is never suppressed by a guard that already did its job.
    #[test]
    fn a_second_delete_at_the_same_key_is_not_masked_by_an_earlier_consumed_guard() {
        let p = PendingWrites::default();

        // t=0: reconcile deletes sidecar S (our own cleanup).
        p.arm_delete("sidecar.md");
        assert!(
            p.take_delete("sidecar.md"),
            "our own delete's Remove event is suppressed"
        );

        // t=5s: the record re-enters conflict; reconcile writes S again
        // (not modelled here — no delete guard is armed for this write).
        // t=10s: the GM deletes S themselves. No delete guard is armed for
        // this deletion, since the only prior arm was already consumed.
        assert!(
            !p.take_delete("sidecar.md"),
            "a second, independent Remove at the same key must NOT be masked \
             by an earlier guard that already suppressed one event"
        );
    }

    #[test]
    fn sweep_expires_delete_guards_older_than_the_ttl() {
        let p = PendingWrites::default();
        p.arm_delete_at(
            "sidecar.md",
            std::time::Instant::now() - PendingWrites::TTL - std::time::Duration::from_secs(1),
        );
        p.sweep();
        assert!(
            !p.take_delete("sidecar.md"),
            "a delete event that never arrived must not pin a guard forever"
        );
    }

    #[test]
    fn write_and_delete_guards_are_independent() {
        // A key can be both armed for a prior write and, separately, deleted.
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        assert!(
            !p.take_delete("k.md"),
            "arming a write must not arm a delete"
        );
        p.arm_delete("k.md");
        assert!(
            p.matches("k.md", 42),
            "arming a delete must not disarm a write"
        );
    }

    #[tokio::test]
    async fn drain_coalesces_repeat_enqueues_of_the_same_ref() {
        // Compiling 200 entities enqueues 200 refs; the drain writes each once.
        let (out, rx) = QueueOutbound::new();
        for _ in 0..5 {
            out.enqueue(vref("a"));
        }
        drop(out);

        let exported = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let e = std::sync::Arc::clone(&exported);
        drain_loop_with(rx, move |_vref| {
            e.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await;

        assert_eq!(
            exported.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "5 enqueues, 1 write"
        );
    }

    #[tokio::test]
    async fn drain_continues_after_an_export_failure() {
        let (out, rx) = QueueOutbound::new();
        out.enqueue(vref("bad"));
        out.enqueue(vref("good"));
        drop(out);

        let seen = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let s = std::sync::Arc::clone(&seen);
        drain_loop_with(rx, move |v: VaultRef| {
            s.lock().unwrap().push(v.id.clone());
            if v.id == "bad" {
                Err(crate::VaultError::Frontmatter("boom".into()))
            } else {
                Ok(())
            }
        })
        .await;

        assert_eq!(
            seen.lock().unwrap().len(),
            2,
            "one failing ref must not stop the drain"
        );
    }
}
