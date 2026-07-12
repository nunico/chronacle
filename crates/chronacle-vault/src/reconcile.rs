//! Bidirectional reconcile.
//!
//! Reconcile is the **correctness guarantee**; the outbound queue (D4a) is only
//! a latency optimisation. A dropped `enqueue()` degrades to "the file updates
//! on next reconcile", never to "the file is permanently wrong". That is also
//! why a backend with no change feed (S3, WebDAV) is still correct.
//!
//! `Conflict` materialises as a sidecar file (`<key>.conflict.md`) that
//! carries the DB's render while the record freezes. Deleting the sidecar is
//! the GM's resolution signal.

use std::collections::HashMap;
use std::sync::Arc;

use chronacle_core::{VaultRecord, VaultRecordStore, VaultRef, VaultStore};

use crate::decide::{decide, SyncAction};
use crate::keys::{key_for, VaultIndex};
use crate::render::{content_hash, render_record};
use crate::VaultError;

/// Outcome counts for one reconcile pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    /// Files written.
    pub exported: usize,
    /// Records already in sync.
    pub unchanged: usize,
    /// Stale merge bases adopted (crash recovery).
    pub adopted: usize,
    /// Inbound edits applied to the DB and re-exported canonically.
    pub applied: usize,
    /// Divergent edits seen this pass. Includes both newly-frozen records
    /// and already-frozen records that are still frozen.
    pub conflicts: usize,
    /// Conflicts resolved this pass (the GM deleted the sidecar).
    pub resolved: usize,
    /// Records soft-deleted because their vault file disappeared.
    pub soft_deleted: usize,
    /// Orphaned sync-state rows cleared (record no longer syncs).
    pub swept: usize,
    /// Managed files whose frontmatter could not be parsed. Never applied,
    /// never overwritten.
    pub invalid: usize,
    /// Keys that failed to write. The run continues.
    pub failed: usize,
    /// Refs applied this pass, for callers that need to react (e.g. a
    /// wikilink resync). Populated alongside `applied`.
    pub applied_refs: Vec<VaultRef>,
}

/// The vault sync engine.
pub struct VaultSyncService {
    store: Arc<dyn VaultStore>,
    records: Arc<dyn VaultRecordStore>,
    pending: Arc<crate::outbound::PendingWrites>,
}

/// The stable identity carried by every `VaultRecord` variant.
fn vref_of(record: &VaultRecord) -> &VaultRef {
    match record {
        VaultRecord::Entity(e) => &e.vref,
        VaultRecord::Session(s) => &s.vref,
        VaultRecord::RuleEntry(r) => &r.vref,
    }
}

impl VaultSyncService {
    /// Construct the engine over a storage backend, a record backend, and the
    /// shared write-loop guard (also consulted by the watcher, E6).
    pub fn new(
        store: Arc<dyn VaultStore>,
        records: Arc<dyn VaultRecordStore>,
        pending: Arc<crate::outbound::PendingWrites>,
    ) -> Self {
        Self {
            store,
            records,
            pending,
        }
    }

    /// Expire stale write guards. The drain loop calls this after each batch.
    pub fn sweep_pending(&self) {
        self.pending.sweep();
    }

    /// Wipe every persisted merge base — fresh baseline for a new vault dir.
    pub async fn clear_all_bases(&self) -> Result<(), VaultError> {
        Ok(self.records.clear_all_synced().await?)
    }

    /// Run one bidirectional reconcile pass over every syncable record.
    ///
    /// Loads every record and the on-disk vault index, computes a three-way
    /// sync decision per record, and acts on `Export`/`AdoptBase`/`NoOp`/
    /// `Apply`/`SoftDelete`/`Conflict`. A `Conflict` writes (or refreshes) a
    /// `<key>.conflict.md` sidecar with the DB's render and freezes the
    /// record; deleting the sidecar resolves it in favour of the GM's file.
    /// A trailing orphan sweep clears sync-state rows whose record no longer
    /// syncs, deleting the vault file (and any sidecar) only while it still
    /// matches the last-known base.
    pub async fn reconcile(&self) -> Result<ReconcileReport, VaultError> {
        let index = VaultIndex::scan(self.store.as_ref()).await?;
        let records = self.records.list_all().await?;

        // Un-suffixed keys claimed by more than one record must all be
        // suffixed, so a later shared-name record doesn't clobber an earlier
        // one's file.
        let mut unsuffixed_counts: HashMap<String, usize> = HashMap::new();
        for record in &records {
            *unsuffixed_counts.entry(key_for(record, false)).or_insert(0) += 1;
        }

        let synced: HashMap<VaultRef, chronacle_core::SyncedRow> = self
            .records
            .list_synced()
            .await?
            .into_iter()
            .map(|row| (row.vref.clone(), row))
            .collect();

        let mut report = ReconcileReport::default();

        for record in &records {
            let vref = vref_of(record);
            let collides = unsuffixed_counts
                .get(&key_for(record, false))
                .is_some_and(|&n| n > 1);

            let rendered = render_record(record);
            let db = content_hash(&rendered);

            let existing_key = index.key_of(vref).cloned();
            let key = existing_key
                .clone()
                .unwrap_or_else(|| key_for(record, collides));
            // A file can exist at `key` without being in `by_ref` when its
            // frontmatter is corrupted — the id-based lookup can't find it,
            // but `managed_keys` still saw it on disk. Read it either way so
            // the Apply/invalid path can inspect (and never resurrect) it.
            let file_exists = existing_key.is_some() || index.has_key(&key);
            let file_content = if file_exists {
                // Per-record: an unreadable file must not abort the run, or one
                // bad file blocks every other record from ever syncing. Skipping
                // is safe — a record with no decision is neither written nor
                // deleted, and the next pass retries it.
                match self.store.read(&key).await {
                    Ok(content) => Some(content),
                    Err(e) => {
                        eprintln!("vault: read of {key} failed: {e}");
                        report.failed += 1;
                        continue;
                    }
                }
            } else {
                None
            };
            let file = file_content.as_deref().map(content_hash);
            let state = synced.get(vref);
            let base = state.and_then(|s| s.synced_hash);
            let was_frozen = state.is_some_and(|s| s.conflict);
            let sidecar = crate::keys::sidecar_key(&key);

            let action = decide(base, db, file);

            if was_frozen && action != SyncAction::Conflict {
                // The conflict evaporated on its own (e.g. the GM reverted
                // the file back to what the DB already has). Clean up before
                // handling the action normally.
                match self.store.delete(&sidecar).await {
                    Ok(()) | Err(chronacle_core::VaultStoreError::NotFound(_)) => {}
                    Err(e) => eprintln!("vault: sidecar cleanup of {sidecar} failed: {e}"),
                }
                if let Err(e) = self.records.set_conflict(vref, &key, false).await {
                    eprintln!("vault: unfreeze of {} failed: {e}", vref.to_thing());
                    report.failed += 1;
                    continue;
                }
            }

            match action {
                SyncAction::NoOp => report.unchanged += 1,
                SyncAction::AdoptBase => {
                    if let Err(e) = self.records.set_synced_hash(vref, &key, db).await {
                        eprintln!("vault: adopt-base of {} failed: {e}", vref.to_thing());
                        report.failed += 1;
                        continue;
                    }
                    report.adopted += 1;
                }
                SyncAction::Export => {
                    self.pending.arm(&key, db);
                    match self.store.write(&key, &rendered).await {
                        Ok(()) => {
                            if let Err(e) = self.records.set_synced_hash(vref, &key, db).await {
                                eprintln!(
                                    "vault: set-synced-hash of {} failed after export: {e}",
                                    vref.to_thing()
                                );
                                report.failed += 1;
                                continue;
                            }
                            report.exported += 1;
                        }
                        Err(e) => {
                            eprintln!("vault: failed to write {key}: {e}");
                            report.failed += 1;
                            continue;
                        }
                    }
                }
                SyncAction::Apply => {
                    match self
                        .apply_inbound(vref, &key, file_content.as_deref().unwrap_or(""))
                        .await
                    {
                        Ok(true) => {
                            report.applied += 1;
                            report.applied_refs.push(vref.clone());
                        }
                        Ok(false) => report.invalid += 1,
                        Err(e) => {
                            eprintln!("vault: inbound apply of {key} failed: {e}");
                            report.failed += 1;
                        }
                    }
                }
                SyncAction::Conflict => {
                    let sidecar_content = match self.store.read(&sidecar).await {
                        Ok(c) => Some(c),
                        Err(chronacle_core::VaultStoreError::NotFound(_)) => None,
                        Err(e) => {
                            eprintln!("vault: sidecar read of {sidecar} failed: {e}");
                            report.failed += 1;
                            continue;
                        }
                    };
                    match (was_frozen, sidecar_content) {
                        // Deletion of the sidecar is the GM's resolution signal.
                        (true, None) => {
                            match self
                                .apply_inbound(vref, &key, file_content.as_deref().unwrap_or(""))
                                .await
                            {
                                Ok(true) => {
                                    if let Err(e) =
                                        self.records.set_conflict(vref, &key, false).await
                                    {
                                        eprintln!(
                                            "vault: resolve of {} failed: {e}",
                                            vref.to_thing()
                                        );
                                        report.failed += 1;
                                        continue;
                                    }
                                    report.resolved += 1;
                                    report.applied_refs.push(vref.clone());
                                }
                                Ok(false) => report.invalid += 1,
                                Err(e) => {
                                    eprintln!("vault: conflict resolution of {key} failed: {e}");
                                    report.failed += 1;
                                }
                            }
                        }
                        // Frozen: keep the sidecar current with the DB render.
                        (true, Some(existing)) => {
                            if content_hash(&existing) != db {
                                self.pending.arm(&sidecar, db);
                                if let Err(e) = self.store.write(&sidecar, &rendered).await {
                                    eprintln!("vault: sidecar refresh of {sidecar} failed: {e}");
                                    report.failed += 1;
                                    continue;
                                }
                            }
                            report.conflicts += 1;
                        }
                        // New conflict: preserve the DB version, freeze.
                        (false, _) => {
                            self.pending.arm(&sidecar, db);
                            if let Err(e) = self.store.write(&sidecar, &rendered).await {
                                eprintln!("vault: sidecar write of {sidecar} failed: {e}");
                                report.failed += 1;
                                continue;
                            }
                            if let Err(e) = self.records.set_conflict(vref, &key, true).await {
                                eprintln!("vault: freeze of {} failed: {e}", vref.to_thing());
                                report.failed += 1;
                                continue;
                            }
                            report.conflicts += 1;
                        }
                    }
                }
                // Identity is the frontmatter `id`. A file that loses its
                // frontmatter AND is renamed to a name that matches no
                // record is unattributable here — `key_of` misses (no
                // parsable id) and `has_key` misses (name differs), so this
                // arm fires even though the file is still on disk. That is
                // safe: soft-delete only flips `vault_deleted` (recoverable
                // in the DB) and the orphan sweep's never-clobber rule
                // leaves the file untouched since its hash won't match the
                // base. Recovering it is the deferred "id-less file
                // adoption" feature, not this arm's job.
                SyncAction::SoftDelete => {
                    if let Err(e) = self.records.soft_delete(vref).await {
                        eprintln!("vault: soft-delete of {} failed: {e}", vref.to_thing());
                        report.failed += 1;
                        continue;
                    }
                    if let Err(e) = self.records.clear_synced_hash(vref).await {
                        eprintln!(
                            "vault: clear-synced-hash of {} failed after soft-delete: {e}",
                            vref.to_thing()
                        );
                        report.failed += 1;
                        continue;
                    }
                    report.soft_deleted += 1;
                }
            }
        }

        // Orphan sweep: rows whose record no longer syncs (in-app soft/hard
        // delete). Delete the file only while it still matches the base —
        // never clobber prose the GM kept editing after the record died.
        let record_refs: std::collections::HashSet<&VaultRef> =
            records.iter().map(vref_of).collect();
        for row in synced.values() {
            if record_refs.contains(&row.vref) {
                continue;
            }
            match self.store.read(&row.key).await {
                Ok(content) => {
                    if row.synced_hash == Some(content_hash(&content)) {
                        if let Err(e) = self.store.delete(&row.key).await {
                            eprintln!("vault: orphan delete of {} failed: {e}", row.key);
                            report.failed += 1;
                            continue;
                        }
                    }
                }
                Err(chronacle_core::VaultStoreError::NotFound(_)) => {}
                Err(e) => {
                    eprintln!("vault: orphan read of {} failed: {e}", row.key);
                    report.failed += 1;
                    continue;
                }
            }
            // Sidecars are compiler-owned: delete unconditionally, unlike the
            // record's own file above which is spared if the GM edited it.
            let sidecar = crate::keys::sidecar_key(&row.key);
            match self.store.delete(&sidecar).await {
                Ok(()) | Err(chronacle_core::VaultStoreError::NotFound(_)) => {}
                Err(e) => eprintln!("vault: orphan sidecar delete of {sidecar} failed: {e}"),
            }
            if let Err(e) = self.records.clear_synced_hash(&row.vref).await {
                eprintln!(
                    "vault: clear-synced-hash of {} failed during orphan sweep: {e}",
                    row.vref.to_thing()
                );
                report.failed += 1;
                continue;
            }
            report.swept += 1;
        }

        // Every `Export` above armed a write guard (crate::outbound::PendingWrites)
        // so a watcher event racing the write still matches it. Only `drain_loop`
        // swept those before; a reconcile-heavy workload ("Sync now" repeatedly)
        // would accumulate armed-but-unswept entries indefinitely. Sweep here too
        // so the guard map is self-limiting regardless of which path armed it.
        self.pending.sweep();

        Ok(report)
    }

    /// Apply the GM-owned parts of `file_content` to the DB, then re-export the
    /// canonical render over `key` and set the base. Returns Ok(false) when the
    /// file has no parsable frontmatter (never applied, never overwritten).
    async fn apply_inbound(
        &self,
        vref: &VaultRef,
        key: &str,
        file_content: &str,
    ) -> Result<bool, VaultError> {
        let Ok((_fm, body)) = crate::frontmatter::parse(file_content) else {
            return Ok(false);
        };
        let parts = crate::markdown::split_body(&body);
        let gm = chronacle_core::GmParts {
            summary: parts.summary,
            notes: parts.notes,
        };
        self.records.apply_gm_parts(vref, &gm).await?;

        // Re-export canonical: fence and frontmatter edits are reverted here,
        // and the record settles to NoOp on the next pass.
        let Some(record) = self.records.load(vref).await? else {
            return Ok(true); // deleted mid-flight; the next pass sweeps it
        };
        let rendered = render_record(&record);
        let hash = content_hash(&rendered);
        self.pending.arm(key, hash);
        self.store.write(key, &rendered).await?;
        self.records.set_synced_hash(vref, key, hash).await?;
        Ok(true)
    }

    /// Export a single record, using `index` to find a file the GM already
    /// renamed in the vault instead of blindly writing the computed slug.
    ///
    /// A missing record (deleted between enqueue and drain) is not an error.
    /// The write guard is armed *before* the write so a watcher event racing
    /// the write still matches it (Tranche 5).
    async fn export_one_using(
        &self,
        vref: &VaultRef,
        index: &crate::keys::VaultIndex,
    ) -> Result<(), VaultError> {
        let Some(record) = self.records.load(vref).await? else {
            return Ok(());
        };
        let rendered = render_record(&record);
        let db = content_hash(&rendered);
        // Index wins: a file the GM renamed keeps its name. Fall back to the
        // computed slug (unsuffixed — collision suffixing is reconcile's job) only
        // when the vault holds no file for this id yet.
        let key = index
            .key_of(vref)
            .cloned()
            .unwrap_or_else(|| key_for(&record, false));
        self.pending.arm(&key, db); // arm BEFORE the write so a watcher event matches
        self.store.write(&key, &rendered).await?;
        self.records.set_synced_hash(vref, &key, db).await?;
        Ok(())
    }

    /// Export a single record — the single-ref path.
    ///
    /// Scans the vault index so a renamed file keeps its name.
    pub async fn export_one(&self, vref: &VaultRef) -> Result<(), VaultError> {
        let index = crate::keys::VaultIndex::scan(self.store.as_ref()).await?;
        self.export_one_using(vref, &index).await
    }

    /// Export a batch of refs — the drain path D4a's outbound queue calls.
    ///
    /// One index scan for the whole batch (not per ref — the compile case
    /// enqueues ~200 distinct refs). A failing ref is logged and skipped;
    /// reconcile is the correctness guarantee, this is a latency optimisation.
    pub async fn export_refs(
        &self,
        refs: &std::collections::HashSet<VaultRef>,
    ) -> Result<(), VaultError> {
        let index = crate::keys::VaultIndex::scan(self.store.as_ref()).await?;
        for vref in refs {
            if let Err(e) = self.export_one_using(vref, &index).await {
                eprintln!("vault: export of {} failed: {e}", vref.to_thing());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{
        EntityRecord, MockVaultRecordStore, MockVaultStore, VaultRecord, VaultRef, VaultScope,
        VaultStoreError,
    };
    use std::sync::Arc;

    fn npc(article: Option<&str>) -> VaultRecord {
        VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: "n1".into(),
            },
            name: "Seraphina".into(),
            summary: None,
            notes: Some("N.".into()),
            codex_article: article.map(str::to_owned),
            scope: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "SoV".into(),
            },
            created_at: "x".into(),
            updated_at: "y".into(),
        })
    }
    const KEY: &str = "campaigns/sov/entities/npc/seraphina.md";

    #[tokio::test]
    async fn reconcile_exports_a_record_that_has_never_synced() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store
            .expect_write()
            .withf(|k, _| k == KEY)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(|| Ok(vec![]));
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.exported, 1);
    }

    #[tokio::test]
    async fn reconcile_is_a_noop_when_nothing_changed() {
        let rendered = crate::render::render_record(&npc(Some("A.")));
        let h = crate::render::content_hash(&rendered);

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(rendered.clone()));
        store.expect_write().never(); // the point of the test
        store.expect_delete().never();

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(h),
                conflict: false,
            }])
        });
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.exported, 0);
    }

    /// A recompiled article changes `codex_article` but NOT `updated_at`
    /// (`compile.rs:220-224`). A timestamp-driven reconcile would miss it.
    #[tokio::test]
    async fn reconcile_reexports_a_recompiled_article_despite_unchanged_updated_at() {
        let old = crate::render::render_record(&npc(Some("OLD.")));
        let old_hash = crate::render::content_hash(&old);

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(old.clone()));
        store.expect_write().times(1).returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        // Same updated_at ("y"), different article.
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("NEW."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(old_hash),
                conflict: false,
            }])
        });
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        assert_eq!(svc.reconcile().await.expect("reconcile").exported, 1);
    }

    /// Crash between write and base-update: the file already equals the DB.
    /// Adopt the base; never raise a conflict, never rewrite.
    #[tokio::test]
    async fn reconcile_adopts_a_stale_base_without_conflicting() {
        let rendered = crate::render::render_record(&npc(Some("A.")));

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(rendered.clone()));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(|| {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(999_999), // stale
                conflict: false,
            }])
        });
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.adopted, 1);
        assert_eq!(report.conflicts, 0, "identical sides are not a conflict");
    }

    /// db == base, file differs => Apply: GM parts land in the DB, the canonical
    /// render is re-exported (fence/frontmatter edits reverted), base updated.
    #[tokio::test]
    async fn reconcile_applies_an_inbound_edit_and_reexports_canonical() {
        let old_rendered = crate::render::render_record(&npc(Some("A.")));
        let base = crate::render::content_hash(&old_rendered);
        // The GM's file: valid frontmatter, edited notes, and a tampered fence.
        let gm_file = format!(
            "---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\
             \n{}\nGM EDIT INSIDE FENCE\n{}\n\n## Notes\n\nEdited notes.\n",
            crate::markdown::FENCE_START,
            crate::markdown::FENCE_END
        );

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(gm_file.clone()));
        // The re-export writes the canonical render (fence reverted).
        store
            .expect_write()
            .withf(|k, content| k == KEY && !content.contains("GM EDIT INSIDE FENCE"))
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: false,
            }])
        });
        records
            .expect_apply_gm_parts()
            .withf(|_, parts| parts.notes.as_deref() == Some("Edited notes."))
            .times(1)
            .returning(|_, _| Ok(()));
        // After apply, load() returns the updated record for re-render.
        let updated = {
            let VaultRecord::Entity(mut e) = npc(Some("A.")) else {
                unreachable!()
            };
            e.notes = Some("Edited notes.".into());
            VaultRecord::Entity(e)
        };
        records
            .expect_load()
            .returning(move |_| Ok(Some(updated.clone())));
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.applied, 1);
        assert_eq!(report.applied_refs.len(), 1);
        assert_eq!(report.exported, 0);
    }

    /// A managed file with unparsable frontmatter is counted invalid and never
    /// applied or overwritten.
    #[tokio::test]
    async fn reconcile_counts_an_unparsable_file_as_invalid_and_leaves_it() {
        let base = crate::render::content_hash(&crate::render::render_record(&npc(Some("A."))));
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store
            .expect_read()
            .returning(|_| Ok("no frontmatter at all".to_string()));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: false,
            }])
        });
        records.expect_apply_gm_parts().never();
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.invalid, 1);
        assert_eq!(report.applied, 0);
    }

    /// base set, file gone => SoftDelete: vault_deleted set, base cleared,
    /// file never resurrected.
    #[tokio::test]
    async fn reconcile_soft_deletes_a_record_whose_file_is_gone() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(|| {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(123),
                conflict: false,
            }])
        });
        records.expect_soft_delete().times(1).returning(|_| Ok(()));
        records
            .expect_clear_synced_hash()
            .times(1)
            .returning(|_| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.soft_deleted, 1);
    }

    /// A sync-state row whose record no longer exists (in-app soft delete):
    /// the file is deleted only while it still matches the base.
    #[tokio::test]
    async fn orphan_sweep_deletes_an_unmodified_file_and_spares_an_edited_one() {
        let rendered = crate::render::render_record(&npc(Some("A.")));
        let matching = crate::render::content_hash(&rendered);

        // Case 1: file matches the base -> deleted. Its sidecar is always
        // deleted too (compiler-owned, unconditional; NotFound is fine).
        let sidecar = crate::keys::sidecar_key(KEY);
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(rendered.clone()));
        store
            .expect_delete()
            .withf(|k| k == KEY)
            .times(1)
            .returning(|_| Ok(()));
        let sidecar_for_del = sidecar.clone();
        store
            .expect_delete()
            .withf(move |k| k == sidecar_for_del)
            .times(1)
            .returning(|_| Err(VaultStoreError::NotFound("no sidecar".into())));

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![])); // record is gone
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(matching),
                conflict: false,
            }])
        });
        records
            .expect_clear_synced_hash()
            .times(1)
            .returning(|_| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.swept, 1);

        // Case 2: the GM edited the file after the record died -> file survives,
        // but the sidecar is still deleted unconditionally.
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store
            .expect_read()
            .returning(|_| Ok("GM kept writing here".to_string()));
        store.expect_delete().withf(|k| k == KEY).never();
        let sidecar_for_del2 = sidecar.clone();
        store
            .expect_delete()
            .withf(move |k| k == sidecar_for_del2)
            .times(1)
            .returning(|_| Err(VaultStoreError::NotFound("no sidecar".into())));
        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(matching),
                conflict: false,
            }])
        });
        records
            .expect_clear_synced_hash()
            .times(1)
            .returning(|_| Ok(()));
        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.swept, 1, "row cleared either way");
    }

    /// A file that cannot be read is a per-record failure, not a run-ending one.
    /// Propagating it would let a single unreadable file block every other
    /// record in the vault from ever syncing again.
    #[tokio::test]
    async fn reconcile_continues_past_a_file_it_cannot_read() {
        const BAD: &str = "campaigns/sov/entities/npc/bad.md";

        let bad = VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: "bad".into(),
            },
            name: "Bad".into(),
            summary: None,
            notes: None,
            codex_article: None,
            scope: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "SoV".into(),
            },
            created_at: "x".into(),
            updated_at: "y".into(),
        });
        let good = npc(Some("A.")); // exports to KEY; no file on disk yet

        let mut store = MockVaultStore::new();
        // Only the bad record has a file on disk, and reading it fails.
        store.expect_list().returning(|_| Ok(vec![BAD.to_string()]));
        store.expect_read().returning(|k| {
            if k == BAD {
                Err(VaultStoreError::Io("permission denied".into()))
            } else {
                Ok(String::new())
            }
        });
        // The healthy record must still be exported.
        store
            .expect_write()
            .withf(|k, _| k == KEY)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(move || Ok(vec![bad.clone(), good.clone()]));
        records.expect_list_synced().returning(|| Ok(vec![]));
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc
            .reconcile()
            .await
            .expect("an unreadable file must not fail the run");
        assert_eq!(report.failed, 1, "the unreadable file is counted");
        assert_eq!(
            report.exported, 1,
            "the healthy record must still sync past the bad one"
        );
    }

    #[tokio::test]
    async fn reconcile_suffixes_colliding_slugs() {
        let a = VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: "aaa".into(),
            },
            name: "Guard".into(),
            summary: None,
            notes: None,
            codex_article: None,
            scope: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "SoV".into(),
            },
            created_at: "x".into(),
            updated_at: "y".into(),
        });
        let b = VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: "bbb".into(),
            },
            name: "Guard".into(),
            summary: None,
            notes: None,
            codex_article: None,
            scope: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "SoV".into(),
            },
            created_at: "x".into(),
            updated_at: "y".into(),
        });

        let written = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let w = Arc::clone(&written);
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store.expect_write().returning(move |k, _| {
            w.lock().unwrap().push(k.to_string());
            Ok(())
        });

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(move || Ok(vec![a.clone(), b.clone()]));
        records.expect_list_synced().returning(|| Ok(vec![]));
        records.expect_set_synced_hash().returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        svc.reconcile().await.expect("reconcile");

        let keys = written.lock().unwrap().clone();
        assert_eq!(keys.len(), 2);
        assert_ne!(keys[0], keys[1], "colliding names must not share a key");
    }

    /// Every `Export` arms a write guard (`crate::outbound::PendingWrites`),
    /// but only `drain_loop` used to sweep them. A "Sync now"-heavy workload
    /// would accumulate armed-but-unswept entries forever. `reconcile` must
    /// sweep expired guards itself, even ones it did not arm this pass.
    #[tokio::test]
    async fn reconcile_sweeps_expired_guards_it_did_not_arm() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![])); // nothing to reconcile
        records.expect_list_synced().returning(|| Ok(vec![]));

        let pending = Arc::new(crate::outbound::PendingWrites::default());
        // A guard from a previous batch that never got its watcher event.
        pending.arm_at(
            "stale.md",
            1,
            std::time::Instant::now()
                - crate::outbound::PendingWrites::TTL
                - std::time::Duration::from_secs(1),
        );
        assert_eq!(pending.len(), 1);

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records), Arc::clone(&pending));
        svc.reconcile().await.expect("reconcile");

        assert_eq!(
            pending.len(),
            0,
            "reconcile must sweep expired guards, not just arm new ones"
        );
    }

    /// A file that loses both its frontmatter id AND its expected filename
    /// (renamed and corrupted in the same GM edit) is unattributable: `key_of`
    /// misses because there is no parsable id, and `has_key` misses because
    /// the name no longer matches the computed slug. `decide` sees no file
    /// and soft-deletes the record. That is safe and non-destructive — it
    /// only flips `vault_deleted` (recoverable) — and the file itself, still
    /// sitting on disk under its new name, is never touched. Recovering it is
    /// the deferred "id-less file adoption" feature, not this arm's job.
    #[tokio::test]
    async fn a_renamed_and_corrupted_file_soft_deletes_the_record_but_never_touches_the_file() {
        const RENAMED_CORRUPT_KEY: &str = "campaigns/sov/entities/npc/renamed-corrupt.md";

        let mut store = MockVaultStore::new();
        store
            .expect_list()
            .returning(|_| Ok(vec![RENAMED_CORRUPT_KEY.to_string()]));
        // Scanned once to build the index; unparsable, so it lands only in
        // `managed_keys`, never in `by_ref`.
        store
            .expect_read()
            .withf(|k| k == RENAMED_CORRUPT_KEY)
            .returning(|_| Ok("no frontmatter, and not named seraphina.md either".to_string()));
        store.expect_write().never();
        store.expect_delete().never();

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(|| {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(123),
                conflict: false,
            }])
        });
        records.expect_soft_delete().times(1).returning(|_| Ok(()));
        records
            .expect_clear_synced_hash()
            .times(1)
            .returning(|_| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.soft_deleted, 1);
    }

    #[tokio::test]
    async fn reconcile_reports_an_io_failure_without_aborting_the_run() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store
            .expect_write()
            .returning(|_, _| Err(VaultStoreError::Io("disk full".into())));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(|| Ok(vec![]));
        records.expect_set_synced_hash().never(); // never claim a base we did not write

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc
            .reconcile()
            .await
            .expect("a failing key must not fail the run");
        assert_eq!(report.exported, 0);
        assert_eq!(report.failed, 1);
    }

    /// A failing `soft_delete` on one record must be logged and counted, not
    /// let abort the whole pass — the SoftDelete arm must treat DB errors the
    /// same way Export/Apply already do.
    #[tokio::test]
    async fn reconcile_continues_past_a_failing_soft_delete() {
        const KEY2: &str = "campaigns/sov/entities/npc/second.md";
        let second = VaultRecord::Entity(EntityRecord {
            vref: VaultRef {
                table: "npc".into(),
                id: "n2".into(),
            },
            name: "Second".into(),
            summary: None,
            notes: None,
            codex_article: None,
            scope: VaultScope::Campaign {
                id: "campaign:c1".into(),
                name: "SoV".into(),
            },
            created_at: "x".into(),
            updated_at: "y".into(),
        });

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store
            .expect_write()
            .withf(|k, _| k == KEY2)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(move || Ok(vec![npc(Some("A.")), second.clone()]));
        records.expect_list_synced().returning(|| {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(123),
                conflict: false,
            }])
        });
        records
            .expect_soft_delete()
            .times(1)
            .returning(|_| Err(chronacle_core::VaultRecordError::Backend("db down".into())));
        records.expect_clear_synced_hash().never(); // soft_delete failed first — never called
        records
            .expect_set_synced_hash()
            .withf(|_, k, _| k == KEY2)
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc
            .reconcile()
            .await
            .expect("a failing soft-delete must not fail the run");
        assert_eq!(report.soft_deleted, 0);
        assert_eq!(report.failed, 1);
        assert_eq!(report.exported, 1, "the second record is still processed");
    }

    // -- conflict lifecycle (E4) -----------------------------------------

    /// Both sides diverged from the base: the DB render lands in the sidecar,
    /// the GM's file is untouched, the record is frozen (no base update).
    #[tokio::test]
    async fn a_new_conflict_writes_the_sidecar_and_freezes_the_record() {
        let base = 111_u64; // both sides differ from it and from each other
        let db_render = crate::render::render_record(&npc(Some("A.")));

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().withf(|k| k == KEY).returning(|_| {
            Ok(
                "---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\nGM version.\n"
                    .to_string(),
            )
        });
        let expected_sidecar = crate::keys::sidecar_key(KEY);
        let expected_sidecar_read = expected_sidecar.clone();
        store
            .expect_read()
            .withf(move |k| k == expected_sidecar_read)
            .returning(|_| Err(VaultStoreError::NotFound("no sidecar yet".into())));
        let render_for_assert = db_render.clone();
        store
            .expect_write()
            .withf(move |k, content| k == expected_sidecar && content == render_for_assert)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: false,
            }])
        });
        records
            .expect_set_conflict()
            .withf(|_, _, flag| *flag)
            .times(1)
            .returning(|_, _, _| Ok(()));
        records.expect_apply_gm_parts().never();
        records.expect_set_synced_hash().never(); // frozen: no base movement

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.conflicts, 1);
    }

    /// Frozen and the sidecar already reflects a stale DB render: refresh it
    /// in place. Still frozen — the base does not move, GM parts are untouched.
    #[tokio::test]
    async fn a_frozen_conflict_with_a_live_sidecar_stays_frozen_and_refreshes_a_stale_sidecar() {
        let base = 111_u64;
        let db_render = crate::render::render_record(&npc(Some("A.")));
        let old_sidecar = crate::render::render_record(&npc(Some("OLD.")));

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().withf(|k| k == KEY).returning(|_| {
            Ok(
                "---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\nGM version.\n"
                    .to_string(),
            )
        });
        let sidecar_key = crate::keys::sidecar_key(KEY);
        let sidecar_key_read = sidecar_key.clone();
        store
            .expect_read()
            .withf(move |k| k == sidecar_key_read)
            .returning(move |_| Ok(old_sidecar.clone()));
        let sidecar_key_write = sidecar_key.clone();
        let db_render_write = db_render.clone();
        store
            .expect_write()
            .withf(move |k, content| k == sidecar_key_write && content == db_render_write)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: true,
            }])
        });
        records.expect_set_conflict().never();
        records.expect_apply_gm_parts().never();
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.conflicts, 1);
    }

    /// Deleting the sidecar is the GM's resolution signal: the GM file is
    /// applied inbound, re-exported canonically, and the record unfreezes.
    #[tokio::test]
    async fn deleting_the_sidecar_resolves_the_conflict_by_applying_the_gm_file() {
        let base = 111_u64;
        let gm_file =
            "---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\n## Notes\n\nGM resolved.\n"
                .to_string();

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        let gm_file_read = gm_file.clone();
        store
            .expect_read()
            .withf(|k| k == KEY)
            .returning(move |_| Ok(gm_file_read.clone()));
        let sidecar_key = crate::keys::sidecar_key(KEY);
        let sidecar_key_read = sidecar_key.clone();
        store
            .expect_read()
            .withf(move |k| k == sidecar_key_read)
            .returning(|_| Err(VaultStoreError::NotFound("gone".into())));
        store
            .expect_write()
            .withf(|k, _| k == KEY)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: true,
            }])
        });
        records
            .expect_apply_gm_parts()
            .withf(|_, parts| parts.notes.as_deref() == Some("GM resolved."))
            .times(1)
            .returning(|_, _| Ok(()));
        let updated = {
            let VaultRecord::Entity(mut e) = npc(Some("A.")) else {
                unreachable!()
            };
            e.notes = Some("GM resolved.".into());
            VaultRecord::Entity(e)
        };
        records
            .expect_load()
            .returning(move |_| Ok(Some(updated.clone())));
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));
        records
            .expect_set_conflict()
            .withf(|_, _, flag| !*flag)
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.resolved, 1);
    }

    /// The conflict evaporates on its own (the GM reverted the file back to
    /// the base): the sidecar is cleaned up and the record unfreezes.
    #[tokio::test]
    async fn an_evaporated_conflict_cleans_up_the_sidecar_and_proceeds() {
        let rendered = crate::render::render_record(&npc(Some("A.")));
        let base = crate::render::content_hash(&rendered);

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        let rendered_read = rendered.clone();
        store
            .expect_read()
            .withf(|k| k == KEY)
            .returning(move |_| Ok(rendered_read.clone()));
        let sidecar_key = crate::keys::sidecar_key(KEY);
        let sidecar_key_del = sidecar_key.clone();
        store
            .expect_delete()
            .withf(move |k| k == sidecar_key_del)
            .times(1)
            .returning(|_| Ok(()));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: true,
            }])
        });
        records
            .expect_set_conflict()
            .withf(|_, _, flag| !*flag)
            .times(1)
            .returning(|_, _, _| Ok(()));
        records.expect_apply_gm_parts().never();
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.conflicts, 0);
    }

    /// A file already claims a record's id before the record ever synced (no
    /// sync-state row exists yet): `decide` returns `Conflict` with `base ==
    /// None`. `set_conflict` UPSERTs a key-only row (no base); the next pass
    /// still reads `Conflict` (frozen) until the GM deletes the sidecar, at
    /// which point the file is adopted.
    #[tokio::test]
    async fn an_unmanaged_file_conflict_freezes_then_resolves_on_sidecar_deletion() {
        let gm_file =
            "---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\n## Notes\n\nPre-existing GM file.\n"
                .to_string();

        // Pass 1: new conflict, no prior sync-state row.
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        let gm_file_read = gm_file.clone();
        store
            .expect_read()
            .withf(|k| k == KEY)
            .returning(move |_| Ok(gm_file_read.clone()));
        let sidecar_key = crate::keys::sidecar_key(KEY);
        let sidecar_key_read = sidecar_key.clone();
        store
            .expect_read()
            .withf(move |k| k == sidecar_key_read)
            .returning(|_| Err(VaultStoreError::NotFound("no sidecar yet".into())));
        let sidecar_key_write = sidecar_key.clone();
        store
            .expect_write()
            .withf(move |k, _| k == sidecar_key_write)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(|| Ok(vec![])); // no row yet
        records
            .expect_set_conflict()
            .withf(|_, _, flag| *flag)
            .times(1)
            .returning(|_, _, _| Ok(()));
        records.expect_apply_gm_parts().never();

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile pass 1");
        assert_eq!(report.conflicts, 1);

        // Pass 2: frozen row now exists with no base (key-only UPSERT); the GM
        // deletes the sidecar, resolving in favour of the file.
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        let gm_file_read2 = gm_file.clone();
        store
            .expect_read()
            .withf(|k| k == KEY)
            .returning(move |_| Ok(gm_file_read2.clone()));
        let sidecar_key2 = crate::keys::sidecar_key(KEY);
        let sidecar_key_read2 = sidecar_key2.clone();
        store
            .expect_read()
            .withf(move |k| k == sidecar_key_read2)
            .returning(|_| Err(VaultStoreError::NotFound("gone".into())));
        store
            .expect_write()
            .withf(|k, _| k == KEY)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef {
                    table: "npc".into(),
                    id: "n1".into(),
                },
                key: KEY.into(),
                synced_hash: None, // no base: this row exists only for the conflict flag
                conflict: true,
            }])
        });
        records.expect_apply_gm_parts().returning(|_, _| Ok(()));
        let updated = npc(Some("A."));
        records
            .expect_load()
            .returning(move |_| Ok(Some(updated.clone())));
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));
        records
            .expect_set_conflict()
            .withf(|_, _, flag| !*flag)
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile pass 2");
        assert_eq!(report.resolved, 1);
    }

    // -- export_one -----------------------------------------------------
    //
    // Not covered by the brief's Step 1 tests; added so the public
    // single-record drain path (D4a) doesn't ship untested.

    #[tokio::test]
    async fn export_one_is_a_noop_when_the_record_is_gone() {
        let mut store = MockVaultStore::new();
        // export_one now scans the index first (empty vault → no existing file).
        store.expect_list().returning(|_| Ok(vec![]));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records.expect_load().returning(|_| Ok(None));
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let vref = VaultRef {
            table: "npc".into(),
            id: "gone".into(),
        };
        svc.export_one(&vref)
            .await
            .expect("a deleted record is not an error");
    }

    #[tokio::test]
    async fn export_one_writes_the_record_and_sets_the_base() {
        let mut store = MockVaultStore::new();
        // Empty vault → index has no entry for this ref → falls back to the
        // computed slug.
        store.expect_list().returning(|_| Ok(vec![]));
        store
            .expect_write()
            .withf(|k, _| k == KEY)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_load()
            .returning(|_| Ok(Some(npc(Some("A.")))));
        records
            .expect_set_synced_hash()
            .withf(|_, k, _| k == KEY)
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };
        svc.export_one(&vref).await.expect("export_one");
    }

    /// The export path must be index-aware: a file the GM renamed in the
    /// vault keeps its name — the drain must not create a stray duplicate at
    /// the computed slug. This is the test that proves the D4a fix.
    #[tokio::test]
    async fn export_refs_writes_to_the_existing_renamed_key() {
        const RENAMED_KEY: &str = "campaigns/sov/entities/npc/renamed.md";

        let record = npc(Some("A."));
        let rendered_for_index = crate::render::render_record(&record);

        let mut store = MockVaultStore::new();
        store
            .expect_list()
            .returning(|_| Ok(vec![RENAMED_KEY.to_string()]));
        store
            .expect_read()
            .withf(|k| k == RENAMED_KEY)
            .returning(move |_| Ok(rendered_for_index.clone()));
        store
            .expect_write()
            .withf(|k, _| k == RENAMED_KEY)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records
            .expect_load()
            .returning(|_| Ok(Some(npc(Some("A.")))));
        records
            .expect_set_synced_hash()
            .withf(|_, k, _| k == RENAMED_KEY)
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };
        let mut refs = std::collections::HashSet::new();
        refs.insert(vref);

        svc.export_refs(&refs).await.expect("export_refs");
    }
}
