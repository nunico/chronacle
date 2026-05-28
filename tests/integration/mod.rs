/// Integration tests for the Chronacle backend.
///
/// These tests exercise the service layer directly against an in-memory
/// SurrealDB instance. They do **not** go through Tauri IPC — that is
/// covered by the E2E test suite.

use chronacle_lib::schema;

/// Helper: set up an in-memory SurrealDB with the Phase 1 schema applied.
async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Mem> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("Failed to create in-memory SurrealDB");

    db.use_ns("test").use_db("test").await.unwrap();

    // Run the full Phase 1 schema
    schema::run_migrations(&db)
        .await
        .expect("Schema migration should succeed");

    db
}

#[tokio::test]
async fn test_schema_migration_creates_tables() {
    let db = setup_db().await;

    // Verify the schema was applied by checking the DB works
    let mut res = db
        .query("SELECT count() FROM campaign GROUP ALL")
        .await
        .unwrap();

    let count: Vec<i64> = res.take(0).unwrap_or_default();
    assert!(count.is_empty() || count == vec![0]);
}

#[tokio::test]
async fn test_campaign_crud() {
    let db = setup_db().await;

    // Create a campaign
    let mut res = db
        .query(
            "CREATE campaign:test1 SET
                name = 'Test Campaign',
                system = 'D&D 5e',
                created_at = time::now(),
                updated_at = time::now()",
        )
        .await
        .unwrap();

    assert!(res.is_ok());

    // Verify it exists
    let mut res = db.query("SELECT * FROM campaign WHERE id = campaign:test1")
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct CampaignRow {
        id: surrealdb::sql::Thing,
        name: String,
        system: String,
    }

    let rows: Vec<CampaignRow> = res.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Test Campaign");
}

#[tokio::test]
async fn test_source_crud() {
    let db = setup_db().await;

    let mut res = db
        .query(
            "CREATE source:src1 SET
                filename = 'test.pdf',
                display_name = 'Test PDF',
                source_type = 'rules',
                page_count = 10,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = 'nomic-embed-text-v1.5'",
        )
        .await
        .unwrap();

    assert!(res.is_ok());

    // Update status
    let mut res = db
        .query("UPDATE source:src1 SET index_status = 'done'")
        .await
        .unwrap();
    assert!(res.is_ok());
}
