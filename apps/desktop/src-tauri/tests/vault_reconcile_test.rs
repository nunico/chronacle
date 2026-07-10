//! End-to-end reconcile over a real temp vault and a real in-memory database.

use std::sync::Arc;

use chronacle_domain::vault_record_store::SurrealVaultRecordStore;
use chronacle_providers::vault_store::LocalFsVaultStore;
use chronacle_vault::reconcile::VaultSyncService;
use tempfile::TempDir;

async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://")
        .await
        .expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db
}

/// Gherkin: Given a campaign with entities and no vault configured, when the GM
/// sets a vault path, then a full reconcile writes one .md per record, and each
/// entity file carries `aliases` matching its name.
#[tokio::test]
async fn reconcile_writes_one_file_per_record_with_resolving_aliases() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'Shadows of Valdris', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'Seraphina Aldric', notes = 'GM notes', \
             codex_article = 'She guards [[The Iron Tower]].', \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    )
    .await
    .expect("seed")
    .check()
    .expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );

    let report = svc.reconcile().await.expect("reconcile");
    assert_eq!(report.exported, 1);

    let path = dir
        .path()
        .join("campaigns/shadows-of-valdris/entities/npc/seraphina-aldric.md");
    let content = std::fs::read_to_string(&path).expect("file must exist at the derived key");
    assert!(content.contains(r#"id: "npc:n1""#));
    assert!(
        content.contains(r#"aliases: ["Seraphina Aldric"]"#),
        "wikilinks would break without this"
    );
    assert!(content.contains("[[The Iron Tower]]"));
    assert!(!content.contains("is_gm_only"));
}

/// Gherkin: Given a configured vault and no changes, when the GM clicks
/// "Sync now", then no file contents change.
#[tokio::test]
async fn a_second_reconcile_writes_nothing() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'C', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'A', created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    ).await.expect("seed").check().expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );

    assert_eq!(svc.reconcile().await.expect("first").exported, 1);
    let second = svc.reconcile().await.expect("second");
    assert_eq!(second.exported, 0);
    assert_eq!(second.unchanged, 1);
}

/// Gherkin: Given a record with vault_deleted = TRUE, when reconcile runs,
/// then no file is written for it.
#[tokio::test]
async fn reconcile_skips_soft_deleted_records() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'C', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'A', vault_deleted = true, created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    ).await.expect("seed").check().expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );
    assert_eq!(svc.reconcile().await.expect("reconcile").exported, 0);
    assert!(
        !dir.path().join("campaigns/c/entities/npc/a.md").exists(),
        "a soft-deleted record must never be written to the vault"
    );
}

/// Gherkin: Given a collection subscribed to two campaigns, when reconcile
/// runs, then its entities appear exactly once, under collections/<slug>/.
#[tokio::test]
async fn a_shared_collection_entity_is_written_once_under_collections() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'One', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE campaign:c2 SET name = 'Two', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE collection:k1 SET name = 'Core', created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->subscribes_to->collection:k1 SET created_at = time::now(); \
         RELATE campaign:c2->subscribes_to->collection:k1 SET created_at = time::now(); \
         CREATE creature:g1 SET name = 'Goblin', created_at = time::now(), updated_at = time::now(); \
         RELATE collection:k1->in_collection->creature:g1;",
    ).await.expect("seed").check().expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );
    assert_eq!(svc.reconcile().await.expect("reconcile").exported, 1);
    assert!(dir
        .path()
        .join("collections/core/entities/creature/goblin.md")
        .exists());
    assert!(!dir
        .path()
        .join("campaigns/one/entities/creature/goblin.md")
        .exists());
    assert!(!dir
        .path()
        .join("campaigns/two/entities/creature/goblin.md")
        .exists());
}
