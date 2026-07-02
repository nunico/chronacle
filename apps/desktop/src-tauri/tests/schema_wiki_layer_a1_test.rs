//! Schema-level tests for the LLM Wiki layer migration (`002_wiki_layer.surql`).
//!
//! These tests focus on the *observable* schema surface added by A1a and are
//! deliberately kept small. Domain-level behaviour (auto-create, orphan-log,
//! cascade) lives in `campaign_service_test.rs`.

use serde::Deserialize;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[tokio::test]
async fn migration_002_is_idempotent_when_run_twice() {
    // First run happens inside setup_db.
    let db = setup_db().await;

    // Seed a collection so we can prove data survives a second run of the
    // full migration set.
    db.query(
        "CREATE collection SET \
            id = 'coll-idem', \
            name = 'Idempotency Sentinel', \
            description = NULL, \
            created_at = time::now(), \
            updated_at = time::now()",
    )
    .await
    .unwrap();

    // Second run — must be a no-op.
    chronacle_db::run_migrations(&db).await.unwrap();

    #[derive(Deserialize)]
    struct Row {
        count: i64,
    }
    let mut resp = db
        .query("SELECT count() FROM collection WHERE id = collection:`coll-idem` GROUP ALL")
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.first().map(|r| r.count).unwrap_or(0), 1);
}

#[tokio::test]
async fn collection_owner_campaign_defaults_to_none() {
    let db = setup_db().await;

    db.query(
        "CREATE collection SET \
            id = 'coll-default', \
            name = 'No Owner', \
            description = NULL, \
            created_at = time::now(), \
            updated_at = time::now()",
    )
    .await
    .unwrap();

    #[derive(Deserialize)]
    struct Row {
        count: i64,
    }
    let mut resp = db
        .query(
            "SELECT count() FROM collection \
             WHERE id = collection:`coll-default` \
               AND owner_campaign = NONE GROUP ALL",
        )
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(
        rows.first().map(|r| r.count).unwrap_or(0),
        1,
        "owner_campaign must default to NONE for collections created without it"
    );
}

#[tokio::test]
async fn lint_finding_accepts_orphaned_edge_row() {
    let db = setup_db().await;

    db.query(
        "CREATE lint_finding SET \
            kind = 'orphaned_edge', \
            payload = { \
                campaign_id: 'campaign:abc', \
                collection_id: 'collection:def', \
                edge_id: 'relates_to:xyz', \
                from: 'npc:1', \
                to: 'npc:2', \
                rel_type: 'allied_with' \
            }",
    )
    .await
    .unwrap()
    .check()
    .expect("lint_finding must accept an orphaned_edge row");

    #[derive(Deserialize)]
    struct Row {
        count: i64,
    }
    let mut resp = db
        .query("SELECT count() FROM lint_finding WHERE kind = 'orphaned_edge' GROUP ALL")
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.first().map(|r| r.count).unwrap_or(0), 1);

    // resolved_at defaults to NONE.
    let mut resp2 = db
        .query("SELECT count() FROM lint_finding WHERE resolved_at = NONE GROUP ALL")
        .await
        .unwrap();
    let rows2: Vec<Row> = resp2.take(0).unwrap();
    assert_eq!(rows2.first().map(|r| r.count).unwrap_or(0), 1);
}
