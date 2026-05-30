use std::collections::HashMap;
use std::sync::{Arc, RwLock};

mod commands;
pub mod providers;
pub mod schema;
pub mod services;

use providers::embedding::EmbeddingProvider;
use providers::llm_provider::{AnthropicProvider, LlmProvider, OllamaProvider, OpenAIProvider};

/// Shared application state managed by Tauri.
///
/// `DbBackend` is `surrealdb::engine::local::Db` (the local SurrealDB
/// connection type that works with both RocksDB and in-memory engines).
///
/// `llm_provider` is behind a `RwLock` so the settings page can swap
/// providers at runtime without restarting the app.
pub struct AppState {
    pub db: surrealdb::Surreal<surrealdb::engine::local::Db>,
    pub llm_provider: RwLock<Arc<dyn LlmProvider>>,
    pub vector_store: Arc<dyn providers::vector_store::VectorStore>,
    pub blob_store: Arc<dyn providers::blob_store::BlobStore>,
    pub embedding_provider: Arc<dyn providers::embedding::EmbeddingProvider>,
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

    // Try to initialize fastembed; fall back to a mock if model not cached.
    let embedding_provider: Arc<dyn providers::embedding::EmbeddingProvider> =
        match providers::embedding::FastEmbedProvider::try_new() {
            Ok(p) => {
                eprintln!("Embedding model '{}' ready ({} dim)", p.model_name(), p.dimension());
                Arc::new(p)
            }
            Err(e) => {
                eprintln!(
                    "Warning: Embedding model not available — {}. \
                     Chat and PDF ingestion will fail until the model \
                     is downloaded on first use.",
                    e
                );
                Arc::new(providers::embedding::MockEmbeddingProvider::new(768))
            }
        };

    // Construct the LLM provider from persisted settings, falling back to
    // OpenAI (no-op if no API key is configured).
    let llm_provider = build_llm_provider_from_db(&db).await;
    let provider_name = provider_type_name(&llm_provider);
    eprintln!("LLM provider '{}' initialised", provider_name);

    let state = Arc::new(AppState {
        db,
        llm_provider: RwLock::new(llm_provider),
        vector_store,
        blob_store,
        embedding_provider,
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_setting,
            commands::upload_source,
            commands::chat_send,
            commands::get_chat_history,
            commands::reconfigure_llm_provider,
            commands::get_llm_provider_status,
            commands::get_custom_providers,
            commands::create_custom_provider,
            commands::update_custom_provider,
            commands::delete_custom_provider,
            commands::get_provider_models,
            commands::add_provider_model,
            commands::remove_provider_model,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");
}

/// Read LLM settings from the database and construct the correct provider.
async fn build_llm_provider_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Arc<dyn LlmProvider> {
    let settings = match services::settings_service::get_all(db).await {
        Ok(s) => s.into_iter().map(|s| (s.key, s.value)).collect::<HashMap<_, _>>(),
        Err(_) => HashMap::new(),
    };

    build_llm_provider_from_map(&settings, Some(db)).await
}

pub(crate) async fn build_llm_provider_from_map(
    settings: &HashMap<String, String>,
    db: Option<&surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> Arc<dyn LlmProvider> {
    let provider = settings.get("llm_provider").map(|s| s.as_str()).unwrap_or("openai");
    let api_key = settings.get("llm_api_key").cloned().unwrap_or_default();
    let model = settings.get("llm_model").cloned().unwrap_or_default();
    let base_url = settings.get("llm_base_url").cloned().unwrap_or_default();

    // Check for custom provider prefix ("custom:ProviderName")
    if let Some(custom_name) = provider.strip_prefix("custom:") {
        if let Some(db) = db {
            match build_custom_provider(db, custom_name, &model).await {
                Ok(p) => return p,
                Err(e) => {
                    eprintln!("Warning: custom provider '{custom_name}' not found ({e}), falling back to OpenAI");
                }
            }
        }
    }

    match provider {
        "anthropic" => Arc::new(AnthropicProvider::with_base_url(api_key, model, base_url)),
        "ollama" => Arc::new(OllamaProvider::new(base_url, model)),
        _ => Arc::new(OpenAIProvider::new(api_key, model)),
    }
}

async fn build_custom_provider(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
    name: &str,
    model: &str,
) -> Result<Arc<dyn LlmProvider>, String> {
    let providers = crate::services::custom_provider_service::get_all(db).await?;

    let cp = providers.into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Custom provider '{name}' not found"))?;

    match cp.provider_type.as_str() {
        "openai" => Ok(Arc::new(OpenAIProvider::with_base_url(
            cp.api_key, model.to_string(), cp.base_url,
        ))),
        "anthropic" => Ok(Arc::new(AnthropicProvider::with_base_url(
            cp.api_key, model.to_string(), cp.base_url,
        ))),
        _ => Err(format!("Unknown provider type: {}", cp.provider_type)),
    }
}

/// Return a short human-readable provider type name.
pub(crate) fn provider_type_name(provider: &Arc<dyn LlmProvider>) -> &'static str {
    provider.provider_type()
}
