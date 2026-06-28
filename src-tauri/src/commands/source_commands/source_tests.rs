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

// ── get_sources — collection_id filter ───────────────────────────────────

#[tokio::test]
async fn get_sources_filters_by_collection() {
    let db = setup_db().await;

    let col_a = crate::services::collection_service::create(&db, "Col A", None)
        .await
        .unwrap();
    let col_b = crate::services::collection_service::create(&db, "Col B", None)
        .await
        .unwrap();

    db.query(
        "CREATE source SET
             id = 'src_a',
             filename = 'a.pdf',
             display_name = 'Source A',
             source_type = 'rules',
             page_count = 0,
             indexed_at = time::now(),
             index_status = 'pending',
             embed_model = 'nomic-embed-text-v1.5',
             campaign = NULL,
             collection = type::thing('collection', $cid)",
    )
    .bind(("cid", col_a.id.clone()))
    .await
    .unwrap();

    db.query(
        "CREATE source SET
             id = 'src_b',
             filename = 'b.pdf',
             display_name = 'Source B',
             source_type = 'rules',
             page_count = 0,
             indexed_at = time::now(),
             index_status = 'pending',
             embed_model = 'nomic-embed-text-v1.5',
             campaign = NULL,
             collection = type::thing('collection', $cid)",
    )
    .bind(("cid", col_b.id.clone()))
    .await
    .unwrap();

    let mut resp_a = db
        .query(
            "SELECT * FROM source \
             WHERE collection = type::thing('collection', $cid) \
             ORDER BY display_name ASC",
        )
        .bind(("cid", col_a.id.clone()))
        .await
        .unwrap();

    #[derive(Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let rows_a: Vec<Row> = resp_a.take(0).unwrap();
    assert_eq!(rows_a.len(), 1);
    assert_eq!(rows_a[0].id.id.to_raw(), "src_a");

    let mut resp_all = db
        .query("SELECT * FROM source ORDER BY display_name ASC")
        .await
        .unwrap();
    let rows_all: Vec<Row> = resp_all.take(0).unwrap();
    assert_eq!(rows_all.len(), 2);
}
