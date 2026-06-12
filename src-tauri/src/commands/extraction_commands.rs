use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use tauri::State;

use crate::AppState;

/// Summary returned to the frontend when extraction completes.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractionSummary {
    pub entities_created: usize,
    pub relations_created: usize,
}

/// Trigger LLM-powered entity extraction for a collection.
///
/// The command spawns a background task (same pattern as `chat_send`) and
/// returns immediately.  Progress is emitted via `extract-progress` events;
/// the command resolves with `ExtractionSummary` when all batches are done.
///
/// Event payload: `{ phase: string, detail: string, entities_found: number, relations_found: number }`
#[tauri::command]
pub async fn extract_entities_from_collection(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    collection_id: String,
) -> Result<ExtractionSummary, String> {
    let state_ref = state.inner().clone();

    let llm = state_ref
        .llm_provider
        .read()
        .map_err(|e| format!("LLM lock error: {e}"))?
        .clone();

    let embed = state_ref
        .embedding_provider
        .read()
        .map_err(|e| format!("Embedding lock error: {e}"))?
        .clone();

    let app = app_handle.clone();
    let cid = collection_id.clone();

    let result = crate::services::extraction_service::extract_from_collection(
        &state_ref.db,
        &llm,
        &embed,
        &cid,
        move |progress| {
            let _ = app.emit("extract-progress", &progress);
        },
    )
    .await
    .map_err(|e| format!("Extraction failed: {e}"))?;

    Ok(ExtractionSummary {
        entities_created: result.entities_created,
        relations_created: result.relations_created,
    })
}
