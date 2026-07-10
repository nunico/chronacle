//! Bidirectional reconcile — export direction only in this tranche.
//!
//! Reconcile is the **correctness guarantee**; the outbound queue (D4a) is only
//! a latency optimisation. A dropped `enqueue()` degrades to "the file updates
//! on next reconcile", never to "the file is permanently wrong". That is also
//! why a backend with no change feed (S3, WebDAV) is still correct.

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
    /// Inbound edits seen but not applied (tranche 5).
    pub deferred_apply: usize,
    /// Divergent edits seen but not materialised (tranche 5).
    pub deferred_conflict: usize,
    /// Vault deletions seen but not soft-deleted (tranche 5).
    pub deferred_delete: usize,
    /// Keys that failed to write. The run continues.
    pub failed: usize,
}

/// The vault sync engine.
pub struct VaultSyncService {
    store: Arc<dyn VaultStore>,
    records: Arc<dyn VaultRecordStore>,
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
    /// Construct the engine over a storage backend and a record backend.
    pub fn new(store: Arc<dyn VaultStore>, records: Arc<dyn VaultRecordStore>) -> Self {
        Self { store, records }
    }

    /// Run one export-direction reconcile pass over every syncable record.
    ///
    /// Loads every record and the on-disk vault index, computes a three-way
    /// sync decision per record, and acts on `Export`/`AdoptBase`/`NoOp`.
    /// `Apply`, `Conflict`, and `SoftDelete` are only counted and logged —
    /// tranche 5 turns those on.
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
            let file = match &existing_key {
                Some(k) => Some(content_hash(&self.store.read(k).await?)),
                None => None,
            };
            let base = self.records.get_synced_hash(vref).await?;

            match decide(base, db, file) {
                SyncAction::NoOp => report.unchanged += 1,
                SyncAction::AdoptBase => {
                    self.records.set_synced_hash(vref, &key, db).await?;
                    report.adopted += 1;
                }
                SyncAction::Export => match self.store.write(&key, &rendered).await {
                    Ok(()) => {
                        self.records.set_synced_hash(vref, &key, db).await?;
                        report.exported += 1;
                    }
                    Err(e) => {
                        eprintln!("vault: failed to write {key}: {e}");
                        report.failed += 1;
                        continue;
                    }
                },
                action @ (SyncAction::Apply | SyncAction::Conflict | SyncAction::SoftDelete) => {
                    eprintln!(
                        "vault: inbound action deferred to tranche 5: {action:?} for {vref:?}"
                    );
                    match action {
                        SyncAction::Apply => report.deferred_apply += 1,
                        SyncAction::Conflict => report.deferred_conflict += 1,
                        SyncAction::SoftDelete => report.deferred_delete += 1,
                        _ => unreachable!(),
                    }
                }
            }
        }

        Ok(report)
    }

    /// Export a single record — the drain path D4a's outbound queue calls.
    ///
    /// A missing record (deleted between enqueue and drain) is not an error.
    pub async fn export_one(&self, vref: &VaultRef) -> Result<(), VaultError> {
        let Some(record) = self.records.load(vref).await? else {
            return Ok(());
        };
        let rendered = render_record(&record);
        let db = content_hash(&rendered);
        let key = key_for(&record, false);
        self.store.write(&key, &rendered).await?;
        self.records.set_synced_hash(vref, &key, db).await?;
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
        records.expect_get_synced_hash().returning(|_| Ok(None));
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
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
        records
            .expect_get_synced_hash()
            .returning(move |_| Ok(Some(h)));
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
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
        records
            .expect_get_synced_hash()
            .returning(move |_| Ok(Some(old_hash)));
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
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
        records
            .expect_get_synced_hash()
            .returning(|_| Ok(Some(999_999))); // stale
        records
            .expect_set_synced_hash()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.adopted, 1);
        assert_eq!(
            report.deferred_conflict, 0,
            "identical sides are not a conflict"
        );
    }

    #[tokio::test]
    async fn reconcile_defers_apply_and_conflict_without_writing() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(|_| {
            Ok("---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\nGM edited this.\n"
                .to_string())
        });
        store.expect_write().never(); // export-only tranche must not clobber
        store.expect_delete().never();

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        // db == base, file differs => Apply (deferred to tranche 5)
        records.expect_get_synced_hash().returning(|_| {
            Ok(Some(crate::render::content_hash(
                &crate::render::render_record(&npc(Some("A."))),
            )))
        });
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.deferred_apply, 1);
        assert_eq!(report.exported, 0);
    }

    #[tokio::test]
    async fn reconcile_defers_soft_delete_and_does_not_resurrect_the_file() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![])); // file is gone
        store.expect_write().never(); // MUST NOT rewrite it

        let mut records = MockVaultRecordStore::new();
        records
            .expect_list_all()
            .returning(|| Ok(vec![npc(Some("A."))]));
        records
            .expect_get_synced_hash()
            .returning(|_| Ok(Some(123))); // we wrote it once
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.deferred_delete, 1);
        assert_eq!(
            report.exported, 0,
            "reconcile must never resurrect a deleted file"
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
        records.expect_get_synced_hash().returning(|_| Ok(None));
        records.expect_set_synced_hash().returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        svc.reconcile().await.expect("reconcile");

        let keys = written.lock().unwrap().clone();
        assert_eq!(keys.len(), 2);
        assert_ne!(keys[0], keys[1], "colliding names must not share a key");
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
        records.expect_get_synced_hash().returning(|_| Ok(None));
        records.expect_set_synced_hash().never(); // never claim a base we did not write

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc
            .reconcile()
            .await
            .expect("a failing key must not fail the run");
        assert_eq!(report.exported, 0);
        assert_eq!(report.failed, 1);
    }

    // -- export_one -----------------------------------------------------
    //
    // Not covered by the brief's Step 1 tests; added so the public
    // single-record drain path (D4a) doesn't ship untested.

    #[tokio::test]
    async fn export_one_is_a_noop_when_the_record_is_gone() {
        let mut store = MockVaultStore::new();
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records.expect_load().returning(|_| Ok(None));
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
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

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let vref = VaultRef {
            table: "npc".into(),
            id: "n1".into(),
        };
        svc.export_one(&vref).await.expect("export_one");
    }
}
