//! L2: switching vault folders must never read as mass deletion.

use std::sync::Arc;

use chronacle_core::{VaultRecordStore, VaultRef};
use chronacle_domain::vault_record_store::SurrealVaultRecordStore;
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
