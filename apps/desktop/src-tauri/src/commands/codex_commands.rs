use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use tauri::State;

use crate::AppState;
use chronacle_extraction::codex_service::CompileProgress;

/// Summary returned to the frontend when a codex compile run completes.
#[derive(Debug, Clone, Serialize)]
pub struct CompileSummary {
    pub articles_compiled: usize,
    pub remaining_stale: usize,
}

/// Emit a phased progress event to the frontend.
fn emit_progress(app: &tauri::AppHandle, p: &CompileProgress) {
    let _ = app.emit("codex-progress", p);
}

/// Compile every stale (or article-less) entity in a collection into a
/// grounded codex article. Abortable.
#[tauri::command]
pub async fn compile_collection(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    collection_id: String,
) -> Result<CompileSummary, String> {
    let state_ref = state.inner().clone();
    let llm = state_ref
        .llm_provider
        .read()
        .map_err(|e| format!("LLM lock: {e}"))?
        .clone();
    let embed = state_ref
        .embedding_provider
        .read()
        .map_err(|e| format!("Embed lock: {e}"))?
        .clone();
    let vector_store = state_ref.vector_store.clone();
    let app = app_handle.clone();
    let task_state = state_ref.clone();
    let task_collection = collection_id.clone();

    let task = tokio::spawn(async move {
        chronacle_extraction::codex_service::compile_collection(
            &task_state.db,
            &llm,
            &embed,
            &vector_store,
            &task_collection,
            move |p| emit_progress(&app, &p),
        )
        .await
    });

    *state.compile_task.lock().await = Some(task.abort_handle());

    match task.await {
        Ok(Ok(result)) => Ok(CompileSummary {
            articles_compiled: result.articles_compiled,
            remaining_stale: result.remaining_stale,
        }),
        Ok(Err(e)) => Err(format!("Compile failed: {e}")),
        Err(join_err) if join_err.is_cancelled() => Err("cancelled".to_string()),
        Err(join_err) => Err(format!("Compile task error: {join_err}")),
    }
}

/// Compile a single entity (per-entity "Recompile" in the UI). Not abortable —
/// runs inline since a single entity compile is fast.
#[tauri::command]
pub async fn compile_entity(
    state: State<'_, Arc<AppState>>,
    kind: String,
    id: String,
) -> Result<bool, String> {
    let state_ref = state.inner().clone();
    let llm = state_ref
        .llm_provider
        .read()
        .map_err(|e| format!("LLM lock: {e}"))?
        .clone();
    let embed = state_ref
        .embedding_provider
        .read()
        .map_err(|e| format!("Embed lock: {e}"))?
        .clone();
    let vector_store = state_ref.vector_store.clone();

    chronacle_extraction::codex_service::compile_entity(
        &state_ref.db,
        &llm,
        &embed,
        &vector_store,
        &kind,
        &id,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Codex staleness status for a collection (drives the UI badges).
#[tauri::command]
pub async fn get_codex_status(
    state: State<'_, Arc<AppState>>,
    collection_id: String,
) -> Result<chronacle_extraction::codex_service::CodexStatus, String> {
    chronacle_extraction::codex_service::codex_status(&state.db, &collection_id).await
}

/// Cancel the in-flight codex compile task, if any.
#[tauri::command]
pub async fn cancel_compile(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::commands::cancel_chat_task(&state.compile_task).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn cancel_compile_aborts_registered_task_and_empties_slot() {
        let slot: tokio::sync::Mutex<Option<tokio::task::AbortHandle>> =
            tokio::sync::Mutex::new(None);
        let task = tokio::spawn(async {
            loop {
                tokio::task::yield_now().await;
            }
        });
        *slot.lock().await = Some(task.abort_handle());

        assert!(crate::commands::cancel_chat_task(&slot).await);
        let err = task.await.expect_err("task should have been aborted");
        assert!(err.is_cancelled());
        assert!(!crate::commands::cancel_chat_task(&slot).await);
    }
}
