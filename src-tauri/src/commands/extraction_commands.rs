use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;
use tauri::State;

use crate::services::extraction_service::ExtractionProgress;
use crate::AppState;

/// Summary returned to the frontend when extraction completes.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractionSummary {
    pub entities_created: usize,
    pub relations_created: usize,
}

/// Emit a phased progress event to the frontend.
fn emit_progress(app: &tauri::AppHandle, p: &ExtractionProgress) {
    let _ = app.emit("extract-progress", p);
}

/// Build the persisted `extraction` chat-message body for a finished run.
///
/// Stored as JSON so the chat thread can re-render the [`ExtractionCard`] after
/// navigation instead of losing the result (it lived only in transient UI
/// state). `name` is `Some` for a single-entity extraction, `None` for a full
/// sweep. Mirrors the live card's wording in `OracleView.runExtraction`.
fn extraction_summary_content(name: Option<&str>, entities: usize, relations: usize) -> String {
    let (status, title, detail) = if entities == 0 {
        let detail = match name {
            Some(n) => format!("No passages found for \"{n}\""),
            None => "No passages found".to_string(),
        };
        ("empty", "Nothing found", detail)
    } else {
        (
            "done",
            "Extraction complete",
            format!("Created {entities} entities, {relations} relations"),
        )
    };
    serde_json::json!({
        "status": status,
        "title": title,
        "detail": detail,
        "entitiesFound": entities,
        "relationsFound": relations,
    })
    .to_string()
}

/// Persist a finished extraction as an `extraction`-role message bound to the
/// campaign, so it survives navigating away from and back to the chat.
async fn persist_extraction_summary(
    db: &surrealdb::Surreal<impl surrealdb::Connection>,
    campaign_id: &str,
    name: Option<&str>,
    entities: usize,
    relations: usize,
) {
    let content = extraction_summary_content(name, entities, relations);
    if let Err(e) = crate::services::agent_service::persist_message(
        db,
        "extraction",
        &content,
        false,
        Some(campaign_id),
    )
    .await
    {
        eprintln!("extraction: failed to persist summary message: {e}");
    }
}

/// Seed-anchored extraction of a single named entity across the active
/// campaign's collections. Runs as an abortable spawned task.
#[tauri::command]
pub async fn extract_entity_by_name(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    name: String,
) -> Result<ExtractionSummary, String> {
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
    let task_campaign = campaign_id.clone();
    let task_name = name.clone();

    let task = tokio::spawn(async move {
        crate::services::extraction_service::extract_seed_anchored(
            &task_state.db,
            &llm,
            &embed,
            &vector_store,
            &task_campaign,
            &task_name,
            move |p| emit_progress(&app, &p),
        )
        .await
    });

    *state.extract_task.lock().await = Some(task.abort_handle());

    match task.await {
        Ok(Ok(result)) => {
            persist_extraction_summary(
                &state_ref.db,
                &campaign_id,
                Some(&name),
                result.entities_created,
                result.relations_created,
            )
            .await;
            Ok(ExtractionSummary {
                entities_created: result.entities_created,
                relations_created: result.relations_created,
            })
        }
        Ok(Err(e)) => Err(format!("Extraction failed: {e}")),
        Err(join_err) if join_err.is_cancelled() => Err("cancelled".to_string()),
        Err(join_err) => Err(format!("Extraction task error: {join_err}")),
    }
}

/// Full sweep across every collection linked to the campaign. Abortable.
#[tauri::command]
pub async fn extract_all_from_campaign(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<ExtractionSummary, String> {
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
    let app = app_handle.clone();
    let task_state = state_ref.clone();
    let task_campaign = campaign_id.clone();

    let task = tokio::spawn(async move {
        let cids =
            crate::services::agent_service::resolve_collection_ids(&task_state.db, &task_campaign)
                .await
                .map_err(|e| e.to_string())?;

        let mut entities_created = 0usize;
        let mut relations_created = 0usize;
        for cid in cids {
            let app = app.clone();
            let ec = entities_created;
            let rc = relations_created;
            let result = crate::services::extraction_service::extract_from_collection(
                &task_state.db,
                &llm,
                &embed,
                &cid,
                move |mut p| {
                    // Make counts cumulative across collections.
                    p.entities_found += ec;
                    p.relations_found += rc;
                    emit_progress(&app, &p);
                },
            )
            .await
            .map_err(|e| e.to_string())?;
            entities_created += result.entities_created;
            relations_created += result.relations_created;
        }
        Ok::<_, String>((entities_created, relations_created))
    });

    *state.extract_task.lock().await = Some(task.abort_handle());

    match task.await {
        Ok(Ok((entities_created, relations_created))) => {
            persist_extraction_summary(
                &state_ref.db,
                &campaign_id,
                None,
                entities_created,
                relations_created,
            )
            .await;
            Ok(ExtractionSummary {
                entities_created,
                relations_created,
            })
        }
        Ok(Err(e)) => Err(format!("Extraction failed: {e}")),
        Err(join_err) if join_err.is_cancelled() => Err("cancelled".to_string()),
        Err(join_err) => Err(format!("Extraction task error: {join_err}")),
    }
}

/// Cancel the in-flight extraction task, if any.
#[tauri::command]
pub async fn cancel_extraction(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    crate::commands::cancel_chat_task(&state.extract_task).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::extraction_summary_content;

    #[test]
    fn summary_content_done_carries_counts_and_status() {
        let v: serde_json::Value =
            serde_json::from_str(&extraction_summary_content(Some("Varn"), 5, 3)).unwrap();
        assert_eq!(v["status"], "done");
        assert_eq!(v["title"], "Extraction complete");
        assert_eq!(v["detail"], "Created 5 entities, 3 relations");
        assert_eq!(v["entitiesFound"], 5);
        assert_eq!(v["relationsFound"], 3);
    }

    #[test]
    fn summary_content_empty_when_nothing_created() {
        let named: serde_json::Value =
            serde_json::from_str(&extraction_summary_content(Some("Varn"), 0, 0)).unwrap();
        assert_eq!(named["status"], "empty");
        assert_eq!(named["title"], "Nothing found");
        assert_eq!(named["detail"], "No passages found for \"Varn\"");

        let sweep: serde_json::Value =
            serde_json::from_str(&extraction_summary_content(None, 0, 0)).unwrap();
        assert_eq!(sweep["detail"], "No passages found");
    }

    #[tokio::test]
    async fn cancel_extraction_aborts_registered_task_and_empties_slot() {
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
