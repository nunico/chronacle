use super::*;

async fn seed_db_with_sources(models: &[&str]) -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("t").use_db("t").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE collection SET id='col1', name='Test', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    for (i, model) in models.iter().enumerate() {
        let q = format!(
            "CREATE source SET id='s{i}', filename='f{i}.pdf', display_name='F{i}', \
             source_type='rules', page_count=1, indexed_at=time::now(), index_status='done', \
             embed_model='{model}', collection=type::thing('collection','col1')"
        );
        db.query(q).await.unwrap().check().unwrap();
    }
    db
}

#[tokio::test]
async fn mismatch_check_returns_clean_when_all_sources_match() {
    let db = seed_db_with_sources(&["nomic-embed-text-v1.5", "nomic-embed-text-v1.5"]).await;
    let report = check_embedding_model_consistency(&db, "nomic-embed-text-v1.5")
        .await
        .unwrap();
    assert!(report.is_clean());
    assert_eq!(report.total_stale_sources(), 0);
    assert_eq!(report.active_model, "nomic-embed-text-v1.5");
}

#[tokio::test]
async fn mismatch_check_returns_clean_when_no_sources_indexed() {
    let db = seed_db_with_sources(&[]).await;
    let report = check_embedding_model_consistency(&db, "nomic-embed-text-v1.5")
        .await
        .unwrap();
    assert!(report.is_clean());
}

#[tokio::test]
async fn mismatch_check_lists_stale_models_with_counts() {
    let db = seed_db_with_sources(&[
        "nomic-embed-text-v1.5",
        "all-MiniLM-L6-v2",
        "all-MiniLM-L6-v2",
        "nomic-embed-text-v1",
    ])
    .await;
    let report = check_embedding_model_consistency(&db, "nomic-embed-text-v1.5")
        .await
        .unwrap();
    assert!(!report.is_clean());
    assert_eq!(report.total_stale_sources(), 3);
    // Stale entries are sorted by model name for deterministic UI display.
    assert_eq!(report.stale.len(), 2);
    assert_eq!(report.stale[0].embed_model, "all-MiniLM-L6-v2");
    assert_eq!(report.stale[0].source_count, 2);
    assert_eq!(report.stale[1].embed_model, "nomic-embed-text-v1");
    assert_eq!(report.stale[1].source_count, 1);
}
