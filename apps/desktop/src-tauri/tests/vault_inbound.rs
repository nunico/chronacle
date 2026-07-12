//! L2 (path-switch) + E3 (inbound reconcile) integration coverage.

use std::sync::Arc;

use chronacle_core::{VaultRecordStore, VaultRef, VaultStoreError};
use chronacle_domain::vault_record_store::SurrealVaultRecordStore;
use chronacle_lib::commands::vault_commands::configure_vault_path;
use chronacle_lib::services::settings_service;
use chronacle_providers::vault_store::LocalFsVaultStore;
use chronacle_vault::outbound::PendingWrites;
use chronacle_vault::reconcile::VaultSyncService;

async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .expect("mem");
    db.use_ns("t").use_db("t").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db.query(
        "CREATE campaign:c1 SET name = 'SoV', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'Seraphina', notes = 'N.', \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    )
    .await
    .expect("seed")
    .check()
    .expect("seed ok");
    db
}

fn svc_for(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    root: &std::path::Path,
) -> Arc<VaultSyncService> {
    Arc::new(VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(root.to_str().unwrap())),
        Arc::new(SurrealVaultRecordStore::new(db.clone())),
        Arc::new(PendingWrites::default()),
    ))
}

#[tokio::test]
async fn switching_to_a_fresh_dir_after_clearing_bases_exports_cleanly() {
    let db = db().await;
    let dir_a = tempfile::TempDir::new().unwrap();
    let dir_b = tempfile::TempDir::new().unwrap();

    // First vault: export establishes a base.
    let a = svc_for(&db, dir_a.path());
    let r = a.reconcile().await.expect("reconcile a");
    assert_eq!(r.exported, 1);

    // Switch: fresh baseline, then reconcile against the empty dir B.
    let b = svc_for(&db, dir_b.path());
    b.clear_all_bases().await.expect("clear");
    let r = b.reconcile().await.expect("reconcile b");
    assert_eq!(
        r.exported, 1,
        "a fresh dir is a first export, not a deletion"
    );

    // The record's file exists in B; nothing was flagged deleted.
    let store = SurrealVaultRecordStore::new(db.clone());
    let vref = VaultRef {
        table: "npc".into(),
        id: "n1".into(),
    };
    assert!(store.get_synced_hash(&vref).await.expect("get").is_some());
}

/// Drives the command-layer helper directly (no Tauri `State`), exercising
/// the exact ordering `set_vault_path` relies on: clear-if-changed → reconcile
/// → persist. This is what the two tests below prove pieces of.
#[tokio::test]
async fn resubmitting_the_same_path_preserves_the_base_and_exports_an_edit() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().to_str().unwrap().to_string();

    let svc = svc_for(&db, dir.path());
    configure_vault_path(&db, &svc, &path)
        .await
        .expect("first configure exports the initial record");

    let store = SurrealVaultRecordStore::new(db.clone());
    let vref = VaultRef {
        table: "npc".into(),
        id: "n1".into(),
    };
    let hash_1 = store
        .get_synced_hash(&vref)
        .await
        .expect("get")
        .expect("base set after first configure");

    // Edit the record in the DB without touching the vault file — this is
    // the "Sync now" scenario: same path, changed content.
    db.query("UPDATE npc:n1 SET notes = 'Revised.', updated_at = time::now()")
        .await
        .expect("edit")
        .check()
        .expect("edit ok");

    // Re-submitting the SAME path must NOT wipe the base. If it did, the
    // base would go to `None` while the on-disk file still holds the old
    // content, and since the DB now disagrees with both, `decide()` would
    // read that as a `Conflict` (deferred, no write, no base update) instead
    // of the plain `Export` it actually is — leaving `hash_1` stuck forever
    // instead of advancing.
    configure_vault_path(&db, &svc, &path)
        .await
        .expect("re-submitting the same path reconciles cleanly");

    let hash_2 = store
        .get_synced_hash(&vref)
        .await
        .expect("get")
        .expect("base must still be present, not wiped");
    assert_ne!(
        hash_2, hash_1,
        "the edit must be exported and the base advanced, not stuck at the pre-edit hash \
         (a base stuck at hash_1 means the base was wrongly cleared and the edit was \
         misread as a Conflict instead of an Export)"
    );
}

/// A `VaultStore` whose `list` always errors — `VaultIndex::scan` propagates
/// that with `?`, making `reconcile()` fail before it writes or touches any
/// base. Used to prove a failed reconcile never reaches the `upsert`.
fn failing_svc(db: &surrealdb::Surreal<surrealdb::engine::any::Any>) -> Arc<VaultSyncService> {
    let mut store = chronacle_core::MockVaultStore::new();
    store.expect_list().returning(|_| {
        Err(VaultStoreError::Io(
            "simulated: cannot read vault root".into(),
        ))
    });
    Arc::new(VaultSyncService::new(
        Arc::new(store),
        Arc::new(SurrealVaultRecordStore::new(db.clone())),
        Arc::new(PendingWrites::default()),
    ))
}

#[tokio::test]
async fn a_failing_reconcile_does_not_persist_the_new_vault_path() {
    let db = db().await;
    let dir_old = tempfile::TempDir::new().unwrap();
    let old_path = dir_old.path().to_str().unwrap().to_string();

    // Establish a working, persisted "old" configuration first.
    let old_svc = svc_for(&db, dir_old.path());
    configure_vault_path(&db, &old_svc, &old_path)
        .await
        .expect("initial configure succeeds");

    // Attempt to switch to a path whose reconcile always fails.
    let broken = failing_svc(&db);
    let err = configure_vault_path(&db, &broken, "/some/other/path")
        .await
        .expect_err("a failing reconcile must surface as an error");
    assert!(!err.is_empty());

    // The setting must still hold the OLD path — the failed switch did not
    // take effect. This fails if the `upsert` is moved back before the
    // `reconcile` call.
    let persisted = settings_service::get_all(&db)
        .await
        .expect("get_all")
        .into_iter()
        .find(|s| s.key == "vault_sync_path")
        .map(|s| s.value);
    assert_eq!(
        persisted.as_deref(),
        Some(old_path.as_str()),
        "a failed vault path switch must leave the previous path in force"
    );
}

#[tokio::test]
async fn gm_edit_round_trips_through_reconcile_into_the_db() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("first export");

    // Find the exported file and append GM notes outside the fence.
    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\n\nInbound edit from Obsidian.\n")).unwrap();

    let report = svc.reconcile().await.expect("inbound pass");
    assert_eq!(report.applied, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        notes: Option<String>,
    }
    let mut resp = db
        .query("SELECT notes FROM npc:n1")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0]
        .notes
        .as_deref()
        .unwrap_or("")
        .contains("Inbound edit from Obsidian."));

    // Third pass: everything converged.
    let report = svc.reconcile().await.expect("settle");
    assert_eq!(report.unchanged, 1);
}

#[tokio::test]
async fn conflict_freezes_then_sidecar_deletion_resolves_to_the_file_version() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    // Diverge BOTH sides: edit the file, and edit the DB notes.
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit.'")
        .await
        .unwrap()
        .check()
        .unwrap();

    let report = svc.reconcile().await.expect("conflict pass");
    assert_eq!(report.conflicts, 1);
    let sidecar = path.with_file_name(format!(
        "{}.conflict.md",
        path.file_stem().unwrap().to_str().unwrap()
    ));
    assert!(sidecar.exists(), "DB version preserved in the sidecar");
    assert!(std::fs::read_to_string(&sidecar)
        .unwrap()
        .contains("App-side edit."));

    // Frozen: another pass changes nothing, file untouched.
    let report = svc.reconcile().await.expect("frozen pass");
    assert_eq!(report.conflicts, 1);
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("Vault-side edit."));

    // GM resolves by deleting the sidecar.
    std::fs::remove_file(&sidecar).unwrap();
    let report = svc.reconcile().await.expect("resolution pass");
    assert_eq!(report.resolved, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        notes: Option<String>,
    }
    let mut resp = db
        .query("SELECT notes FROM npc:n1")
        .await
        .unwrap()
        .check()
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0]
        .notes
        .as_deref()
        .unwrap()
        .contains("Vault-side edit."));
}

/// E5: soft-deleting an entity in the app must sweep its vault file on the
/// next reconcile — the same orphan-sweep path a hard delete already used.
#[tokio::test]
async fn soft_deleting_an_entity_sweeps_its_vault_file_on_reconcile() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("first export");

    let entities_dir = dir.path().join("campaigns/sov/entities/npc");
    assert_eq!(entities_dir.read_dir().unwrap().count(), 1);

    chronacle_extraction::entity_service::soft_delete(
        &db,
        "n1",
        chronacle_extraction::entity_service::EntityKind::Npc,
    )
    .await
    .expect("soft delete");

    let report = svc.reconcile().await.expect("sweep pass");
    assert_eq!(report.soft_deleted, 0, "already soft-deleted, not newly so");
    assert_eq!(report.swept, 1, "orphaned sync row is swept");
    assert_eq!(
        entities_dir.read_dir().unwrap().count(),
        0,
        "the vault file must be removed"
    );
}

/// E5: `VaultSyncService::conflicts()` end-to-end — a genuine two-sided
/// divergence freezes the record, and `conflicts()` surfaces it with a
/// resolved display name and the sidecar's key.
#[tokio::test]
async fn conflicts_lists_a_frozen_record_end_to_end() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    assert!(
        svc.conflicts().await.expect("conflicts").is_empty(),
        "nothing frozen yet"
    );

    let path = dir
        .path()
        .join("campaigns/sov/entities/npc")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit.'")
        .await
        .unwrap()
        .check()
        .unwrap();
    svc.reconcile().await.expect("conflict pass");

    let conflicts = svc.conflicts().await.expect("conflicts");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].vref.table, "npc");
    assert_eq!(conflicts[0].vref.id, "n1");
    assert_eq!(conflicts[0].name, "Seraphina");
    assert!(conflicts[0].sidecar_key.ends_with(".conflict.md"));
}
