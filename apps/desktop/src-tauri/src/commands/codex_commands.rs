use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use tauri::State;

use crate::AppState;
use chronacle_extraction::codex_service::{CompileProgress, RuleEntry};

/// Summary returned to the frontend when a codex compile run completes.
#[derive(Debug, Clone, Serialize)]
pub struct CompileSummary {
    pub articles_compiled: usize,
    pub remaining_stale: usize,
    pub entries_created: usize,
    pub entries_updated: usize,
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

    let task: tokio::task::JoinHandle<
        Result<
            (
                chronacle_extraction::codex_service::CompileResult,
                chronacle_extraction::codex_service::RulesCompileResult,
            ),
            chronacle_extraction::codex_service::CodexError,
        >,
    > = tokio::spawn(async move {
        let article_app = app.clone();
        let articles = chronacle_extraction::codex_service::compile_collection(
            &task_state.db,
            &llm,
            &embed,
            &vector_store,
            &task_collection,
            move |p| emit_progress(&article_app, &p),
        )
        .await?;

        let rules = chronacle_extraction::codex_service::compile_rules(
            &task_state.db,
            &llm,
            &embed,
            &task_collection,
            move |p| emit_progress(&app, &p),
        )
        .await?;

        Ok((articles, rules))
    });

    *state.compile_task.lock().await = Some(task.abort_handle());

    match task.await {
        Ok(Ok((articles, rules))) => Ok(CompileSummary {
            articles_compiled: articles.articles_compiled,
            remaining_stale: articles.remaining_stale,
            entries_created: rules.entries_created,
            entries_updated: rules.entries_updated,
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

/// List all compiled rule entries for a collection (drives the Rules UI).
#[tauri::command]
pub async fn get_rule_entries(
    state: State<'_, Arc<AppState>>,
    collection_id: String,
) -> Result<Vec<RuleEntry>, String> {
    chronacle_extraction::codex_service::list_rule_entries(&state.db, &collection_id).await
}

/// Update a rule entry's freeform GM notes.
#[tauri::command]
pub async fn update_rule_notes(
    state: State<'_, Arc<AppState>>,
    id: String,
    notes: Option<String>,
) -> Result<(), String> {
    chronacle_extraction::codex_service::update_rule_notes(&state.db, &id, notes).await
}

/// Regenerate a single rule entry honoring a new GM objection. Runs inline —
/// single-entry latency is acceptable, so no abort handle is needed.
#[tauri::command]
pub async fn redo_rule_entry(
    state: State<'_, Arc<AppState>>,
    id: String,
    objection: String,
) -> Result<(), String> {
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

    chronacle_extraction::codex_service::redo_rule_entry(
        &state_ref.db,
        &llm,
        &embed,
        &id,
        &objection,
    )
    .await
    .map_err(|e| e.to_string())
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
