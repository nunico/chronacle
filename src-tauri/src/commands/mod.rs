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
///
/// For Phase 1 the ingestion pipeline is a stub that marks the source as
/// `pending`; full processing will be wired in a later iteration.
#[tauri::command]
pub async fn upload_source(
    state: State<'_, Arc<AppState>>,
    file_path: String,
    display_name: Option<String>,
    source_type: Option<String>,
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

    // Insert source record
    let mut response = state
        .db
        .query(
            "CREATE source SET
                id = $id,
                filename = $filename,
                display_name = $display_name,
                source_type = $source_type,
                page_count = 0,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = $embed_model",
        )
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

    Ok(created
        .into_iter()
        .next()
        .unwrap_or(serde_json::json!({"id": source_id})))
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

    let new_provider = crate::build_llm_provider_from_map(&map);
    let provider_type = crate::provider_type_name(&new_provider);

    // Swap the provider under the write lock
    *state
        .llm_provider
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {e}"))? = new_provider;

    Ok(provider_type.to_string())
}
