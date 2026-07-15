use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub mod commands;
pub mod services;

use chronacle_providers::embedding::EmbeddingProvider;
use chronacle_providers::llm_provider::{
    AnthropicProvider, LlmProvider, OllamaProvider, OpenAIProvider,
};

/// Shared application state managed by Tauri.
///
/// Both `llm_provider` and `embedding_provider` are behind an `RwLock` so they
/// can be swapped at runtime — e.g. when the embedding model finishes downloading
/// or the user changes LLM settings — without restarting the app.
pub struct AppState {
    pub db: surrealdb::Surreal<surrealdb::engine::any::Any>,
    pub llm_provider: RwLock<Arc<dyn LlmProvider>>,
    pub vector_store: Arc<dyn chronacle_providers::vector_store::VectorStore>,
    pub blob_store: Arc<dyn chronacle_providers::blob_store::BlobStore>,
    pub embedding_provider: RwLock<Arc<dyn chronacle_providers::embedding::EmbeddingProvider>>,
    pub pdf_extractor: Arc<dyn chronacle_ingestion::pdf_extractor::PdfExtractor>,
    /// Abort handle for the in-flight chat task, if any (see `chat_cancel`).
    pub chat_task: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
    /// Abort handle for the in-flight extraction task, if any (see `cancel_extraction`).
    pub extract_task: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
    /// Abort handle for the in-flight codex compile task, if any (see `cancel_compile`).
    pub compile_task: tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
    /// Vault sync engine, write guard, and watcher task, wired together.
    /// `None` until `vault_sync_path` is configured.
    pub vault: tokio::sync::RwLock<Option<VaultRuntime>>,
    /// Producer handle the five record producers enqueue onto. Falls back to
    /// `NoopOutbound` whenever no vault is configured — producers never branch
    /// on `Option`, see `chronacle_core::VaultOutbound`.
    pub outbound: tokio::sync::RwLock<Arc<dyn chronacle_core::VaultOutbound>>,
}

/// Everything a live vault configuration owns. Replaced wholesale on
/// `set_vault_path`; the watcher and drain tasks are aborted when dropped
/// out, so neither can act against a root that is no longer current.
pub struct VaultRuntime {
    pub svc: Arc<chronacle_vault::reconcile::VaultSyncService>,
    pub pending: Arc<chronacle_vault::outbound::PendingWrites>,
    pub watcher_task: Option<tauri::async_runtime::JoinHandle<()>>,
    /// The outbound drain loop's task handle, so a vault-path switch can
    /// abort it — the drain has no other stop signal short of dropping its
    /// producer, which only closes the channel once queued work drains.
    pub outbound_task: Option<tauri::async_runtime::JoinHandle<()>>,
    /// The root this runtime is bound to. Kept so a failed `set_vault_path`
    /// can respawn the exact same watcher/drain over the previous runtime
    /// instead of leaving the app with none.
    pub root: String,
}

/// Construct the vault sync engine and its shared write guard.
fn build_vault_service(
    db: surrealdb::Surreal<surrealdb::engine::any::Any>,
    root: &str,
) -> (
    Arc<chronacle_vault::reconcile::VaultSyncService>,
    Arc<chronacle_vault::outbound::PendingWrites>,
) {
    let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
    let svc = Arc::new(chronacle_vault::reconcile::VaultSyncService::new(
        Arc::new(chronacle_providers::vault_store::LocalFsVaultStore::new(
            root,
        )),
        Arc::new(chronacle_domain::vault_record_store::SurrealVaultRecordStore::new(db)),
        Arc::clone(&pending),
    ));
    (svc, pending)
}

/// Whether a batch of watcher events contains anything the consumer loop
/// should act on, i.e. anything that is not Chronacle's own write or delete.
///
/// Extracted out of `spawn_watcher`'s loop body so it is directly unit-
/// testable (`spawn_watcher` itself is only reachable end-to-end, since it
/// owns a real `NotifyWatcher`) — a future change to the relevance rule
/// cannot silently drift from what the integration tests exercise.
///
/// A `Remove` event is checked against `is_own_delete`, not treated as
/// automatically relevant: reconcile's own orphan sweep and evaporated-
/// conflict cleanup delete files/sidecars too, and a sidecar deletion is
/// separately the GM's conflict-resolution signal — conflating "we deleted
/// it" with "the GM deleted it" would misread our own cleanup as GM intent.
///
/// `is_own_delete` is consuming (Finding 1: the delete guard is one-shot),
/// so this must be called at most once per batch, and each event in the
/// batch checked at most once — exactly what the single pass below does.
pub(crate) async fn batch_is_relevant(
    batch: &[chronacle_core::VaultEvent],
    svc: &chronacle_vault::reconcile::VaultSyncService,
) -> bool {
    let mut relevant = false;
    for ev in batch {
        match ev {
            chronacle_core::VaultEvent::Upsert(key) => {
                if !svc.is_own_write(key).await {
                    relevant = true;
                }
            }
            chronacle_core::VaultEvent::Remove(key) => {
                if !svc.is_own_delete(key) {
                    relevant = true;
                }
            }
            chronacle_core::VaultEvent::Rescan => relevant = true,
        }
    }
    relevant
}

/// Consume watcher events: drop our own writes and deletes, trigger one
/// reconcile per surviving batch (single in-flight by construction — this
/// loop is the only caller and awaits the reconcile), re-embed applied
/// entities.
pub(crate) fn spawn_watcher(
    state: Arc<AppState>,
    svc: Arc<chronacle_vault::reconcile::VaultSyncService>,
    root: String,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let watcher = chronacle_providers::vault_watcher::NotifyWatcher::new(&root);
        let mut rx = chronacle_core::VaultWatcher::subscribe(&watcher).await;
        while let Some(first) = rx.recv().await {
            let mut batch = vec![first];
            while let Ok(next) = rx.try_recv() {
                batch.push(next);
            }
            if !batch_is_relevant(&batch, &svc).await {
                continue;
            }
            match svc.reconcile().await {
                Ok(report) => {
                    crate::commands::vault_commands::embed_applied_refs(
                        &state,
                        &report.applied_refs,
                    )
                    .await;
                }
                Err(e) => eprintln!("vault: watcher-triggered reconcile failed: {e}"),
            }
        }
    })
}

/// Build a fresh outbound queue producer and spawn its drain loop against
/// `svc`, returning the drain's task handle alongside it so a vault-path
/// switch can abort it outright — dropping the producer alone only closes
/// the channel, and the drain still finishes whatever was already queued
/// against the OLD service before it notices (Finding 4, tranche-5
/// whole-branch review).
pub(crate) fn spawn_outbound(
    svc: Arc<chronacle_vault::reconcile::VaultSyncService>,
) -> (
    Arc<dyn chronacle_core::VaultOutbound>,
    tauri::async_runtime::JoinHandle<()>,
) {
    let (producer, rx) = chronacle_vault::outbound::QueueOutbound::new();
    let task = tauri::async_runtime::spawn(chronacle_vault::outbound::drain_loop(rx, svc));
    (Arc::new(producer), task)
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

/// Initialise the embedded SurrealDB (RocksDB), select namespace/database,
/// and run schema migrations.
///
/// Returns the canonical app data directory and the database handle. Callers
/// may use the returned `(data_dir, db)` tuple directly without recomputing
/// paths.
async fn init_database() -> (
    std::path::PathBuf,
    surrealdb::Surreal<surrealdb::engine::any::Any>,
) {
    let data_dir = app_data_dir();
    let db_path = data_dir.join("chronacle.db");

    let db = surrealdb::engine::any::connect(format!("rocksdb://{}", db_path.display()))
        .await
        .expect("Failed to initialise SurrealDB (RocksDB)");

    db.use_ns("chronacle")
        .use_db("chronacle")
        .await
        .expect("Failed to select namespace / database");

    chronacle_db::run_migrations(&db)
        .await
        .expect("Failed to run schema migrations");

    (data_dir, db)
}

/// Entry point that wires up all dependencies and starts the Tauri application.
///
/// 1. Initialises an embedded SurrealDB (RocksDB) in the app data directory.
/// 2. Runs schema migrations from the `.surql` files.
/// 3. Constructs the service layer with trait-object dependencies.
/// 4. Registers IPC command handlers and starts the Tauri event loop.
#[tokio::main]
pub async fn run() {
    let (data_dir, db) = init_database().await;

    // ── Build service dependencies ──────────────────────────────────
    let vector_store: Arc<dyn chronacle_providers::vector_store::VectorStore> = Arc::new(
        chronacle_providers::vector_store::SurrealDbVector::new(db.clone()),
    );

    let pdfs_dir = data_dir.join("pdfs");
    if !pdfs_dir.exists() {
        std::fs::create_dir_all(&pdfs_dir).expect("Failed to create PDFs directory");
    }

    let blob_store: Arc<dyn chronacle_providers::blob_store::BlobStore> = Arc::new(
        chronacle_providers::blob_store::LocalFileStore::new(pdfs_dir),
    );

    // Select the embedding backend from settings (local fastembed vs OpenAI
    // cloud). See `build_embedding_provider_from_map`.
    let embedding_provider = build_embedding_provider_from_db(&db, &data_dir).await;

    // Construct the LLM provider from persisted settings, falling back to
    // OpenAI (no-op if no API key is configured).
    let llm_provider = build_llm_provider_from_db(&db).await;
    let provider_name = provider_type_name(&llm_provider);
    eprintln!("LLM provider '{}' initialised", provider_name);

    let pdf_extractor: Arc<dyn chronacle_ingestion::pdf_extractor::PdfExtractor> = Arc::new(
        chronacle_ingestion::pdf_extractor::PdfiumExtractor::new(pdfium_library_path()),
    );

    // Build the vault sync engine now if a vault root is already configured,
    // reading the setting before `db` is moved into `AppState` below. The
    // watcher task is spawned later, inside `.setup()`, once `Arc<AppState>`
    // exists — `spawn_watcher` needs it to re-embed applied entities.
    let vault_settings = read_settings_map(&db).await;
    let vault_root = vault_settings
        .get("vault_sync_path")
        .filter(|p| !p.is_empty())
        .cloned();
    let vault_svc_and_pending = vault_root
        .as_deref()
        .map(|path| build_vault_service(db.clone(), path));
    let (outbound, startup_outbound_task): (
        Arc<dyn chronacle_core::VaultOutbound>,
        Option<tauri::async_runtime::JoinHandle<()>>,
    ) = match &vault_svc_and_pending {
        Some((svc, _)) => {
            let (out, task) = spawn_outbound(Arc::clone(svc));
            (out, Some(task))
        }
        None => (Arc::new(chronacle_core::NoopOutbound), None),
    };
    // Captured by value here, BEFORE `vault_svc_and_pending` is consumed
    // below — this is the `original_svc` the startup watcher-spawn task (in
    // `.setup()`) compares against via `Arc::ptr_eq` to detect whether
    // `set_vault_path` already replaced the vault while it was waiting.
    let startup_vault_svc = vault_svc_and_pending
        .as_ref()
        .map(|(svc, _)| Arc::clone(svc));
    let startup_vault_root = vault_root.clone().unwrap_or_default();
    let vault = vault_svc_and_pending.map(|(svc, pending)| VaultRuntime {
        svc,
        pending,
        watcher_task: None, // spawned below, once `state` exists
        outbound_task: startup_outbound_task,
        root: startup_vault_root,
    });

    let state = Arc::new(AppState {
        db,
        llm_provider: RwLock::new(llm_provider),
        vector_store,
        blob_store,
        embedding_provider: RwLock::new(embedding_provider),
        pdf_extractor,
        chat_task: tokio::sync::Mutex::new(None),
        extract_task: tokio::sync::Mutex::new(None),
        compile_task: tokio::sync::Mutex::new(None),
        vault: tokio::sync::RwLock::new(vault),
        outbound: tokio::sync::RwLock::new(outbound),
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
                match chronacle_providers::embedding::check_embedding_model_consistency(
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

            // Spawn the vault watcher now that `Arc<AppState>` exists —
            // `spawn_watcher` needs it to re-embed applied entities after a
            // watcher-triggered reconcile.
            //
            // Capture the startup service BY VALUE before spawning: if
            // `set_vault_path` runs and replaces `state.vault` before this
            // task acquires the write lock below, this task must not clobber
            // the newer, legitimate watcher with one pointed at the stale
            // startup root. `Arc::ptr_eq` against the just-captured `Arc`
            // (not a fresh read through the lock) is what lets the task tell
            // "still installed" from "already replaced".
            if let (Some(root), Some(original_svc)) =
                (vault_root.clone(), startup_vault_svc.clone())
            {
                let state_for_watcher = state.clone();
                tokio::spawn(async move {
                    let watcher_svc = Arc::clone(&original_svc);
                    let task = spawn_watcher(state_for_watcher.clone(), watcher_svc, root);
                    let mut guard = state_for_watcher.vault.write().await;
                    let Some(rt) = guard.as_mut() else {
                        // Vault was cleared before this task ran; the watcher
                        // we just spawned has nothing to attach to. Abort it.
                        task.abort();
                        return;
                    };
                    if !Arc::ptr_eq(&rt.svc, &original_svc) {
                        // Someone already replaced the vault while we were
                        // waiting for the lock (or for the watcher to spawn).
                        // Their watcher is authoritative; ours would be wired
                        // to the stale startup service/root, so drop it
                        // instead of overwriting (and orphaning) theirs.
                        task.abort();
                        return;
                    }
                    rt.watcher_task = Some(task);
                    drop(guard);

                    // Reconcile once at startup, AFTER the watcher is live.
                    //
                    // The watcher only reports what happens from now on, so
                    // without this pass every edit the GM made in their vault
                    // while Chronacle was closed would be invisible until they
                    // happened to touch something else or hit "Sync now" — and
                    // any DB change made while the vault was disconnected would
                    // never be exported. Reconcile is the correctness
                    // guarantee; it has to actually run at least once per boot.
                    //
                    // Ordering matters: spawning the watcher first means an
                    // edit landing *during* this pass still produces an event,
                    // so it is picked up by the next reconcile rather than
                    // being missed in the gap.
                    match original_svc.reconcile().await {
                        Ok(report) => {
                            crate::commands::vault_commands::embed_applied_refs(
                                &state_for_watcher,
                                &report.applied_refs,
                            )
                            .await;
                        }
                        Err(e) => eprintln!("vault: startup reconcile failed: {e}"),
                    }
                });
            }
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
            commands::get_embedding_provider_status,
            commands::reconfigure_embedding_provider,
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
            commands::soft_delete_entity,
            commands::relate_entities,
            commands::merge_entities,
            commands::get_events_timeline,
            commands::get_entity_graph,
            commands::get_entity_relations,
            commands::delete_relation,
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
            commands::compile_collection,
            commands::compile_entity,
            commands::get_codex_status,
            commands::cancel_compile,
            commands::get_rule_entries,
            commands::update_rule_notes,
            commands::redo_rule_entry,
            commands::save_chat_to_codex,
            commands::get_proposals,
            commands::accept_proposal,
            commands::reject_proposal,
            commands::get_maintenance_counts,
            commands::run_lint,
            commands::get_lint_findings,
            commands::resolve_lint_finding,
            commands::get_vault_path,
            commands::set_vault_path,
            commands::vault_sync_now,
            commands::list_vault_conflicts,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Tauri application");
}

/// Read all settings from the database into a flat map (empty on error).
pub(crate) async fn read_settings_map(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
) -> HashMap<String, String> {
    match services::settings_service::get_all(db).await {
        Ok(s) => s.into_iter().map(|s| (s.key, s.value)).collect(),
        Err(_) => HashMap::new(),
    }
}

/// Read LLM settings from the database and construct the correct provider.
async fn build_llm_provider_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
) -> Arc<dyn LlmProvider> {
    let settings = read_settings_map(db).await;
    build_llm_provider_from_map(&settings, Some(db)).await
}

/// Read settings from the database and construct the embedding provider.
pub(crate) async fn build_embedding_provider_from_db(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    data_dir: &std::path::Path,
) -> Arc<dyn chronacle_providers::embedding::EmbeddingProvider> {
    let settings = read_settings_map(db).await;
    build_embedding_provider_from_map(&settings, data_dir).await
}

/// Construct the embedding provider from settings.
///
/// `embedding_backend` selects between `local` (bundled fastembed /
/// `nomic-embed-text-v1.5`) and `openai`/`cloud` (an OpenAI-compatible
/// `/embeddings` endpoint at 768 dims). When the setting is absent, the default
/// is `local` where ONNX Runtime is bundled, and `openai` where it is not
/// (notably macOS x86_64) — so Intel Macs steer to the cloud automatically.
///
/// The local path mirrors the startup constraint: only construct fastembed when
/// the model is already cached; otherwise return the mock placeholder and let
/// the download screen provision it. fastembed's `try_new` would otherwise block
/// on a multi-hundred-MB download.
pub(crate) async fn build_embedding_provider_from_map(
    settings: &HashMap<String, String>,
    data_dir: &std::path::Path,
) -> Arc<dyn chronacle_providers::embedding::EmbeddingProvider> {
    use chronacle_providers::embedding::{
        local_embeddings_available, FastEmbedProvider, MockEmbeddingProvider,
        OpenAiEmbeddingProvider,
    };

    let default_backend = if local_embeddings_available() {
        "local"
    } else {
        "openai"
    };
    let backend = settings
        .get("embedding_backend")
        .map(|s| s.as_str())
        .unwrap_or(default_backend);

    match backend {
        "openai" | "cloud" => {
            let api_key = settings
                .get("embedding_api_key")
                .cloned()
                .unwrap_or_default();
            let model = settings.get("embedding_model").cloned().unwrap_or_default();
            let base_url = settings
                .get("embedding_base_url")
                .cloned()
                .unwrap_or_default();
            let provider = OpenAiEmbeddingProvider::new(api_key, model, base_url);
            eprintln!(
                "Embedding backend: OpenAI cloud ('{}', {} dim)",
                provider.model_name(),
                provider.dimension()
            );
            Arc::new(provider)
        }
        _ => {
            let cache_dir = FastEmbedProvider::cache_dir(data_dir);
            if FastEmbedProvider::is_cached(&cache_dir) {
                match FastEmbedProvider::try_new(Some(&cache_dir)) {
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
                             using mock placeholder."
                        );
                        Arc::new(MockEmbeddingProvider::new(768))
                    }
                }
            } else {
                eprintln!(
                    "Embedding model not cached — starting with mock placeholder. \
                     Use the download screen to download the model."
                );
                Arc::new(MockEmbeddingProvider::new(768))
            }
        }
    }
}

pub(crate) async fn build_llm_provider_from_map(
    settings: &HashMap<String, String>,
    db: Option<&surrealdb::Surreal<surrealdb::engine::any::Any>>,
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
        // `with_base_url` honors a configured `llm_base_url` (OpenAI-compatible /
        // self-hosted endpoints) and falls back to api.openai.com when empty.
        // `::new` would hardcode the OpenAI URL and silently ignore the setting.
        _ => Arc::new(OpenAIProvider::with_base_url(api_key, model, base_url)),
    }
}

async fn build_custom_provider(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    name: &str,
    model: &str,
) -> Result<Arc<dyn LlmProvider>, String> {
    let providers = chronacle_domain::custom_provider_service::get_all(db).await?;

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

#[cfg(test)]
mod watcher_relevance_tests {
    use super::*;
    use chronacle_core::VaultEvent;

    /// A bare `VaultSyncService` with an in-memory-backed store/records is
    /// overkill here — `batch_is_relevant` only needs `is_own_write` and
    /// `is_own_delete`, which are pure functions of the shared `PendingWrites`
    /// guard plus (for writes) whatever the store currently holds. Build the
    /// smallest real service that exercises both.
    fn svc_with(
        pending: Arc<chronacle_vault::outbound::PendingWrites>,
        content: Option<&'static str>,
    ) -> chronacle_vault::reconcile::VaultSyncService {
        let mut store = chronacle_core::MockVaultStore::new();
        store.expect_read().returning(move |_| {
            content
                .map(str::to_string)
                .ok_or_else(|| chronacle_core::VaultStoreError::NotFound("gone".into()))
        });
        let records = chronacle_core::MockVaultRecordStore::new();
        chronacle_vault::reconcile::VaultSyncService::new(
            Arc::new(store),
            Arc::new(records),
            pending,
        )
    }

    #[tokio::test]
    async fn a_rescan_event_is_always_relevant() {
        let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
        let svc = svc_with(Arc::clone(&pending), None);
        assert!(batch_is_relevant(&[VaultEvent::Rescan], &svc).await);
    }

    #[tokio::test]
    async fn an_upsert_matching_our_own_armed_write_is_not_relevant() {
        let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
        let content = "hello";
        let hash = chronacle_vault::render::content_hash(content);
        pending.arm("k.md", hash);
        let svc = svc_with(Arc::clone(&pending), Some(content));
        assert!(
            !batch_is_relevant(&[VaultEvent::Upsert("k.md".into())], &svc).await,
            "our own write must not trigger a reconcile"
        );
    }

    #[tokio::test]
    async fn an_upsert_with_no_matching_write_guard_is_relevant() {
        let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
        let svc = svc_with(Arc::clone(&pending), Some("a GM edit"));
        assert!(
            batch_is_relevant(&[VaultEvent::Upsert("k.md".into())], &svc).await,
            "an unguarded upsert (a GM edit) must trigger a reconcile"
        );
    }

    #[tokio::test]
    async fn a_remove_matching_our_own_armed_delete_is_not_relevant() {
        let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
        pending.arm_delete("sidecar.md");
        let svc = svc_with(Arc::clone(&pending), None);
        assert!(
            !batch_is_relevant(&[VaultEvent::Remove("sidecar.md".into())], &svc).await,
            "our own cleanup delete must not trigger a reconcile"
        );
    }

    #[tokio::test]
    async fn a_remove_with_no_matching_delete_guard_is_relevant() {
        let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
        let svc = svc_with(Arc::clone(&pending), None);
        assert!(
            batch_is_relevant(&[VaultEvent::Remove("sidecar.md".into())], &svc).await,
            "the GM's own sidecar deletion (no guard armed) must trigger a reconcile"
        );
    }

    #[tokio::test]
    async fn one_relevant_event_in_a_batch_makes_the_whole_batch_relevant() {
        let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
        let content = "hello";
        let hash = chronacle_vault::render::content_hash(content);
        pending.arm("ours.md", hash);
        let svc = svc_with(Arc::clone(&pending), Some(content));
        let batch = vec![
            VaultEvent::Upsert("ours.md".into()), // our own write, not relevant alone
            VaultEvent::Remove("theirs.md".into()), // no guard armed — relevant
        ];
        assert!(batch_is_relevant(&batch, &svc).await);
    }
}
