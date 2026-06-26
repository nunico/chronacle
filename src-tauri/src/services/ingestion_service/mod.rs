/// Ingestion service — orchestrates PDF extraction, chunking, and embedding.
///
/// Phase 1: extracts text via the `PdfExtractor` trait (backed by `pdfium-render`),
/// normalizes PDF artifacts, chunks into sentence-aware ~250-token chunks, embeds
/// via fastembed with Nomic task prefixes, and stores in SurrealDB.
///
/// Progress reporting: every pipeline stage calls `on_progress` with a fractional
/// progress (0.0–1.0) and a human-readable step label. The caller is responsible
/// for forwarding these to the UI (e.g. via Tauri events).
use std::sync::Arc;

use crate::AppState;

mod db;
mod pipeline;
mod types;

pub(crate) use db::get_source_info;
pub use types::{IngestionError, IngestionProgress};

use pipeline::{
    chunk_text, embed_chunks, normalize_extracted, EXTRACT_FRACTION_END, EXTRACT_FRACTION_START,
};

/// Ingest a source PDF: extract text, chunk, embed, and store.
///
/// `on_progress` is called after each stage with the current progress fraction
/// (0.0–1.0) and a human-readable step label.
///
/// On any failure, this function marks `source.index_status = 'failed'` and
/// deletes orphan chunks written before the error so a retry starts from a
/// clean slate (architecture.md Phase 1: "cleanup partial chunks on failure").
pub async fn ingest_source(
    state: &Arc<AppState>,
    source_id: &str,
    on_progress: Arc<dyn Fn(IngestionProgress) + Send + Sync>,
) -> Result<(), IngestionError> {
    let result = ingest_source_inner(state, source_id, on_progress).await;
    if let Err(e) = &result {
        eprintln!("Ingestion failed for source {source_id}: {e}");
        if let Err(cleanup_err) = db::mark_failed_and_cleanup(&state.db, source_id).await {
            eprintln!(
                "Ingestion cleanup also failed for {source_id}: {cleanup_err} (original error: {e})"
            );
        }
    }
    result
}

async fn ingest_source_inner(
    state: &Arc<AppState>,
    source_id: &str,
    on_progress: Arc<dyn Fn(IngestionProgress) + Send + Sync>,
) -> Result<(), IngestionError> {
    on_progress(IngestionProgress::stage(0.02, "Reading source metadata"));

    state
        .db
        .query("UPDATE source SET index_status = 'indexing' WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(format!("Failed to update index_status: {e}")))?;

    let source_info = get_source_info(&state.db, source_id).await?;

    on_progress(IngestionProgress::stage(
        0.05,
        "Loading PDF file from storage",
    ));
    let pdf_data = state
        .blob_store
        .retrieve(source_id, &source_info.filename)
        .await
        .map_err(|e| IngestionError::Store(e.to_string()))?;

    on_progress(IngestionProgress::stage(
        EXTRACT_FRACTION_START,
        "Extracting text from PDF pages",
    ));
    let page_progress = on_progress.clone();
    let on_page: crate::services::pdf_extractor::PageProgressFn =
        Arc::new(move |page: usize, total: usize| {
            let span = EXTRACT_FRACTION_END - EXTRACT_FRACTION_START;
            let fraction = if total == 0 {
                EXTRACT_FRACTION_END
            } else {
                EXTRACT_FRACTION_START + span * (page as f32 / total as f32)
            };
            page_progress(IngestionProgress::counted(
                fraction,
                format!("Extracting text from page {page}/{total}"),
                page as u32,
                total as u32,
            ));
        });
    let extracted = state
        .pdf_extractor
        .extract_with_progress(&pdf_data, on_page)
        .await
        .map_err(|e| IngestionError::PdfExtraction(e.to_string()))?;
    let extracted = normalize_extracted(&extracted);

    on_progress(IngestionProgress::stage(
        0.25,
        format!(
            "Splitting {} pages into searchable chunks",
            extracted.page_count
        ),
    ));
    let chunks = chunk_text(&extracted, source_id)?;
    let chunk_count = chunks.len();
    on_progress(IngestionProgress::counted(
        0.28,
        format!("Split into {chunk_count} chunks"),
        chunk_count as u32,
        chunk_count as u32,
    ));

    let embed_provider = state
        .embedding_provider
        .read()
        .map_err(|e| IngestionError::Db(format!("Embedding lock: {e}")))?
        .clone();
    let indexed = embed_chunks(
        &embed_provider,
        chunks,
        source_id,
        &source_info.collection_id,
        on_progress.as_ref(),
    )
    .await?;

    drop(embed_provider);

    on_progress(IngestionProgress::stage(
        0.85,
        format!("Writing {chunk_count} chunks to database"),
    ));
    state
        .vector_store
        .upsert(source_id, &indexed)
        .await
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    on_progress(IngestionProgress::stage(0.98, "Finalizing indexing"));
    state
        .db
        .query("UPDATE source SET index_status = 'done', page_count = $page_count WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .bind(("page_count", extracted.page_count as i64))
        .await
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests;
