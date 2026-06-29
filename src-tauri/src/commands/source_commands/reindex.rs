use std::sync::Arc;

use tauri::{Emitter, State};

use crate::AppState;

use super::query::list_all_source_ids;

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
            dyn Fn(chronacle_ingestion::ingestion_service::IngestionProgress) + Send + Sync,
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
        chronacle_ingestion::ingestion_service::ingest_source(
            &state_ref.db,
            &state_ref.blob_store,
            &state_ref.pdf_extractor,
            &state_ref.embedding_provider,
            &state_ref.vector_store,
            sid,
            on_progress,
        )
        .await
        .map_err(|e| format!("re-ingest {sid}: {e}"))?;
    }

    Ok(total)
}
