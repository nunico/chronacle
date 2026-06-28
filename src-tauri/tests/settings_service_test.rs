use chronacle_lib::services::settings_service::{get_all, upsert};
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

// ── Test 1 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn upsert_and_get_roundtrip() {
    let db = setup_db().await;

    upsert(&db, "llm_provider", "openai").await.unwrap();
    let settings = get_all(&db).await.unwrap();

    assert!(
        settings
            .iter()
            .any(|s| s.key == "llm_provider" && s.value == "openai"),
        "expected to find 'llm_provider' = 'openai' in settings after upsert"
    );
}

// ── Test 2 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn upsert_overwrites_existing_value() {
    let db = setup_db().await;

    // First upsert
    upsert(&db, "llm_model", "gpt-4o").await.unwrap();
    let settings = get_all(&db).await.unwrap();
    assert!(
        settings
            .iter()
            .any(|s| s.key == "llm_model" && s.value == "gpt-4o"),
        "expected 'llm_model' = 'gpt-4o' after first upsert"
    );

    // Second upsert should overwrite
    upsert(&db, "llm_model", "claude-sonnet-4-6").await.unwrap();
    let settings = get_all(&db).await.unwrap();

    let llm_model_entries: Vec<_> = settings.iter().filter(|s| s.key == "llm_model").collect();
    assert_eq!(
        llm_model_entries.len(),
        1,
        "expected exactly 1 entry for 'llm_model', got {}",
        llm_model_entries.len()
    );
    assert_eq!(
        llm_model_entries[0].value, "claude-sonnet-4-6",
        "expected 'llm_model' to be updated to 'claude-sonnet-4-6'"
    );
}

// ── Test 3 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_all_returns_all_upserted_keys() {
    let db = setup_db().await;

    upsert(&db, "llm_provider", "openai").await.unwrap();
    upsert(&db, "llm_model", "gpt-4o").await.unwrap();
    upsert(&db, "embedding_backend", "local").await.unwrap();

    let settings = get_all(&db).await.unwrap();

    assert!(
        settings
            .iter()
            .any(|s| s.key == "llm_provider" && s.value == "openai"),
        "expected 'llm_provider' in settings"
    );
    assert!(
        settings
            .iter()
            .any(|s| s.key == "llm_model" && s.value == "gpt-4o"),
        "expected 'llm_model' in settings"
    );
    assert!(
        settings
            .iter()
            .any(|s| s.key == "embedding_backend" && s.value == "local"),
        "expected 'embedding_backend' in settings"
    );
}

// ── Test 4 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_all_on_empty_db_returns_no_user_keys() {
    let db = setup_db().await;

    let settings = get_all(&db).await.unwrap();

    assert!(
        !settings.iter().any(|s| s.key == "llm_provider"),
        "expected no 'llm_provider' key on fresh DB"
    );
}
