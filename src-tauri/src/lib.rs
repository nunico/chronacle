use std::sync::Arc;

mod commands;
pub mod providers;
pub mod schema;
pub mod services;

/// Shared application state managed by Tauri.
///
/// `DbBackend` is `surrealdb::engine::local::Db` (the local SurrealDB
/// connection type that works with both RocksDB and in-memory engines).
pub struct AppState {
    pub db: surrealdb::Surreal<surrealdb::engine::local::Db>,
    pub llm_provider: Arc<dyn providers::llm_provider::LlmProvider>,
    pub vector_store: Arc<dyn providers::vector_store::VectorStore>,
    pub blob_store: Arc<dyn providers::blob_store::BlobStore>,
}

/// Determines the application data directory, creating it if needed.
fn app_data_dir() -> std::path::PathBuf {
    let dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("com.chronacle.app");

    if !dir.exists() {
        std::fs::create_dir_all(&dir).expect("Failed to create app data directory");
    }

    dir
}

/// Entry point that wires up all dependencies and starts the Tauri application.
///
/// 1. Initialises an embedded SurrealDB (RocksDB) in the app data directory.
/// 2. Runs schema migrations from the `.surql` files.
/// 3. Constructs the service layer with trait-object dependencies.
/// 4. Registers IPC command handlers and starts the Tauri event loop.
#[tokio::main]
pub async fn run() {
    let data_dir = app_data_dir();
    let db_path = data_dir.join("chronacle.db");

    let db = surrealdb::Surreal::new::<surrealdb::engine::local::RocksDb>(db_path)
        .await
        .expect("Failed to initialise SurrealDB (RocksDB)");

    db.use_ns("chronacle")
        .use_db("chronacle")
        .await
        .expect("Failed to select namespace / database");

    // Run schema migrations
    schema::run_migrations(&db)
        .await
        .expect("Failed to run schema migrations");

    // ── Build service dependencies ──────────────────────────────────
    let llm_provider: Arc<dyn providers::llm_provider::LlmProvider> = Arc::new(
        providers::llm_provider::OpenAIProvider::new(
            String::new(),
            String::new(),
        ),
    );

    let vector_store: Arc<dyn providers::vector_store::VectorStore> = Arc::new(
        providers::vector_store::SurrealDbVector::new(db.clone()),
    );

    let pdfs_dir = data_dir.join("pdfs");
    if !pdfs_dir.exists() {
        std::fs::create_dir_all(&pdfs_dir).expect("Failed to create PDFs directory");
    }

    let blob_store: Arc<dyn providers::blob_store::BlobStore> = Arc::new(
        providers::blob_store::LocalFileStore::new(pdfs_dir),
    );

    let state = Arc::new(AppState {
        db,
        llm_provider,
        vector_store,
        blob_store,
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_setting,
            commands::upload_source,
            commands::chat_send,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");
}
