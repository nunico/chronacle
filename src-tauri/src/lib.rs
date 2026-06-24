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
/// Both `llm_provider` and `embedding_provider` are behind an `RwLock` so they
/// can be swapped at runtime — e.g. when the embedding model finishes downloading
/// or the user changes LLM settings — without restarting the app.
pub struct AppState {
    pub db: surrealdb::Surreal<surrealdb::engine::local::Db>,
    pub llm_provider: RwLock<Arc<dyn LlmProvider>>,
    pub vector_store: Arc<dyn providers::vector_store::VectorStore>,
    pub blob_store: Arc<dyn providers::blob_store::BlobStore>,
    pub embedding_provider: RwLock<Arc<dyn providers::embedding::EmbeddingProvider>>,
    pub pdf_extractor: Arc<dyn services::pdf_extractor::PdfExtractor>,
    /// Abort handle for the in-flight chat task, if any (see `chat_cancel`).
    pub chat_task: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
    /// Abort handle for the in-flight extraction task, if any (see `cancel_extraction`).
    pub extract_task: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
}

/// Locate the bundled pdfium dynamic library.
///
/// In `cargo tauri dev` the binary lives under `target/`, so we resolve via
/// `CARGO_MANIFEST_DIR` (set at compile time). In a bundled app the dylib is
/// shipped alongside the executable in the platform-specific resource path.
fn pdfium_library_path() -> std::path::PathBuf {
    let name = if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else if cfg!(target_os = "linux") {
        "libpdfium.so"
    } else {
        "pdfium.dll"
    };
    // Dev: <manifest>/resources/pdfium/<lib>
    let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/pdfium")
        .join(name);
    if dev.exists() {
        return dev;
    }
    // Bundled app: try <exe-dir>/../Resources/resources/pdfium/<lib> on mac,
    // <exe-dir>/resources/pdfium/<lib> elsewhere.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let mac_resources = exe_dir.join("../Resources/resources/pdfium").join(name);
            if mac_resources.exists() {
                return mac_resources;
            }
            let other = exe_dir.join("resources/pdfium").join(name);
            if other.exists() {
                return other;
            }
        }
    }
    // Last resort: return the dev path (extraction will fail with a clear
    // LibLoad error if the file is genuinely missing).
    dev
}

/// Determines the application data directory, creating it if needed.
fn app_data_dir() -> std::path::PathBuf {
    let dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dev.tea-driven.chronacle.desktop");

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
    let vector_store: Arc<dyn providers::vector_store::VectorStore> =
        Arc::new(providers::vector_store::SurrealDbVector::new(db.clone()));

    let pdfs_dir = data_dir.join("pdfs");
    if !pdfs_dir.exists() {
        std::fs::create_dir_all(&pdfs_dir).expect("Failed to create PDFs directory");
    }

    let blob_store: Arc<dyn providers::blob_store::BlobStore> =
        Arc::new(providers::blob_store::LocalFileStore::new(pdfs_dir));

    // Initialise fastembed only when the model is ALREADY cached. fastembed's
    // `try_new` downloads the model synchronously on a cache miss, which would
    // block the main thread — and therefore the Tauri window — for the entire
    // (hundreds-of-MB) download. When uncached, start with the mock placeholder
    // and let the UI's download screen drive `download_embedding_model`, which
    // hot-swaps the real provider in when it completes.
    let embedding_cache_dir = providers::embedding::FastEmbedProvider::cache_dir(&data_dir);
    let embedding_provider: Arc<dyn providers::embedding::EmbeddingProvider> =
        if providers::embedding::FastEmbedProvider::is_cached(&embedding_cache_dir) {
            match providers::embedding::FastEmbedProvider::try_new(Some(&embedding_cache_dir)) {
                Ok(p) => {
                    eprintln!(
                        "Embedding model '{}' ready ({} dim)",
                        p.model_name(),
                        p.dimension()
                    );
                    Arc::new(p)
                }
                Err(e) => {
                    eprintln!(
                        "Embedding model cached but failed to load ({e}) — \
                         starting with mock placeholder."
                    );
                    Arc::new(providers::embedding::MockEmbeddingProvider::new(768))
                }
            }
        } else {
            eprintln!(
                "Embedding model not cached — starting with mock placeholder. \
                 Use the download screen to download the model."
            );
            Arc::new(providers::embedding::MockEmbeddingProvider::new(768))
        };

    // Construct the LLM provider from persisted settings, falling back to
    // OpenAI (no-op if no API key is configured).
    let llm_provider = build_llm_provider_from_db(&db).await;
    let provider_name = provider_type_name(&llm_provider);
    eprintln!("LLM provider '{}' initialised", provider_name);

    let pdf_extractor: Arc<dyn services::pdf_extractor::PdfExtractor> = Arc::new(
        services::pdf_extractor::PdfiumExtractor::new(pdfium_library_path()),
    );

    let state = Arc::new(AppState {
        db,
        llm_provider: RwLock::new(llm_provider),
        vector_store,
        blob_store,
        embedding_provider: RwLock::new(embedding_provider),
        pdf_extractor,
        chat_task: tokio::sync::Mutex::new(None),
        extract_task: tokio::sync::Mutex::new(None),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state.clone())
        .setup(move |app| {
            // ADR-003: warn if any indexed sources were embedded with a different
            // model than the active embedding provider. Mock provider is treated
            // as "no active model" (pre-download placeholder, not a real
            // mismatch) and skipped here.
            let app_handle = app.handle().clone();
            let state_for_check = state.clone();
            tokio::spawn(async move {
                let active = match state_for_check.embedding_provider.read() {
                    Ok(p) => p.model_name().to_string(),
                    Err(_) => return,
                };
                if active == "mock" {
                    return;
                }
                match providers::embedding::check_embedding_model_consistency(
                    &state_for_check.db,
                    &active,
                )
                .await
                {
                    Ok(report) if !report.is_clean() => {
                        eprintln!(
                            "Embedding model mismatch detected: active={}, {} stale source(s) across {} model(s)",
                            report.active_model,
                            report.total_stale_sources(),
                            report.stale.len()
                        );
                        let _ = tauri::Emitter::emit(
                            &app_handle,
                            "embedding-model-mismatch",
                            &report,
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Embedding model mismatch check failed: {e}");
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_setting,
            commands::upload_source,
            commands::get_sources,
            commands::delete_source,
            commands::chat_send,
            commands::chat_cancel,
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
            commands::get_collections,
            commands::create_collection,
            commands::update_collection,
            commands::delete_collection,
            commands::add_campaign_collection,
            commands::remove_campaign_collection,
            commands::get_campaign_collections,
            commands::get_campaigns,
            commands::get_campaign,
            commands::create_campaign,
            commands::update_campaign,
            commands::delete_campaign,
            commands::check_embedding_model,
            commands::get_embedding_model_mismatch,
            commands::download_embedding_model,
            commands::reindex_all_sources,
            commands::get_chunk_for_citation,
            commands::get_entities,
            commands::get_entity_counts,
            commands::get_entity,
            commands::create_entity,
            commands::update_entity,
            commands::delete_entity,
            commands::relate_entities,
            commands::get_events_timeline,
            commands::get_entity_graph,
            commands::get_entity_relations,
            commands::create_session,
            commands::get_sessions,
            commands::get_session,
            commands::update_session,
            commands::delete_session,
            commands::get_session_entities,
            commands::extract_entity_by_name,
            commands::extract_all_from_campaign,
            commands::cancel_extraction,
            commands::resync_wikilinks,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");
}

/// Read LLM settings from the database and construct the correct provider.
async fn build_llm_provider_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Arc<dyn LlmProvider> {
    let settings = match services::settings_service::get_all(db).await {
        Ok(s) => s
            .into_iter()
            .map(|s| (s.key, s.value))
            .collect::<HashMap<_, _>>(),
        Err(_) => HashMap::new(),
    };

    build_llm_provider_from_map(&settings, Some(db)).await
}

pub(crate) async fn build_llm_provider_from_map(
    settings: &HashMap<String, String>,
    db: Option<&surrealdb::Surreal<surrealdb::engine::local::Db>>,
) -> Arc<dyn LlmProvider> {
    let provider = settings
        .get("llm_provider")
        .map(|s| s.as_str())
        .unwrap_or("openai");
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

    let cp = providers
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Custom provider '{name}' not found"))?;

    match cp.provider_type.as_str() {
        "openai" => Ok(Arc::new(OpenAIProvider::with_base_url(
            cp.api_key,
            model.to_string(),
            cp.base_url,
        ))),
        "anthropic" => Ok(Arc::new(AnthropicProvider::with_base_url(
            cp.api_key,
            model.to_string(),
            cp.base_url,
        ))),
        _ => Err(format!("Unknown provider type: {}", cp.provider_type)),
    }
}

/// Return a short human-readable provider type name.
pub(crate) fn provider_type_name(provider: &Arc<dyn LlmProvider>) -> &'static str {
    provider.provider_type()
}
