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

use crate::providers::embedding::EmbeddingProvider;
use crate::providers::vector_store::IndexedChunk;
use crate::services::chunker::{chunk_document, ExtractedDoc, PageContent};
use crate::AppState;
use surrealdb::Connection;

/// A progress update emitted during ingestion.
///
/// `fraction` advances from 0.0 to 1.0 across all stages.
/// `step` is a human-readable label like "Extracting text from PDF".
/// `current`/`total` carry item counts for batched stages (e.g. embedding
/// "64 of 120 chunks") so the UI can show fine-grained activity. They are
/// `None` for single-shot stages that have no countable unit of work.
#[derive(Debug, Clone)]
pub struct IngestionProgress {
    pub fraction: f32,
    pub step: String,
    pub current: Option<u32>,
    pub total: Option<u32>,
}

impl IngestionProgress {
    /// A single-shot stage with no countable work (extraction, DB write, …).
    fn stage(fraction: f32, step: impl Into<String>) -> Self {
        Self {
            fraction,
            step: step.into(),
            current: None,
            total: None,
        }
    }

    /// A batched stage reporting `current`/`total` items processed so far.
    fn counted(fraction: f32, step: impl Into<String>, current: u32, total: u32) -> Self {
        Self {
            fraction,
            step: step.into(),
            current: Some(current),
            total: Some(total),
        }
    }
}

/// Text extraction reports per-page progress across this fraction range,
/// interpolated linearly by page number.
const EXTRACT_FRACTION_START: f32 = 0.08;
const EXTRACT_FRACTION_END: f32 = 0.20;

/// Chunks are embedded in batches of this size so the UI sees steady progress
/// through what is otherwise the longest, opaque stage of ingestion.
const EMBED_BATCH_SIZE: usize = 32;
/// The embedding stage spans this fraction range; per-batch progress is
/// interpolated linearly across it.
const EMBED_FRACTION_START: f32 = 0.30;
const EMBED_FRACTION_END: f32 = 0.85;

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
/// `on_progress` is called after each stage with the current progress fraction
/// (0.0–1.0) and a human-readable step label.
///
/// On any failure, this function marks `source.index_status = 'failed'` and
/// deletes orphan chunks written before the error so a retry starts from a
/// clean slate (architecture.md Phase 1: "cleanup partial chunks on failure").
/// True resume from the last successful page batch is intentionally deferred —
/// "cleanup + retry" is the simplest correct behavior for Phase 1.
pub async fn ingest_source(
    state: &Arc<AppState>,
    source_id: &str,
    on_progress: Arc<dyn Fn(IngestionProgress) + Send + Sync>,
) -> Result<(), IngestionError> {
    let result = ingest_source_inner(state, source_id, on_progress).await;
    if let Err(e) = &result {
        eprintln!("Ingestion failed for source {source_id}: {e}");
        if let Err(cleanup_err) = mark_failed_and_cleanup(&state.db, source_id).await {
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
        source_info.is_gm_only,
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

/// Apply [`text_normalizer::normalize`] to every page and rebuild the merged
/// full text. This repairs PDF extraction artifacts (soft hyphens, intra-paragraph
/// newlines) before chunking so that embeddings see clean prose.
fn normalize_extracted(doc: &ExtractedDoc) -> ExtractedDoc {
    use crate::services::text_normalizer::normalize;
    let pages: Vec<PageContent> = doc
        .pages
        .iter()
        .map(|p| PageContent {
            page_num: p.page_num,
            text: normalize(&p.text),
        })
        .collect();
    let mut full = String::new();
    for (i, p) in pages.iter().enumerate() {
        if i > 0 && !p.text.is_empty() && !full.is_empty() {
            full.push('\n');
        }
        full.push_str(&p.text);
    }
    ExtractedDoc {
        page_count: pages.len(),
        text: full,
        pages,
    }
}

// PDF extraction now goes through the `PdfExtractor` trait on `AppState`
// (see `src/services/pdf_extractor.rs`). The previous `pdf-extract`-based
// implementation was replaced with `pdfium-render` for layout-aware extraction.

fn chunk_text(doc: &ExtractedDoc, _source_id: &str) -> Result<Vec<RawChunk>, IngestionError> {
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

/// Information about a source needed during ingestion.
#[derive(Debug)]
pub(crate) struct SourceInfo {
    pub(crate) filename: String,
    /// Every source must belong to a collection — non-nullable per schema.
    pub(crate) collection_id: String,
    /// GM-secret flag; chunks inherit it so it propagates into the vector index.
    pub(crate) is_gm_only: bool,
}

/// Embed chunks using the embedding provider, tagging each with the source's collection.
///
/// Embedding is the longest stage of ingestion, so chunks are processed in
/// batches of [`EMBED_BATCH_SIZE`] and `on_progress` is called after each batch
/// with a running `current`/`total` count. The reported fraction is
/// interpolated linearly across [`EMBED_FRACTION_START`]..[`EMBED_FRACTION_END`].
async fn embed_chunks(
    provider: &Arc<dyn EmbeddingProvider>,
    chunks: Vec<RawChunk>,
    source_id: &str,
    collection_id: &str,
    is_gm_only: bool,
    on_progress: &(dyn Fn(IngestionProgress) + Send + Sync),
) -> Result<Vec<IndexedChunk>, IngestionError> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let total = chunks.len();
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();

    on_progress(IngestionProgress::counted(
        EMBED_FRACTION_START,
        format!("Embedding chunks 0/{total}"),
        0,
        total as u32,
    ));

    let mut embeddings: Vec<Vec<f32>> = Vec::with_capacity(total);
    for batch in texts.chunks(EMBED_BATCH_SIZE) {
        let batch_embeddings = provider
            .embed_documents(batch.to_vec())
            .await
            .map_err(|e| IngestionError::Embedding(e.to_string()))?;
        embeddings.extend(batch_embeddings);

        let done = embeddings.len();
        let span = EMBED_FRACTION_END - EMBED_FRACTION_START;
        let fraction = EMBED_FRACTION_START + span * (done as f32 / total as f32);
        on_progress(IngestionProgress::counted(
            fraction,
            format!("Embedding chunks {done}/{total}"),
            done as u32,
            total as u32,
        ));
    }

    let embed_model = provider.model_name().to_string();
    let cid = collection_id.to_owned();

    Ok(chunks
        .into_iter()
        .zip(embeddings)
        .map(|(chunk, embedding)| IndexedChunk {
            chunk_id: format!("{}-{}", source_id, uuid::Uuid::new_v4()),
            collection_id: cid.clone(),
            text: chunk.text,
            page_start: chunk.page_start,
            page_end: chunk.page_end,
            section_heading: chunk.section_heading,
            source_type: String::new(),
            is_gm_only,
            embedding,
            embed_model: embed_model.clone(),
        })
        .collect())
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
        .query(
            "SELECT filename, collection, is_gm_only FROM source \
             WHERE id = type::thing('source', $id)",
        )
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(format!("Failed to query source: {e}")))?;

    #[derive(serde::Deserialize)]
    struct Row {
        filename: String,
        /// Non-optional: matches `source.collection TYPE record<collection>` schema.
        collection: surrealdb::sql::Thing,
        #[serde(default)]
        is_gm_only: bool,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    rows.into_iter()
        .next()
        .map(|r| SourceInfo {
            filename: r.filename,
            collection_id: r.collection.id.to_raw(),
            is_gm_only: r.is_gm_only,
        })
        .ok_or_else(|| IngestionError::Db(format!("Source '{source_id}' not found")))
}

/// Mark a source as `failed` and delete any chunks already written for it.
///
/// Called from the error path of `ingest_source` so a retry starts from a
/// clean slate. If the source row was already deleted by the caller (e.g.
/// `delete_source` racing with a failed ingest), the UPDATE is a no-op.
async fn mark_failed_and_cleanup<C>(
    db: &surrealdb::Surreal<C>,
    source_id: &str,
) -> Result<(), IngestionError>
where
    C: Connection,
{
    db.query(
        "UPDATE source SET index_status = 'error' WHERE id = type::thing('source', $id); \
         DELETE chunk WHERE source = type::thing('source', $id)",
    )
    .bind(("id", source_id.to_owned()))
    .await
    .map_err(|e| IngestionError::Db(format!("cleanup query failed: {e}")))?
    .check()
    .map_err(|e| IngestionError::Db(format!("cleanup statement failed: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_extracted_removes_soft_hyphen_artifacts() {
        let raw = ExtractedDoc {
            page_count: 1,
            text: "power-\nful descen-\ndents of\nthe captain family".to_string(),
            pages: vec![PageContent {
                page_num: 1,
                text: "power-\nful descen-\ndents of\nthe captain family".to_string(),
            }],
        };
        let normalized = normalize_extracted(&raw);
        assert!(
            !normalized.text.contains("-\n"),
            "soft hyphens not removed: {:?}",
            normalized.text
        );
        assert!(normalized.text.contains("powerful"));
        assert!(normalized.text.contains("descendents"));
        assert_eq!(normalized.pages[0].text, normalized.text);
        assert_eq!(normalized.page_count, 1);
    }

    #[test]
    fn normalize_extracted_preserves_page_boundaries() {
        let p1 = "First page paragraph.";
        let p2 = "Second page paragraph.";
        let raw = ExtractedDoc {
            page_count: 2,
            text: format!("{p1}\n{p2}"),
            pages: vec![
                PageContent {
                    page_num: 1,
                    text: p1.to_string(),
                },
                PageContent {
                    page_num: 2,
                    text: p2.to_string(),
                },
            ],
        };
        let normalized = normalize_extracted(&raw);
        assert_eq!(normalized.pages.len(), 2);
        assert_eq!(normalized.pages[0].page_num, 1);
        assert_eq!(normalized.pages[1].page_num, 2);
        assert!(normalized.text.contains(p1));
        assert!(normalized.text.contains(p2));
    }

    #[tokio::test]
    async fn embed_chunks_emits_per_batch_progress_with_counts() {
        use crate::providers::embedding::MockEmbeddingProvider;
        use std::sync::Mutex;

        let provider: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(8));
        // 70 chunks → spans multiple EMBED_BATCH_SIZE (32) batches.
        let chunk_count = 70;
        let chunks: Vec<RawChunk> = (0..chunk_count)
            .map(|i| RawChunk {
                text: format!("chunk number {i}"),
                page_start: 1,
                page_end: 1,
                section_heading: String::new(),
            })
            .collect();

        let updates = Arc::new(Mutex::new(Vec::<IngestionProgress>::new()));
        let captured = updates.clone();
        let on_progress = move |p: IngestionProgress| captured.lock().unwrap().push(p);

        let indexed = embed_chunks(&provider, chunks, "src1", "col1", false, &on_progress)
            .await
            .unwrap();

        assert_eq!(indexed.len(), chunk_count);

        let ups = updates.lock().unwrap();
        // Initial 0/total plus one per batch (ceil(70/32) = 3) → 4 updates.
        assert_eq!(ups.len(), 4, "expected granular per-batch updates: {ups:?}");

        // Every update carries running counts.
        for u in ups.iter() {
            assert_eq!(u.total, Some(chunk_count as u32));
            assert!(u.current.is_some());
        }

        // First reports nothing done, last reports everything done.
        assert_eq!(ups.first().unwrap().current, Some(0));
        let last = ups.last().unwrap();
        assert_eq!(last.current, Some(chunk_count as u32));
        assert!(last.step.contains("70/70"), "step was: {}", last.step);

        // Fractions advance monotonically and stay within the embedding band.
        for w in ups.windows(2) {
            assert!(w[1].fraction >= w[0].fraction, "fractions must not regress");
        }
        assert!((ups.first().unwrap().fraction - EMBED_FRACTION_START).abs() < f32::EPSILON);
        assert!(last.fraction <= EMBED_FRACTION_END + f32::EPSILON);
    }

    #[tokio::test]
    async fn embed_chunks_empty_emits_no_progress() {
        use crate::providers::embedding::MockEmbeddingProvider;
        use std::sync::Mutex;

        let provider: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(8));
        let updates = Arc::new(Mutex::new(Vec::<IngestionProgress>::new()));
        let captured = updates.clone();
        let on_progress = move |p: IngestionProgress| captured.lock().unwrap().push(p);

        let indexed = embed_chunks(&provider, Vec::new(), "src1", "col1", false, &on_progress)
            .await
            .unwrap();

        assert!(indexed.is_empty());
        assert!(updates.lock().unwrap().is_empty());
    }

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

    #[tokio::test]
    async fn get_source_info_not_found_returns_err() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        let result = get_source_info(&db, "does-not-exist").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not found") || msg.contains("does-not-exist"),
            "Got: {msg}"
        );
    }
}
