pub mod entity_commands;
pub use entity_commands::*;

pub mod session_commands;
pub use session_commands::*;

use std::sync::Arc;

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::State;

/// Returns a map of all stored settings key-value pairs.
#[tauri::command]
pub async fn get_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let rows = get_all_settings(&state.db).await?;
    let map = rows
        .into_iter()
        .map(|r| (r.id.id.to_raw(), r.value))
        .collect();
    Ok(map)
}

/// Helper: query all settings from the DB.
async fn get_all_settings(
    db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
) -> Result<Vec<SettingRow>, String> {
    let mut response = db
        .query("SELECT * FROM setting")
        .await
        .map_err(|e| format!("Database query failed: {e}"))?;

    let rows: Vec<SettingRow> = response
        .take(0)
        .map_err(|e| format!("Failed to parse settings: {e}"))?;

    Ok(rows)
}

#[derive(Deserialize)]
struct SettingRow {
    id: surrealdb::sql::Thing,
    value: String,
}

/// A chat-message row returned from the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageRow {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomProviderResponse {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CampaignResponse {
    pub id: String,
    pub name: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderModelResponse {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
}

/// Returns chat history from the `message` table, ordered by creation time.
///
/// When `campaign_id` is `Some`, only messages for that campaign are returned.
/// When `None`, all messages are returned.
#[tauri::command]
pub async fn get_chat_history(
    state: State<'_, Arc<AppState>>,
    campaign_id: Option<String>,
) -> Result<Vec<ChatMessageRow>, String> {
    let sql = match &campaign_id {
        Some(cid) => {
            let safe_id = cid.replace('`', "``");
            format!(
                "SELECT role, content, created_at FROM message WHERE campaign = campaign:`{safe_id}` ORDER BY created_at ASC"
            )
        }
        None => "SELECT role, content, created_at FROM message ORDER BY created_at ASC".to_string(),
    };

    let mut response = state
        .db
        .query(sql)
        .await
        .map_err(|e| format!("Failed to query chat history: {e}"))?;

    let rows: Vec<ChatMessageRow> = response
        .take(0)
        .map_err(|e| format!("Failed to parse chat history: {e}"))?;

    Ok(rows)
}

/// Upserts a single setting by key.
#[tauri::command]
pub async fn update_setting(
    state: State<'_, Arc<AppState>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let safe_key = key.replace('`', "``");
    let sql = format!("UPSERT setting:`{safe_key}` SET value = $value");

    state
        .db
        .query(sql)
        .bind(("value", value.to_owned()))
        .await
        .map_err(|e| format!("Failed to update setting: {e}"))?;

    Ok(())
}

// ── Source Commands ───────────────────────────────────────────────────────────

/// Response shape for a source record returned over IPC.
#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub source_type: String,
    pub page_count: i64,
    pub index_status: String,
    pub embed_model: String,
    pub collection_id: Option<String>,
}

/// Uploads a source PDF file, storing it in the blob store and triggering
/// ingestion (extraction → chunking → embedding).
///
/// After the DB INSERT succeeds the ingestion pipeline runs synchronously and
/// emits `ingestion-progress` / `ingestion-error` Tauri events.
#[tauri::command]
pub async fn upload_source(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    file_path: String,
    display_name: Option<String>,
    source_type: Option<String>,
    collection_id: String,
) -> Result<serde_json::Value, String> {
    if collection_id.trim().is_empty() {
        return Err("collection_id is required".to_string());
    }

    let path = std::path::PathBuf::from(&file_path);
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let display_name = display_name.unwrap_or_else(|| filename.clone());
    let source_type = source_type.unwrap_or_else(|| "rules".to_string());
    let source_id = uuid::Uuid::new_v4().to_string();
    let embed_model = "nomic-embed-text-v1.5".to_string();

    // Read file contents
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // Store in blob store
    state
        .blob_store
        .store(&source_id, &filename, &data)
        .await
        .map_err(|e| format!("Failed to store blob: {e}"))?;

    // Insert source record — collection is bound via parameter, never interpolated.
    #[derive(Deserialize)]
    struct CreatedSource {
        #[expect(dead_code)]
        id: surrealdb::sql::Thing,
    }

    let created: Vec<CreatedSource> = state
        .db
        .query(
            "CREATE source SET
                id = $id,
                campaign = NULL,
                filename = $filename,
                display_name = $display_name,
                source_type = $source_type,
                page_count = 0,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = $embed_model,
                collection = type::thing('collection', $collection_id)",
        )
        .bind(("id", source_id.to_owned()))
        .bind(("filename", filename.to_owned()))
        .bind(("display_name", display_name.to_owned()))
        .bind(("source_type", source_type.to_owned()))
        .bind(("embed_model", embed_model.to_owned()))
        .bind(("collection_id", collection_id.clone()))
        .await
        .map_err(|e| format!("Failed to create source record: {e}"))?
        .check()
        .map_err(|e| format!("Source INSERT violated a schema constraint: {e}"))?
        .take(0)
        .map_err(|e| format!("Failed to parse created source: {e}"))?;

    if created.is_empty() {
        return Err("Source creation failed: no record returned".to_string());
    }

    let source_json = serde_json::json!({
        "id": source_id,
        "filename": filename,
        "display_name": display_name,
        "source_type": source_type,
        "index_status": "pending",
        "embed_model": embed_model,
        "collection_id": collection_id,
    });

    // Build the progress callback — emits Tauri events from each pipeline stage
    let sid = source_id.clone();
    let handle = app_handle.clone();
    let on_progress: std::sync::Arc<
        dyn Fn(crate::services::ingestion_service::IngestionProgress) + Send + Sync,
    > = std::sync::Arc::new(
        move |p: crate::services::ingestion_service::IngestionProgress| {
            let _ = handle.emit(
                "ingestion-progress",
                serde_json::json!({
                    "source_id": sid,
                    "status": "indexing",
                    "progress": p.fraction,
                    "step": p.step,
                }),
            );
        },
    );

    let state_ref = state.inner().clone();
    let sid = source_id.clone();

    match crate::services::ingestion_service::ingest_source(&state_ref, &sid, on_progress).await {
        Ok(()) => {
            let _ = app_handle.emit(
                "ingestion-progress",
                serde_json::json!({
                    "source_id": &sid,
                    "status": "done",
                    "progress": 1.0,
                    "step": "Complete",
                }),
            );
            Ok(source_json)
        }
        Err(e) => {
            let err_msg = e.to_string();
            eprintln!("Ingestion failed for source {sid}: {err_msg}");
            let _ = state_ref
                .db
                .query(
                    "UPDATE source SET index_status = 'error' \
                     WHERE id = type::thing('source', $id)",
                )
                .bind(("id", sid.clone()))
                .await;
            let _ = app_handle.emit(
                "ingestion-error",
                serde_json::json!({
                    "source_id": &sid,
                    "error": &err_msg,
                }),
            );
            Err(format!("PDF ingestion failed: {err_msg}"))
        }
    }
}

/// Returns all sources, optionally filtered to a specific collection.
///
/// When `collection_id` is provided the query uses a parameterised binding
/// (never string interpolation) to avoid SQL-injection risks.
#[tauri::command]
pub async fn get_sources(
    state: State<'_, Arc<AppState>>,
    collection_id: Option<String>,
) -> Result<Vec<SourceResponse>, String> {
    /// Raw row shape as SurrealDB returns it.
    #[derive(Deserialize)]
    struct SourceRow {
        id: surrealdb::sql::Thing,
        filename: String,
        display_name: String,
        source_type: String,
        page_count: i64,
        index_status: String,
        embed_model: String,
        collection: Option<surrealdb::sql::Thing>,
    }

    let mut response = if let Some(ref cid) = collection_id {
        state
            .db
            .query(
                "SELECT * FROM source \
                 WHERE collection = type::thing('collection', $cid) \
                 ORDER BY display_name ASC",
            )
            .bind(("cid", cid.clone()))
            .await
            .map_err(|e| format!("Failed to query sources: {e}"))?
    } else {
        state
            .db
            .query("SELECT * FROM source ORDER BY display_name ASC")
            .await
            .map_err(|e| format!("Failed to query sources: {e}"))?
    };

    let rows: Vec<SourceRow> = response
        .take(0)
        .map_err(|e| format!("Failed to parse sources: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|r| SourceResponse {
            id: r.id.id.to_raw(),
            filename: r.filename,
            display_name: r.display_name,
            source_type: r.source_type,
            page_count: r.page_count,
            index_status: r.index_status,
            embed_model: r.embed_model,
            collection_id: r.collection.map(|t| t.id.to_raw()),
        })
        .collect())
}

// ── Collection Commands ───────────────────────────────────────────────────────

/// IPC response shape for a `collection` record.
#[derive(Debug, Clone, Serialize)]
pub struct CollectionResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<crate::services::collection_service::Collection> for CollectionResponse {
    fn from(c: crate::services::collection_service::Collection) -> Self {
        Self {
            id: c.id,
            name: c.name,
            description: c.description,
        }
    }
}

/// Returns all collections ordered by name.
#[tauri::command]
pub async fn get_collections(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CollectionResponse>, String> {
    let collections = crate::services::collection_service::get_all(&state.db).await?;
    Ok(collections.into_iter().map(Into::into).collect())
}

/// Creates a new collection.  `name` must be non-empty.
#[tauri::command]
pub async fn create_collection(
    state: State<'_, Arc<AppState>>,
    name: String,
    description: Option<String>,
) -> Result<CollectionResponse, String> {
    if name.trim().is_empty() {
        return Err("Collection name is required".to_string());
    }
    let c =
        crate::services::collection_service::create(&state.db, name.trim(), description.as_deref())
            .await?;
    Ok(c.into())
}

/// Updates the name and description of an existing collection.
#[tauri::command]
pub async fn update_collection(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    description: Option<String>,
) -> Result<CollectionResponse, String> {
    if name.trim().is_empty() {
        return Err("Collection name is required".to_string());
    }
    let c = crate::services::collection_service::update(
        &state.db,
        &id,
        name.trim(),
        description.as_deref(),
    )
    .await?;
    Ok(c.into())
}

/// Deletes a collection.  Fails if any campaigns are subscribed or any sources
/// still reference it.
#[tauri::command]
pub async fn delete_collection(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    crate::services::collection_service::delete(&state.db, &id).await
}

/// Subscribes a campaign to a collection.  Idempotent.
#[tauri::command]
pub async fn add_campaign_collection(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    collection_id: String,
) -> Result<(), String> {
    crate::services::collection_service::add_campaign_collection(
        &state.db,
        &campaign_id,
        &collection_id,
    )
    .await
}

/// Removes a campaign's subscription to a collection.
#[tauri::command]
pub async fn remove_campaign_collection(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    collection_id: String,
) -> Result<(), String> {
    crate::services::collection_service::remove_campaign_collection(
        &state.db,
        &campaign_id,
        &collection_id,
    )
    .await
}

/// Returns all collections to which a campaign is subscribed.
#[tauri::command]
pub async fn get_campaign_collections(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<Vec<CollectionResponse>, String> {
    let cols =
        crate::services::collection_service::get_campaign_collections(&state.db, &campaign_id)
            .await?;
    Ok(cols.into_iter().map(Into::into).collect())
}

// ── Chat Commands ─────────────────────────────────────────────────────────────

/// Chat message request payload.
///
/// Tauri's IPC bridge auto-camelCases top-level command arguments but NOT
/// struct fields — without `rename_all = "camelCase"` the `campaignId` the
/// frontend sends silently deserializes as `None`, which collapses every
/// chat into the unscoped path (no campaign-filtered history, empty
/// collection list, RAG returns no context).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub message: String,
    pub campaign_id: Option<String>,
}

/// Chat message response chunk.
#[derive(Debug, Clone, Serialize)]
pub struct ChatToken {
    pub token: String,
    pub done: bool,
}

/// Sends a user message through the full RAG pipeline and emits streaming
/// tokens via Tauri events.
///
/// Pipeline:
///   1. Embed the query → search vector store → build context
///   2. Stream the LLM response token-by-token via `chat-token` events
///   3. On completion, persist the assistant message with parsed citations
///
/// The command returns immediately; the pipeline runs in a background task.
#[tauri::command]
pub async fn chat_send(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    request: ChatRequest,
) -> Result<(), String> {
    let app = app_handle.clone();
    let state_ref = state.inner().clone();
    let message = request.message;
    let campaign_id = request.campaign_id;

    // Spawn so the command returns immediately — tokens come via events
    tokio::spawn(async move {
        // Run the RAG pipeline
        let mut rx = match crate::services::agent_service::stream_response(
            &state_ref,
            &message,
            campaign_id.as_deref(),
        )
        .await
        {
            Ok(rx) => rx,
            Err(e) => {
                let _ = app.emit(
                    "chat-token",
                    ChatToken {
                        token: format!("[Error: {e}]"),
                        done: true,
                    },
                );
                return;
            }
        };

        // Stream tokens to the frontend while collecting the full response
        let mut full_response = String::new();
        while let Some(token_result) = rx.recv().await {
            match token_result {
                Ok(token) => {
                    full_response.push_str(&token);
                    let _ = app.emit("chat-token", ChatToken { token, done: false });
                }
                Err(e) => {
                    let _ = app.emit(
                        "chat-token",
                        ChatToken {
                            token: format!("[Error: {e}]"),
                            done: true,
                        },
                    );
                    return;
                }
            }
        }

        // Persist the full assistant response with parsed citations
        if let Err(e) = crate::services::agent_service::persist_assistant_message(
            &state_ref.db,
            &full_response,
            campaign_id.as_deref(),
        )
        .await
        {
            eprintln!("Failed to persist assistant message: {e}");
        }

        // Signal completion
        let _ = app.emit(
            "chat-token",
            ChatToken {
                token: String::new(),
                done: true,
            },
        );
    });

    Ok(())
}

/// Response payload for the LLM provider status.
#[derive(Debug, Clone, Serialize)]
pub struct LlmProviderStatus {
    pub provider_type: String,
    pub model: String,
    pub api_key_configured: bool,
}

/// Returns the current LLM provider configuration status.
#[tauri::command]
pub async fn get_llm_provider_status(
    state: State<'_, Arc<AppState>>,
) -> Result<LlmProviderStatus, String> {
    let settings = get_all_settings(&state.db).await?;
    let map: std::collections::HashMap<String, String> = settings
        .into_iter()
        .map(|r| (r.id.id.to_raw(), r.value))
        .collect();

    Ok(LlmProviderStatus {
        provider_type: map
            .get("llm_provider")
            .cloned()
            .unwrap_or_else(|| "openai".into()),
        model: map.get("llm_model").cloned().unwrap_or_default(),
        api_key_configured: map
            .get("llm_api_key")
            .map(|k| !k.is_empty())
            .unwrap_or(false),
    })
}

/// Re-read settings from the database and reconstruct the LLM provider at
/// runtime. Returns the active provider type name on success.
#[tauri::command]
pub async fn reconfigure_llm_provider(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    // Read fresh settings from the DB
    let settings = get_all_settings(&state.db).await?;
    let map: std::collections::HashMap<String, String> = settings
        .into_iter()
        .map(|r| (r.id.id.to_raw(), r.value))
        .collect();

    let new_provider = crate::build_llm_provider_from_map(&map, Some(&state.db)).await;
    let provider_type = crate::provider_type_name(&new_provider);

    // Swap the provider under the write lock
    *state
        .llm_provider
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {e}"))? = new_provider;

    Ok(provider_type.to_string())
}

// ── Custom Provider Commands ──────────────────────────────────────────

#[tauri::command]
pub async fn get_custom_providers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CustomProviderResponse>, String> {
    let providers = crate::services::custom_provider_service::get_all(&state.db).await?;
    Ok(providers
        .into_iter()
        .map(|p| CustomProviderResponse {
            id: p.id,
            name: p.name,
            provider_type: p.provider_type,
            base_url: p.base_url,
            api_key: p.api_key,
        })
        .collect())
}

#[tauri::command]
pub async fn create_custom_provider(
    state: State<'_, Arc<AppState>>,
    name: String,
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<CustomProviderResponse, String> {
    if name.trim().is_empty() {
        return Err("Provider name is required".to_string());
    }
    if provider_type != "openai" && provider_type != "anthropic" {
        return Err("provider_type must be 'openai' or 'anthropic'".to_string());
    }
    if base_url.trim().is_empty() {
        return Err("Base URL is required".to_string());
    }
    let provider = crate::services::custom_provider_service::create(
        &state.db,
        name.trim(),
        &provider_type,
        base_url.trim(),
        &api_key,
    )
    .await?;
    Ok(CustomProviderResponse {
        id: provider.id,
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
    })
}

#[tauri::command]
pub async fn update_custom_provider(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<CustomProviderResponse, String> {
    let provider = crate::services::custom_provider_service::update(
        &state.db,
        &id,
        &name,
        &provider_type,
        &base_url,
        &api_key,
    )
    .await?;
    Ok(CustomProviderResponse {
        id: provider.id,
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
    })
}

#[tauri::command]
pub async fn delete_custom_provider(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    crate::services::custom_provider_service::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn get_provider_models(
    state: State<'_, Arc<AppState>>,
    provider_id: String,
) -> Result<Vec<ProviderModelResponse>, String> {
    let models =
        crate::services::custom_provider_service::get_models(&state.db, &provider_id).await?;
    Ok(models
        .into_iter()
        .map(|m| ProviderModelResponse {
            id: m.id,
            provider_id: m.provider_id,
            model_id: m.model_id,
            display_name: m.display_name,
        })
        .collect())
}

#[tauri::command]
pub async fn add_provider_model(
    state: State<'_, Arc<AppState>>,
    provider_id: String,
    model_id: String,
    display_name: String,
) -> Result<ProviderModelResponse, String> {
    if model_id.trim().is_empty() {
        return Err("Model ID is required".to_string());
    }
    if display_name.trim().is_empty() {
        return Err("Display name is required".to_string());
    }
    let model = crate::services::custom_provider_service::add_model(
        &state.db,
        &provider_id,
        model_id.trim(),
        display_name.trim(),
    )
    .await?;
    Ok(ProviderModelResponse {
        id: model.id,
        provider_id: model.provider_id,
        model_id: model.model_id,
        display_name: model.display_name,
    })
}

#[tauri::command]
pub async fn remove_provider_model(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    crate::services::custom_provider_service::remove_model(&state.db, &id).await
}

/// Delete a source, its blob data, and all associated chunks.
#[tauri::command]
pub async fn delete_source(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    // Check source exists before deleting
    let mut exists = state
        .db
        .query("SELECT count() FROM source WHERE id = type::thing('source', $id) GROUP ALL")
        .bind(("id", id.clone()))
        .await
        .map_err(|e| format!("Failed to query source: {e}"))?;

    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }
    let counts: Vec<CountRow> = exists
        .take(0)
        .map_err(|e| format!("Failed to parse source count: {e}"))?;

    if counts.first().map(|c| c.count).unwrap_or(0) > 0 {
        // Delete blob
        state
            .blob_store
            .delete(&id)
            .await
            .map_err(|e| format!("Failed to delete blob: {e}"))?;

        // Delete vector chunks
        state
            .vector_store
            .delete_by_source(&id)
            .await
            .map_err(|e| format!("Failed to delete chunks: {e}"))?;

        // Delete source record
        state
            .db
            .query("DELETE type::thing('source', $id)")
            .bind(("id", id))
            .await
            .map_err(|e| format!("Failed to delete source: {e}"))?;
    }

    Ok(())
}

// ── Campaign Commands ────────────────────────────────────────────────

#[tauri::command]
pub async fn get_campaigns(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CampaignResponse>, String> {
    let campaigns = crate::services::campaign_service::get_all(&state.db).await?;
    Ok(campaigns
        .into_iter()
        .map(|c| CampaignResponse {
            id: c.id,
            name: c.name,
            system: c.system,
        })
        .collect())
}

#[tauri::command]
pub async fn get_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<CampaignResponse, String> {
    let campaign = crate::services::campaign_service::get_by_id(&state.db, &id).await?;
    Ok(CampaignResponse {
        id: campaign.id,
        name: campaign.name,
        system: campaign.system,
    })
}

#[tauri::command]
pub async fn create_campaign(
    state: State<'_, Arc<AppState>>,
    name: String,
    system: String,
) -> Result<CampaignResponse, String> {
    if name.trim().is_empty() {
        return Err("Campaign name is required".to_string());
    }
    let campaign =
        crate::services::campaign_service::create(&state.db, name.trim(), system.trim()).await?;
    Ok(CampaignResponse {
        id: campaign.id,
        name: campaign.name,
        system: campaign.system,
    })
}

#[tauri::command]
pub async fn update_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    system: String,
) -> Result<CampaignResponse, String> {
    let campaign =
        crate::services::campaign_service::update(&state.db, &id, &name, &system).await?;
    Ok(CampaignResponse {
        id: campaign.id,
        name: campaign.name,
        system: campaign.system,
    })
}

#[tauri::command]
pub async fn delete_campaign(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    crate::services::campaign_service::delete(&state.db, &id).await
}

// ── Embedding Model Commands ─────────────────────────────────────────

/// Report which sources were embedded with a different model than the active one.
///
/// Returns an empty `stale` list when every indexed source matches the active
/// embedding provider's model ID. The mock provider (used as a placeholder
/// before the real model is downloaded) is treated as "no active model" and
/// always returns clean — it's not a real mismatch, just the pre-download state.
#[tauri::command]
pub async fn get_embedding_model_mismatch(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::providers::embedding::EmbeddingModelMismatch, String> {
    let active = state
        .embedding_provider
        .read()
        .map_err(|e| format!("embedding lock: {e}"))?
        .model_name()
        .to_string();
    if active == "mock" {
        return Ok(crate::providers::embedding::EmbeddingModelMismatch {
            active_model: active,
            stale: Vec::new(),
        });
    }
    crate::providers::embedding::check_embedding_model_consistency(&state.db, &active)
        .await
        .map_err(|e| format!("mismatch check failed: {e}"))
}

/// Check whether the nomic-embed-text-v1.5 model is already cached.
#[tauri::command]
pub async fn check_embedding_model(_state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let data_dir = crate::app_data_dir();
    let cache_dir = crate::providers::embedding::FastEmbedProvider::cache_dir(&data_dir);
    Ok(crate::providers::embedding::FastEmbedProvider::is_cached(
        &cache_dir,
    ))
}

/// Download the embedding model with streaming progress.
///
/// Progress events are emitted as `model-download-progress` with payload:
/// `{ status: "downloading"|"done"|"error", file, bytes_downloaded,
///   total_bytes, progress: 0.0-1.0 }`.
#[tauri::command]
pub async fn download_embedding_model(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let data_dir = crate::app_data_dir();
    let cache_dir = data_dir.join("embedding_model");

    // Ensure the cache directory exists
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .map_err(|e| format!("Failed to create cache dir: {e}"))?;

    // Emit initial progress
    let _ = app_handle.emit(
        "model-download-progress",
        serde_json::json!({
            "status": "downloading",
            "file": "",
            "bytes_downloaded": 0,
            "total_bytes": 0,
            "progress": 0.0,
        }),
    );

    // Download model files using reqwest streaming
    let client = reqwest::Client::new();

    // Download each model file
    let model_files = vec![
        ("tokenizer.json", "tokenizer.json"),
        ("config.json", "config.json"),
        ("special_tokens_map.json", "special_tokens_map.json"),
        ("tokenizer_config.json", "tokenizer_config.json"),
        ("onnx/model.onnx", "onnx/model.onnx"),
    ];

    // Ensure onnx subdirectory exists
    tokio::fs::create_dir_all(cache_dir.join("onnx"))
        .await
        .map_err(|e| format!("Failed to create onnx dir: {e}"))?;

    let total_files = model_files.len() as f32;
    let hf_base = "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main";

    for (i, (url_suffix, local_path)) in model_files.iter().enumerate() {
        let url = format!("{hf_base}/{url_suffix}");
        let dest = cache_dir.join(local_path);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to download {local_path}: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Failed to download {local_path}: HTTP {}",
                response.status()
            ));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = tokio::fs::File::create(&dest)
            .await
            .map_err(|e| format!("Failed to create {local_path}: {e}"))?;

        use tokio::io::AsyncWriteExt;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
            let chunk = chunk.map_err(|e| format!("Download stream error: {e}"))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Write error: {e}"))?;
            downloaded += chunk.len() as u64;

            let file_progress = if total_size > 0 {
                downloaded as f32 / total_size as f32
            } else {
                0.0
            };
            let overall = (i as f32 + file_progress) / total_files;

            let _ = app_handle.emit(
                "model-download-progress",
                serde_json::json!({
                    "status": "downloading",
                    "file": local_path,
                    "bytes_downloaded": downloaded,
                    "total_bytes": total_size,
                    "progress": overall,
                }),
            );
        }

        file.flush()
            .await
            .map_err(|e| format!("Flush error: {e}"))?;
    }

    // Now initialize the real embedding provider from the cached files
    let model_dir = cache_dir.join("models--nomic-ai--nomic-embed-text-v1.5/snapshots/download");
    tokio::fs::create_dir_all(&model_dir)
        .await
        .map_err(|e| format!("Failed to create model dir: {e}"))?;
    tokio::fs::create_dir_all(model_dir.join("onnx"))
        .await
        .map_err(|e| format!("Failed to create model onnx dir: {e}"))?;

    // Copy downloaded files into hf-hub-compatible cache structure
    for (_, local_path) in &model_files {
        let src = cache_dir.join(local_path);
        let dst = cache_dir
            .join("models--nomic-ai--nomic-embed-text-v1.5/snapshots/download")
            .join(local_path);
        tokio::fs::copy(&src, &dst)
            .await
            .map_err(|e| format!("Failed to copy {local_path}: {e}"))?;
    }

    // Write a sentinel ref so we know the model is cached
    let refs_dir = cache_dir.join("models--nomic-ai--nomic-embed-text-v1.5/refs");
    tokio::fs::create_dir_all(&refs_dir)
        .await
        .map_err(|e| format!("Failed to create refs dir: {e}"))?;
    tokio::fs::write(refs_dir.join("main"), b"download")
        .await
        .map_err(|e| format!("Failed to write ref: {e}"))?;

    // Initialize the real FastEmbedProvider using the custom cache dir
    let real_provider = crate::providers::embedding::FastEmbedProvider::try_new(Some(&cache_dir))
        .map_err(|e| format!("Failed to initialize embedding model: {e}"))?;

    // Swap the provider in AppState
    *state
        .embedding_provider
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {e}"))? = Arc::new(real_provider);

    let _ = app_handle.emit(
        "model-download-progress",
        serde_json::json!({
            "status": "done",
            "file": "",
            "bytes_downloaded": 0,
            "total_bytes": 0,
            "progress": 1.0,
        }),
    );

    Ok(())
}

// ── Re-index all sources ──────────────────────────────────────────────

/// Enumerate all source IDs in the database.
async fn list_all_source_ids<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query("SELECT id FROM source")
        .await
        .map_err(|e| e.to_string())?;
    let rows: Vec<Row> = resp.take(0).map_err(|e| e.to_string())?;
    // Use `.to_raw()` not `.to_string()`. SurrealDB's `Id::to_string()`
    // wraps string values that need escaping (e.g. UUIDs with hyphens) in
    // backticks; passing that back through `type::thing('source', $id)`
    // produces a mangled `source:`\`uuid\`` reference that never matches
    // the real record. See commit e099a79 for the prior occurrence.
    Ok(rows.into_iter().map(|r| r.id.id.to_raw()).collect())
}

/// Re-run ingestion for every source currently in the database.
///
/// For each source: delete existing chunks, then call `ingest_source` again.
/// Emits a `reindex-progress` event per pipeline tick so the UI can show
/// progress across sources.
#[tauri::command]
pub async fn reindex_all_sources(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, String> {
    let ids = list_all_source_ids(&state.db).await?;
    let total = ids.len();

    for (idx, sid) in ids.iter().enumerate() {
        let sid_for_progress = sid.clone();
        let handle = app_handle.clone();
        let on_progress: std::sync::Arc<
            dyn Fn(crate::services::ingestion_service::IngestionProgress) + Send + Sync,
        > = std::sync::Arc::new(move |p| {
            let _ = handle.emit(
                "reindex-progress",
                serde_json::json!({
                    "source_id": &sid_for_progress,
                    "current": idx + 1,
                    "total": total,
                    "progress": p.fraction,
                    "step": p.step,
                }),
            );
        });

        state
            .vector_store
            .delete_by_source(sid)
            .await
            .map_err(|e| format!("delete chunks for {sid}: {e}"))?;

        let state_ref = state.inner().clone();
        crate::services::ingestion_service::ingest_source(&state_ref, sid, on_progress)
            .await
            .map_err(|e| format!("re-ingest {sid}: {e}"))?;
    }

    Ok(total)
}

// ── Citation chunk lookup ─────────────────────────────────────────────

/// The chunk text + locator returned for a citation popover.
#[derive(Serialize)]
pub struct CitationChunk {
    pub text: String,
    pub page_start: i64,
    pub page_end: i64,
    pub section_heading: String,
}

/// Look up the chunk that backs a citation, so the UI can show the source
/// passage when the user clicks the citation badge.
///
/// `source_name` matches `source.filename`. `page` is the cited page (the
/// first number when the citation says `p.45-49`). If multiple chunks span
/// the page, the earliest one is returned. None if no chunk matches.
#[tauri::command]
pub async fn get_chunk_for_citation(
    state: State<'_, Arc<AppState>>,
    source_name: String,
    page: Option<i64>,
) -> Result<Option<CitationChunk>, String> {
    // Resolve the source.id first via filename so the chunk query can use
    // it directly. Doing this in two steps avoids relying on SurrealDB's
    // record-link filtering inside WHERE, which the MTREE optimizer has
    // surprised us with before.
    let mut src_resp = state
        .db
        .query("SELECT id FROM source WHERE filename = $name LIMIT 1")
        .bind(("name", source_name))
        .await
        .map_err(|e| format!("source lookup: {e}"))?;
    #[derive(Deserialize)]
    struct SourceIdRow {
        id: surrealdb::sql::Thing,
    }
    let src_rows: Vec<SourceIdRow> = src_resp
        .take(0)
        .map_err(|e| format!("source decode: {e}"))?;
    let Some(src_id) = src_rows.into_iter().next() else {
        return Ok(None);
    };

    // Build the chunk query — gate on page only when one was provided.
    let sql = if page.is_some() {
        "SELECT text, page_start, page_end, section_heading FROM chunk \
         WHERE source = $src AND page_start <= $page AND page_end >= $page \
         ORDER BY page_start ASC LIMIT 1"
    } else {
        "SELECT text, page_start, page_end, section_heading FROM chunk \
         WHERE source = $src ORDER BY page_start ASC LIMIT 1"
    };

    let mut chunk_resp = state
        .db
        .query(sql)
        .bind(("src", src_id.id))
        .bind(("page", page))
        .await
        .map_err(|e| format!("chunk lookup: {e}"))?;
    #[derive(Deserialize)]
    struct ChunkRow {
        text: String,
        page_start: i64,
        page_end: i64,
        section_heading: String,
    }
    let chunk_rows: Vec<ChunkRow> = chunk_resp
        .take(0)
        .map_err(|e| format!("chunk decode: {e}"))?;
    Ok(chunk_rows.into_iter().next().map(|r| CitationChunk {
        text: r.text,
        page_start: r.page_start,
        page_end: r.page_end,
        section_heading: r.section_heading,
    }))
}

#[cfg(test)]
mod citation_tests {
    use super::*;

    async fn seed_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("t").use_db("t").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE collection SET id='col1', name='Test', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE source SET id='quickstart', filename='Quickstart.pdf', \
             display_name='Quickstart', source_type='rules', page_count=10, \
             indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5', \
             collection=type::thing('collection','col1')",
        )
        .await
        .unwrap();
        // Two chunks: one on p.9, one on p.20-22. The embedding must have
        // dimension 768 to satisfy the MTREE index; the actual values don't
        // matter for citation-lookup tests.
        let zeros: String = std::iter::repeat_n("0.0", 768)
            .collect::<Vec<_>>()
            .join(",");
        db.query(format!(
            "CREATE chunk SET id='c1', source=type::thing('source','quickstart'), \
             collection=type::thing('collection','col1'), \
             text='Lantern orbits Mirovia', page_start=9, page_end=9, \
             section_heading='Intro', source_type='rules', embedding=[{zeros}], \
             embed_model='nomic-embed-text-v1.5'"
        ))
        .await
        .unwrap()
        .check()
        .unwrap();
        db.query(format!(
            "CREATE chunk SET id='c2', source=type::thing('source','quickstart'), \
             collection=type::thing('collection','col1'), \
             text='Council factions list', page_start=20, page_end=22, \
             section_heading='Factions', source_type='rules', embedding=[{zeros}], \
             embed_model='nomic-embed-text-v1.5'"
        ))
        .await
        .unwrap()
        .check()
        .unwrap();
        db
    }

    /// Mirrors get_chunk_for_citation without needing a Tauri State.
    async fn lookup<C: surrealdb::Connection>(
        db: &surrealdb::Surreal<C>,
        source_name: &str,
        page: Option<i64>,
    ) -> Option<CitationChunk> {
        let mut src_resp = db
            .query("SELECT id FROM source WHERE filename = $name LIMIT 1")
            .bind(("name", source_name.to_owned()))
            .await
            .ok()?;
        #[derive(Deserialize)]
        struct SourceIdRow {
            id: surrealdb::sql::Thing,
        }
        let src: Vec<SourceIdRow> = src_resp.take(0).ok()?;
        let src_id = src.into_iter().next()?.id;
        let sql = if page.is_some() {
            "SELECT text, page_start, page_end, section_heading FROM chunk \
             WHERE source = $src AND page_start <= $page AND page_end >= $page \
             ORDER BY page_start ASC LIMIT 1"
        } else {
            "SELECT text, page_start, page_end, section_heading FROM chunk \
             WHERE source = $src ORDER BY page_start ASC LIMIT 1"
        };
        let mut resp = db
            .query(sql)
            .bind(("src", src_id))
            .bind(("page", page))
            .await
            .ok()?;
        #[derive(Deserialize)]
        struct R {
            text: String,
            page_start: i64,
            page_end: i64,
            section_heading: String,
        }
        let rows: Vec<R> = resp.take(0).ok()?;
        rows.into_iter().next().map(|r| CitationChunk {
            text: r.text,
            page_start: r.page_start,
            page_end: r.page_end,
            section_heading: r.section_heading,
        })
    }

    #[tokio::test]
    async fn returns_chunk_for_exact_page_hit() {
        let db = seed_db().await;
        let got = lookup(&db, "Quickstart.pdf", Some(9)).await.unwrap();
        assert_eq!(got.text, "Lantern orbits Mirovia");
        assert_eq!(got.page_start, 9);
        assert_eq!(got.section_heading, "Intro");
    }

    #[tokio::test]
    async fn returns_chunk_when_page_in_range() {
        let db = seed_db().await;
        let got = lookup(&db, "Quickstart.pdf", Some(21)).await.unwrap();
        assert_eq!(got.text, "Council factions list");
        assert_eq!(got.page_start, 20);
        assert_eq!(got.page_end, 22);
    }

    #[tokio::test]
    async fn returns_none_for_unknown_source() {
        let db = seed_db().await;
        assert!(lookup(&db, "Nonexistent.pdf", Some(1)).await.is_none());
    }

    #[tokio::test]
    async fn returns_none_for_page_with_no_chunk() {
        let db = seed_db().await;
        assert!(lookup(&db, "Quickstart.pdf", Some(99)).await.is_none());
    }

    #[tokio::test]
    async fn returns_first_chunk_when_page_omitted() {
        let db = seed_db().await;
        let got = lookup(&db, "Quickstart.pdf", None).await.unwrap();
        // page_start=9 is earlier than page_start=20
        assert_eq!(got.page_start, 9);
    }
}

#[cfg(test)]
mod reindex_tests {
    use super::*;

    #[tokio::test]
    async fn list_all_source_ids_returns_all_ids() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE collection SET id='col1', name='Test', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE source SET id='s1', filename='a.pdf', display_name='a', \
             source_type='rules', page_count=0, indexed_at=time::now(), \
             index_status='done', embed_model='nomic-embed-text-v1.5', \
             collection=type::thing('collection','col1')",
        )
        .await
        .unwrap();
        db.query(
            "CREATE source SET id='s2', filename='b.pdf', display_name='b', \
             source_type='rules', page_count=0, indexed_at=time::now(), \
             index_status='done', embed_model='nomic-embed-text-v1.5', \
             collection=type::thing('collection','col1')",
        )
        .await
        .unwrap();

        let ids = list_all_source_ids(&db).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"s1".to_string()));
        assert!(ids.contains(&"s2".to_string()));
    }

    /// Regression test for the backtick-wrapped-ID bug. UUIDs contain hyphens,
    /// which trigger SurrealDB's `EscapeRidKey` when `Id::to_string()` is used.
    /// `list_all_source_ids` MUST return raw IDs so they can be passed back
    /// through `type::thing('source', $id)` without producing a mangled record
    /// reference. See commit e099a79 for the prior occurrence in delete_source.
    #[tokio::test]
    async fn list_all_source_ids_does_not_wrap_uuids_in_backticks() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        let uuid = "d5a80195-3968-44cb-8b46-270830df952f";
        db.query(
            "CREATE collection SET id='col1', name='Test', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(format!(
            "CREATE source SET id='{uuid}', filename='a.pdf', display_name='a', \
             source_type='rules', page_count=0, indexed_at=time::now(), \
             index_status='done', embed_model='nomic-embed-text-v1.5', \
             collection=type::thing('collection','col1')"
        ))
        .await
        .unwrap();

        let ids = list_all_source_ids(&db).await.unwrap();
        assert_eq!(ids.len(), 1);
        let id = &ids[0];
        assert!(
            !id.contains('`'),
            "ID must not be wrapped in backticks: got {id:?}"
        );
        assert_eq!(id, uuid);

        // Round-trip check: the returned ID must work with type::thing.
        // If the bug recurs, this query returns no rows.
        let mut resp = db
            .query("SELECT id FROM source WHERE id = type::thing('source', $id)")
            .bind(("id", id.clone()))
            .await
            .unwrap();
        #[derive(Deserialize)]
        struct Found {
            #[allow(dead_code)]
            id: surrealdb::sql::Thing,
        }
        let found: Vec<Found> = resp.take(0).unwrap();
        assert_eq!(
            found.len(),
            1,
            "round-trip lookup with raw ID must find the source"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::engine::local::Db;
    use surrealdb::Surreal;

    async fn setup_db() -> Surreal<Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db
    }

    // ── ChatRequest IPC deserialization ─────────────────────────────────────

    /// Regression for bugs #5 and #6: the frontend sends `campaignId` (camelCase)
    /// over the Tauri IPC bridge. Without `#[serde(rename_all = "camelCase")]`
    /// the struct's snake_case field never binds, every chat falls back to
    /// `None`, history is unscoped, and RAG retrieval skips the DB.
    #[test]
    fn chat_request_deserializes_camel_case_campaign_id() {
        let json =
            r#"{"message":"what does cover do?","campaignId":"d5a8019596844cb8b46270830df952f"}"#;
        let req: ChatRequest = serde_json::from_str(json).expect("camelCase IPC payload");
        assert_eq!(req.message, "what does cover do?");
        assert_eq!(
            req.campaign_id.as_deref(),
            Some("d5a8019596844cb8b46270830df952f"),
        );
    }

    #[test]
    fn chat_request_deserializes_missing_campaign_id_as_none() {
        let json = r#"{"message":"hi"}"#;
        let req: ChatRequest = serde_json::from_str(json).expect("no campaign id is valid");
        assert!(req.campaign_id.is_none());
    }

    // ── CollectionResponse conversion ────────────────────────────────────────

    #[test]
    fn collection_response_from_collection() {
        let c = crate::services::collection_service::Collection {
            id: "abc123".to_string(),
            name: "D&D 5e Core".to_string(),
            description: Some("Core rulebooks".to_string()),
        };
        let resp = CollectionResponse::from(c);
        assert_eq!(resp.id, "abc123");
        assert_eq!(resp.name, "D&D 5e Core");
        assert_eq!(resp.description.as_deref(), Some("Core rulebooks"));
    }

    #[test]
    fn collection_response_from_collection_no_desc() {
        let c = crate::services::collection_service::Collection {
            id: "xyz".to_string(),
            name: "Pathfinder".to_string(),
            description: None,
        };
        let resp = CollectionResponse::from(c);
        assert!(resp.description.is_none());
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
}
