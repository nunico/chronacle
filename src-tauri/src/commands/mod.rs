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
        .map(|r| (r.id.id.to_string(), r.value))
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
        None => {
            "SELECT role, content, created_at FROM message ORDER BY created_at ASC"
                .to_string()
        }
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

/// Uploads a source PDF file, storing it in the blob store and triggering
/// ingestion (extraction → chunking → embedding).
#[tauri::command]
pub async fn upload_source(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    file_path: String,
    display_name: Option<String>,
    source_type: Option<String>,
    campaign_id: Option<String>,
) -> Result<serde_json::Value, String> {
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

    // Insert source record with optional campaign
    // Build the query based on whether a campaign_id was provided
    let source_sql = match &campaign_id {
        Some(cid) => {
            let safe_id = cid.replace('`', "``");
            format!(
                "CREATE source SET
                    id = $id,
                    campaign = campaign:`{safe_id}`,
                    filename = $filename,
                    display_name = $display_name,
                    source_type = $source_type,
                    page_count = 0,
                    indexed_at = time::now(),
                    index_status = 'pending',
                    embed_model = $embed_model"
            )
        }
        None => {
            "CREATE source SET
                id = $id,
                campaign = NULL,
                filename = $filename,
                display_name = $display_name,
                source_type = $source_type,
                page_count = 0,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = $embed_model"
                .to_string()
        }
    };
    let mut response = state
        .db
        .query(source_sql)
        .bind(("id", source_id.to_owned()))
        .bind(("filename", filename.to_owned()))
        .bind(("display_name", display_name.to_owned()))
        .bind(("source_type", source_type.to_owned()))
        .bind(("embed_model", embed_model.to_owned()))
        .await
        .map_err(|e| format!("Failed to create source record: {e}"))?;

    let created: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created source: {e}"))?;

    let source = created
        .into_iter()
        .next()
        .unwrap_or(serde_json::json!({ "id": source_id }));

    // Run the ingestion pipeline
    let state_ref = state.inner().clone();
    let sid = source_id.clone();
    let handle = app_handle.clone();
    // Run ingestion synchronously in the command so the caller waits for completion
    let _ = handle.emit(
        "ingestion-progress",
        serde_json::json!({
            "source_id": &sid,
            "status": "indexing",
            "progress": 0.0,
        }),
    );

    match crate::services::ingestion_service::ingest_source(&state_ref, &sid).await {
        Ok(()) => {
            let _ = handle.emit(
                "ingestion-progress",
                serde_json::json!({
                    "source_id": &sid,
                    "status": "done",
                    "progress": 1.0,
                }),
            );
            Ok(source)
        }
        Err(e) => {
            // Mark source as errored
            let err_msg = e.to_string();
            eprintln!("Ingestion failed for source {sid}: {err_msg}");
            let _ = state_ref
                .db
                .query("UPDATE source SET index_status = 'error' WHERE id = type::thing('source', $id)")
                .bind(("id", sid.clone()))
                .await;
            let _ = handle.emit(
                "ingestion-error",
                serde_json::json!({
                    "source_id": &sid,
                    "error": &err_msg,
                }),
            );
            // Return the source record so the user can see it even on partial failure
            // but surface the error
            Err(format!("PDF ingestion failed: {err_msg}"))
        }
    }
}

/// Chat message request payload.
#[derive(Debug, Deserialize)]
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
                    let _ = app.emit(
                        "chat-token",
                        ChatToken {
                            token,
                            done: false,
                        },
                    );
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
    let map: std::collections::HashMap<String, String> =
        settings.into_iter().map(|r| (r.id.id.to_string(), r.value)).collect();

    Ok(LlmProviderStatus {
        provider_type: map.get("llm_provider").cloned().unwrap_or_else(|| "openai".into()),
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
pub async fn reconfigure_llm_provider(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    // Read fresh settings from the DB
    let settings = get_all_settings(&state.db).await?;
    let map: std::collections::HashMap<String, String> =
        settings.into_iter().map(|r| (r.id.id.to_string(), r.value)).collect();

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
    Ok(providers.into_iter().map(|p| CustomProviderResponse {
        id: p.id,
        name: p.name,
        provider_type: p.provider_type,
        base_url: p.base_url,
        api_key: p.api_key,
    }).collect())
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
        &state.db, name.trim(), &provider_type, base_url.trim(), &api_key,
    ).await?;
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
        &state.db, &id, &name, &provider_type, &base_url, &api_key,
    ).await?;
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
    let models = crate::services::custom_provider_service::get_models(&state.db, &provider_id).await?;
    Ok(models.into_iter().map(|m| ProviderModelResponse {
        id: m.id,
        provider_id: m.provider_id,
        model_id: m.model_id,
        display_name: m.display_name,
    }).collect())
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
        &state.db, &provider_id, model_id.trim(), display_name.trim(),
    ).await?;
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

// ── Source Commands ──────────────────────────────────────────────────

/// Response payload for a source record.
#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub source_type: String,
    pub page_count: i64,
    pub index_status: String,
    pub embed_model: String,
    pub campaign_id: Option<String>,
}

/// Returns sources, optionally filtered by campaign.
///
/// When `campaign_id` is `None`, returns all global (non-campaign) sources.
/// Pass an empty string to get all sources regardless of campaign.
#[tauri::command]
pub async fn get_sources(
    state: State<'_, Arc<AppState>>,
    campaign_id: Option<String>,
) -> Result<Vec<SourceResponse>, String> {
    let sql = match &campaign_id {
        // Empty string = all sources
        Some(cid) if cid.is_empty() || cid == "*" => {
            "SELECT * FROM source ORDER BY display_name ASC".to_string()
        }
        Some(cid) => {
            let safe_id = cid.replace('`', "``");
            format!(
                "SELECT * FROM source WHERE campaign = campaign:`{safe_id}` ORDER BY display_name ASC"
            )
        }
        None => {
            "SELECT * FROM source WHERE campaign IS NULL ORDER BY display_name ASC".to_string()
        }
    };
    let mut response = state
        .db
        .query(sql)
        .await
        .map_err(|e| format!("Failed to query sources: {e}"))?;

    #[derive(Deserialize)]
    struct SourceRow {
        id: surrealdb::sql::Thing,
        filename: String,
        display_name: String,
        source_type: String,
        page_count: i64,
        index_status: String,
        embed_model: String,
        campaign: Option<surrealdb::sql::Thing>,
    }

    let rows: Vec<SourceRow> = response
        .take(0)
        .map_err(|e| format!("Failed to parse sources: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|r| SourceResponse {
            id: r.id.id.to_string(),
            filename: r.filename,
            display_name: r.display_name,
            source_type: r.source_type,
            page_count: r.page_count,
            index_status: r.index_status,
            embed_model: r.embed_model,
            campaign_id: r.campaign.map(|c| c.id.to_string()),
        })
        .collect())
}

/// Delete a source, its blob data, and all associated chunks.
#[tauri::command]
pub async fn delete_source(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
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
    Ok(campaigns.into_iter().map(|c| CampaignResponse {
        id: c.id, name: c.name, system: c.system,
    }).collect())
}

#[tauri::command]
pub async fn get_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<CampaignResponse, String> {
    let campaign = crate::services::campaign_service::get_by_id(&state.db, &id).await?;
    Ok(CampaignResponse {
        id: campaign.id, name: campaign.name, system: campaign.system,
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
    let campaign = crate::services::campaign_service::create(
        &state.db, name.trim(), system.trim(),
    ).await?;
    Ok(CampaignResponse {
        id: campaign.id, name: campaign.name, system: campaign.system,
    })
}

#[tauri::command]
pub async fn update_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    system: String,
) -> Result<CampaignResponse, String> {
    let campaign = crate::services::campaign_service::update(
        &state.db, &id, &name, &system,
    ).await?;
    Ok(CampaignResponse {
        id: campaign.id, name: campaign.name, system: campaign.system,
    })
}

#[tauri::command]
pub async fn delete_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    crate::services::campaign_service::delete(&state.db, &id).await
}
