use std::sync::Arc;

use serde::Deserialize;
use tauri::{Emitter, State};

use crate::AppState;

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
        dyn Fn(chronacle_ingestion::ingestion_service::IngestionProgress) + Send + Sync,
    > = std::sync::Arc::new(
        move |p: chronacle_ingestion::ingestion_service::IngestionProgress| {
            let _ = handle.emit(
                "ingestion-progress",
                serde_json::json!({
                    "source_id": sid,
                    "status": "indexing",
                    "progress": p.fraction,
                    "step": p.step,
                    "current": p.current,
                    "total": p.total,
                }),
            );
        },
    );

    let state_ref = state.inner().clone();
    let sid = source_id.clone();

    match chronacle_ingestion::ingestion_service::ingest_source(
        &state_ref.db,
        &state_ref.blob_store,
        &state_ref.pdf_extractor,
        &state_ref.embedding_provider,
        &state_ref.vector_store,
        &sid,
        on_progress,
    )
    .await
    {
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
