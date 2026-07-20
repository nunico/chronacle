//! Chat commands — chat history plus the streaming RAG send/cancel pipeline.

use std::sync::Arc;

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::State;

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
    pub response_language: Option<String>,
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
    let response_language = request
        .response_language
        .unwrap_or_else(|| "en".to_string());

    // Spawn so the command returns immediately — tokens come via events
    let task = tokio::spawn(async move {
        // Run the RAG pipeline
        let mut rx = match chronacle_retrieval::agent_service::stream_response(
            &state_ref.db,
            &state_ref.embedding_provider,
            &state_ref.vector_store,
            &state_ref.llm_provider,
            &message,
            campaign_id.as_deref(),
            &response_language,
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
        if let Err(e) = chronacle_retrieval::agent_service::persist_assistant_message(
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

    // Register the task so `chat_cancel` can abort it mid-stream.
    *state.chat_task.lock().await = Some(task.abort_handle());

    Ok(())
}

/// Abort the registered chat task, if any. Returns whether a task was found.
///
/// Aborting an already-finished task is harmless, so stale handles left in
/// the slot after normal completion are fine — the extra `done` event the
/// command emits is ignored by an idle frontend.
pub(crate) async fn cancel_chat_task(
    slot: &tokio::sync::Mutex<Option<tokio::task::AbortHandle>>,
) -> bool {
    match slot.lock().await.take() {
        Some(handle) => {
            handle.abort();
            true
        }
        None => false,
    }
}

/// Cancel the in-flight chat response, if any.
///
/// Emits a final `chat-token` with `done: true` so the frontend resolves its
/// streaming state; the partial response is kept in the UI but not persisted.
#[tauri::command]
pub async fn chat_cancel(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    if cancel_chat_task(&state.chat_task).await {
        let _ = app_handle.emit(
            "chat-token",
            ChatToken {
                token: String::new(),
                done: true,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChatRequest IPC deserialization ─────────────────────────────────────

    /// Regression for bugs #5 and #6: the frontend sends `campaignId` (camelCase)
    /// over the Tauri IPC bridge. Without `#[serde(rename_all = "camelCase")]`
    /// the struct's snake_case field never binds, every chat falls back to
    /// `None`, history is unscoped, and RAG retrieval skips the DB.
    #[test]
    fn chat_request_deserializes_camel_case_campaign_id() {
        let json = r#"{"message":"what does cover do?","campaignId":"d5a8019596844cb8b46270830df952f","responseLanguage":"de"}"#;
        let req: ChatRequest = serde_json::from_str(json).expect("camelCase IPC payload");
        assert_eq!(req.message, "what does cover do?");
        assert_eq!(
            req.campaign_id.as_deref(),
            Some("d5a8019596844cb8b46270830df952f"),
        );
        assert_eq!(req.response_language.as_deref(), Some("de"));
    }

    #[test]
    fn chat_request_deserializes_missing_campaign_id_as_none() {
        let json = r#"{"message":"hi"}"#;
        let req: ChatRequest = serde_json::from_str(json).expect("no campaign id is valid");
        assert!(req.campaign_id.is_none());
        assert!(req.response_language.is_none());
    }

    // ── Chat cancellation ────────────────────────────────────────────────────

    #[tokio::test]
    async fn cancel_chat_task_aborts_registered_task_and_empties_slot() {
        let slot = tokio::sync::Mutex::new(None);
        let task = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        *slot.lock().await = Some(task.abort_handle());

        assert!(
            cancel_chat_task(&slot).await,
            "should report a cancelled task"
        );
        let err = task.await.expect_err("task should have been aborted");
        assert!(err.is_cancelled());
        assert!(
            !cancel_chat_task(&slot).await,
            "slot should be empty after the first cancel"
        );
    }

    #[tokio::test]
    async fn cancel_chat_task_is_a_noop_without_an_active_task() {
        let slot = tokio::sync::Mutex::new(None);
        assert!(!cancel_chat_task(&slot).await);
    }
}
