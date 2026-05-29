/// Ingestion service — orchestrates PDF extraction, chunking, and embedding.
///
/// Phase 1 provides the full pipeline skeleton; the actual PDF-extraction and
/// embedding calls are stubbed and will be wired in a later iteration.

use std::sync::Arc;

use crate::providers::vector_store::IndexedChunk;
use crate::services::chunker::{chunk_document, ExtractedDoc, PageContent};
use crate::AppState;
use surrealdb::Connection;

/// Errors that can arise during ingestion.
#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF extraction error: {0}")]
    PdfExtraction(String),
    #[error("Embedding error: {0}")]
    Embedding(String),
    #[error("Database error: {0}")]
    Db(String),
    #[error("Store error: {0}")]
    Store(String),
}

/// Ingest a source PDF: extract text, chunk, embed, and store.
///
/// 1. Reads the raw PDF bytes from the blob store.
/// 2. Extracts text with `pdfium-render` (stubbed).
/// 3. Splits into chunks (stubbed — returns empty set).
/// 4. Embeds each chunk with `fastembed` (stubbed — returns empty set).
/// 5. Stores chunks in the vector store and marks `index_status = 'done'`.
pub async fn ingest_source(
    state: &Arc<AppState>,
    source_id: &str,
) -> Result<(), IngestionError> {
    // ── 1. Update status to 'indexing' ──────────────────────────────
    state
        .db
        .query("UPDATE source SET index_status = 'indexing' WHERE id = $id")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(
            format!("Failed to update index_status: {e}")
        ))?;

    // ── 2. Read from blob store ────────────────────────────────────
    let filename = get_source_filename(&state.db, source_id).await?;
    let pdf_data = state
        .blob_store
        .retrieve(source_id, &filename)
        .await
        .map_err(|e| IngestionError::Store(e.to_string()))?;

    // ── 3. Extract text ────────────────────────────────────────────
    let extracted = extract_text(&pdf_data).await?;

    // ── 4. Chunk ───────────────────────────────────────────────────
    let chunks = chunk_text(&extracted, source_id).await?;

    // ── 5. Embed ───────────────────────────────────────────────────
    let indexed = embed_chunks(chunks).await?;

    // ── 6. Store in vector store ───────────────────────────────────
    state
        .vector_store
        .upsert(source_id, &indexed)
        .await
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    // ── 7. Mark done ───────────────────────────────────────────────
    state
        .db
        .query("UPDATE source SET index_status = 'done', page_count = $page_count WHERE id = $id")
        .bind(("id", source_id.to_owned()))
        .bind(("page_count", extracted.page_count as i64))
        .await
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    Ok(())
}

/// Extract text from PDF bytes using `pdfium-render`.
async fn extract_text(_data: &[u8]) -> Result<ExtractedDoc, IngestionError> {
    // TODO: Phase 2 — implement with pdfium-render
    Ok(ExtractedDoc {
        page_count: 1,
        text: String::new(),
        pages: vec![],
    })
}

/// Split extracted document into searchable chunks using sliding-window.
async fn chunk_text(
    doc: &ExtractedDoc,
    _source_id: &str,
) -> Result<Vec<RawChunk>, IngestionError> {
    let chunks = chunk_document(doc);
    Ok(chunks
        .into_iter()
        .map(|c| RawChunk {
            text: c.text,
            page_start: c.page_start,
            page_end: c.page_end,
            section_heading: c.section_heading,
        })
        .collect())
}

struct RawChunk {
    text: String,
    page_start: i64,
    page_end: i64,
    section_heading: String,
}

/// Embed each chunk using `fastembed`.
async fn embed_chunks(
    _chunks: Vec<RawChunk>,
) -> Result<Vec<IndexedChunk>, IngestionError> {
    // TODO: Phase 2 — implement with fastembed (nomic-embed-text-v1.5)
    Ok(Vec::new())
}

/// Helper: fetch the filename for a source record.
async fn get_source_filename<C>(
    db: &surrealdb::Surreal<C>,
    source_id: &str,
) -> Result<String, IngestionError>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT filename FROM source WHERE id = $id")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(
            format!("Failed to query source filename: {e}")
        ))?;

    #[derive(serde::Deserialize)]
    struct Row {
        filename: String,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    rows.into_iter()
        .next()
        .map(|r| r.filename)
        .ok_or_else(|| IngestionError::Db(format!("Source '{source_id}' not found")))
}
