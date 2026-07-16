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
    let outbound = state_ref.outbound.read().await.clone();

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
        for r in &articles.compiled_refs {
            outbound.enqueue(r.clone());
        }

        let rules = chronacle_extraction::codex_service::compile_rules(
            &task_state.db,
            &llm,
            &embed,
            &task_collection,
            move |p| emit_progress(&app, &p),
        )
        .await?;
        for r in &rules.compiled_refs {
            outbound.enqueue(r.clone());
        }

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
    let outbound = state_ref.outbound.read().await.clone();

    let compiled = chronacle_extraction::codex_service::compile_entity(
        &state_ref.db,
        &llm,
        &embed,
        &vector_store,
        &kind,
        &id,
    )
    .await
    .map_err(|e| e.to_string())?;
    if compiled {
        outbound.enqueue(chronacle_core::VaultRef {
            table: kind.clone(),
            id: id.clone(),
        });
    }
    Ok(compiled)
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

/// Distill an assistant chat answer into pending codex proposals
/// ("Save to Codex"). Returns how many proposals were created.
#[tauri::command]
pub async fn save_chat_to_codex(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    content: String,
) -> Result<usize, String> {
    let state_ref = state.inner().clone();
    let llm = state_ref
        .llm_provider
        .read()
        .map_err(|e| format!("LLM lock: {e}"))?
        .clone();
    chronacle_extraction::codex_service::distill_chat_answer(
        &state_ref.db,
        &llm,
        &campaign_id,
        &content,
    )
    .await
    .map_err(|e| e.to_string())
}

/// List codex proposals, optionally filtered by status ('pending' etc.).
#[tauri::command]
pub async fn get_proposals(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
) -> Result<Vec<chronacle_extraction::codex_service::CodexProposal>, String> {
    chronacle_extraction::codex_service::list_proposals(&state.db, status.as_deref()).await
}

/// Accept a proposal: apply it, append provenance, re-embed, resolve.
#[tauri::command]
pub async fn accept_proposal(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let state_ref = state.inner().clone();
    let embed = state_ref
        .embedding_provider
        .read()
        .map_err(|e| format!("Embed lock: {e}"))?
        .clone();
    let outbound = state_ref.outbound.read().await.clone();
    let refs =
        chronacle_extraction::codex_service::accept_proposal(&state_ref.db, &embed, &id).await?;
    for r in refs {
        outbound.enqueue(r);
    }
    Ok(())
}

/// Reject a proposal without applying it.
#[tauri::command]
pub async fn reject_proposal(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    chronacle_extraction::codex_service::reject_proposal(&state.db, &id).await
}

/// Pending proposals + unresolved lint findings (sidebar badge).
#[tauri::command]
pub async fn get_maintenance_counts(
    state: State<'_, Arc<AppState>>,
) -> Result<chronacle_extraction::codex_service::MaintenanceCounts, String> {
    chronacle_extraction::codex_service::maintenance_counts(&state.db).await
}

/// Run the manual lint pass over a campaign's full scope ("Check campaign").
#[tauri::command]
pub async fn run_lint(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<chronacle_extraction::codex_service::LintSummary, String> {
    chronacle_extraction::codex_service::run_lint_campaign(&state.db, &campaign_id).await
}

/// Unresolved lint findings for the Maintenance inbox.
#[tauri::command]
pub async fn get_lint_findings(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<chronacle_extraction::codex_service::LintFinding>, String> {
    chronacle_extraction::codex_service::list_lint_findings(&state.db).await
}

/// Mark one lint finding resolved.
#[tauri::command]
pub async fn resolve_lint_finding(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    chronacle_extraction::codex_service::resolve_lint_finding(&state.db, &id).await
}

/// Resolve a naming conflict: keep the disputed term on `keep_id` and strip it
/// from `drop_id` (whose claim must be an alias, not its primary name).
#[tauri::command]
pub async fn resolve_alias_collision(
    state: State<'_, Arc<AppState>>,
    finding_id: String,
    keep_id: String,
    drop_id: String,
) -> Result<(), String> {
    chronacle_extraction::codex_service::resolve_alias_collision(
        &state.db,
        &finding_id,
        &keep_id,
        &drop_id,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: all new proposal command functions are referenced so the
    /// compiler verifies their signatures, imports, and return types.
    #[test]
    fn proposal_commands_module_compiles() {
        let _ = save_chat_to_codex as fn(_, _, _) -> _;
        let _ = get_proposals as fn(_, _) -> _;
        let _ = accept_proposal as fn(_, _) -> _;
        let _ = reject_proposal as fn(_, _) -> _;
        let _ = get_maintenance_counts as fn(_) -> _;
        let _ = run_lint as fn(_, _) -> _;
        let _ = get_lint_findings as fn(_) -> _;
        let _ = resolve_lint_finding as fn(_, _) -> _;
        let _ = resolve_alias_collision as fn(_, _, _, _) -> _;
    }

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
