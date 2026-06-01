/// Ingestion service — orchestrates PDF extraction, chunking, and embedding.
///
/// Phase 1 provides the full pipeline skeleton; the actual PDF-extraction and
/// embedding calls are stubbed and will be wired in a later iteration.
use std::sync::Arc;

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

/// Metadata fetched from the source record before ingestion begins.
pub(crate) struct SourceInfo {
    pub(crate) filename: String,
    /// Every source must belong to a collection — non-nullable per schema.
    pub(crate) collection_id: String,
}

/// Ingest a source PDF: extract text, chunk, embed, and store.
///
/// 1. Reads the raw PDF bytes from the blob store.
/// 2. Extracts text with `pdfium-render` (stubbed).
/// 3. Splits into chunks (stubbed — returns empty set).
/// 4. Embeds each chunk with `fastembed` (stubbed — returns empty set).
/// 5. Stores chunks in the vector store and marks `index_status = 'done'`.
pub async fn ingest_source(state: &Arc<AppState>, source_id: &str) -> Result<(), IngestionError> {
    // ── 1. Update status to 'indexing' ──────────────────────────────
    state
        .db
        .query("UPDATE source SET index_status = 'indexing' WHERE id = $id")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(format!("Failed to update index_status: {e}")))?;

    // ── 2. Read source metadata and blob ──────────────────────────────
    let source_info = get_source_info(&state.db, source_id).await?;
    let pdf_data = state
        .blob_store
        .retrieve(source_id, &source_info.filename)
        .await
        .map_err(|e| IngestionError::Store(e.to_string()))?;

    // ── 3. Extract text ────────────────────────────────────────────
    let extracted = extract_text(&pdf_data).await?;

    // ── 4. Chunk ───────────────────────────────────────────────────
    let chunks = chunk_text(&extracted, source_id).await?;

    // ── 5. Embed ───────────────────────────────────────────────────
    let indexed = embed_chunks(chunks, &source_info.collection_id).await?;

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

/// Metadata from text extraction.
struct ExtractedDoc {
    page_count: usize,
    _text: String,
    _pages: Vec<PageContent>,
}

struct PageContent {
    _page_num: usize,
    _text: String,
}

/// Extract text from PDF bytes using `pdfium-render`.
async fn extract_text(_data: &[u8]) -> Result<ExtractedDoc, IngestionError> {
    // TODO: Phase 2 — implement with pdfium-render
    Ok(ExtractedDoc {
        page_count: 1,
        _text: String::new(),
        _pages: vec![],
    })
}

/// Split extracted text into searchable chunks.
async fn chunk_text(
    _doc: &ExtractedDoc,
    _source_id: &str,
) -> Result<Vec<RawChunk>, IngestionError> {
    // TODO: Phase 2 — implement semantic / sliding-window chunking
    Ok(Vec::new())
}

struct RawChunk {
    _text: String,
    _page_start: i64,
    _page_end: i64,
    _section_heading: String,
}

/// Embed each chunk using `fastembed`, tagging each with `collection_id`.
///
/// `collection_id` is non-optional: every source must belong to a collection
/// per the schema (`source.collection TYPE record<collection>`), and this
/// value is inherited by every chunk produced from that source.
async fn embed_chunks(
    _chunks: Vec<RawChunk>,
    collection_id: &str,
) -> Result<Vec<crate::providers::vector_store::IndexedChunk>, IngestionError> {
    // TODO: Phase 2 — implement with fastembed (nomic-embed-text-v1.5)
    //
    // Each IndexedChunk will carry `collection_id: collection_id.to_owned()`
    // so the vector store can enforce the collection filter at search time.
    let _ = collection_id; // used in Phase 2 implementation
    Ok(Vec::new())
}

/// Fetch the filename and collection ID for a source record.
///
/// Exposed as `pub(crate)` so the `#[cfg(test)]` block below can call it
/// directly without going through the full `ingest_source` pipeline.
pub(crate) async fn get_source_info<C>(
    db: &surrealdb::Surreal<C>,
    source_id: &str,
) -> Result<SourceInfo, IngestionError>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT filename, collection FROM source WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(format!("Failed to query source: {e}")))?;

    #[derive(serde::Deserialize)]
    struct Row {
        filename: String,
        /// Non-optional: matches `source.collection TYPE record<collection>` schema.
        collection: surrealdb::sql::Thing,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    rows.into_iter()
        .next()
        .map(|r| SourceInfo {
            filename: r.filename,
            collection_id: r.collection.id.to_raw(),
        })
        .ok_or_else(|| IngestionError::Db(format!("Source '{source_id}' not found")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_source_info_reads_collection_id() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE collection SET id='col1', name='Test', created_at=time::now(), updated_at=time::now()"
        ).await.unwrap();
        db.query(
            "CREATE source SET id='src1', collection=type::thing('collection','col1'), \
             filename='test.pdf', display_name='Test', source_type='rules', page_count=0, \
             indexed_at=time::now(), index_status='pending', embed_model='nomic-embed-text-v1.5'",
        )
        .await
        .unwrap();

        let info = get_source_info(&db, "src1").await.unwrap();
        assert_eq!(info.filename, "test.pdf");
        assert_eq!(info.collection_id.as_str(), "col1");
    }
}
