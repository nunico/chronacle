/// Ingestion service — orchestrates PDF extraction, chunking, and embedding.
///
/// Phase 1: extracts text from PDF using `pdf-extract`, chunks via sliding-window
/// section-aware chunker, embeds via fastembed, and stores in SurrealDB.
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
#[derive(Debug, Clone)]
pub struct IngestionProgress {
    pub fraction: f32,
    pub step: String,
}

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
pub async fn ingest_source(
    state: &Arc<AppState>,
    source_id: &str,
    on_progress: Arc<dyn Fn(IngestionProgress) + Send + Sync>,
) -> Result<(), IngestionError> {
    on_progress(IngestionProgress {
        fraction: 0.02,
        step: "Reading source metadata".into(),
    });

    state
        .db
        .query("UPDATE source SET index_status = 'indexing' WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(format!("Failed to update index_status: {e}")))?;

    // Read the source record to get filename and campaign_id
    let source_info = get_source_info(&state.db, source_id).await?;

    on_progress(IngestionProgress {
        fraction: 0.05,
        step: "Loading PDF file from storage".into(),
    });
    let pdf_data = state
        .blob_store
        .retrieve(source_id, &source_info.filename)
        .await
        .map_err(|e| IngestionError::Store(e.to_string()))?;

    on_progress(IngestionProgress {
        fraction: 0.20,
        step: "Extracting text from PDF pages".into(),
    });
    let extracted = extract_text(&pdf_data).await?;

    on_progress(IngestionProgress {
        fraction: 0.25,
        step: "Splitting text into searchable chunks".into(),
    });
    let chunks = chunk_text(&extracted, source_id)?;

    on_progress(IngestionProgress {
        fraction: 0.30,
        step: "Generating vector embeddings for chunks".into(),
    });
    let embed_provider = state
        .embedding_provider
        .read()
        .map_err(|e| IngestionError::Db(format!("Embedding lock: {e}")))?
        .clone();
    let indexed =
        embed_chunks(&embed_provider, chunks, source_id, &source_info.campaign_id).await?;

    drop(embed_provider);

    on_progress(IngestionProgress {
        fraction: 0.85,
        step: "Writing chunks to database".into(),
    });
    state
        .vector_store
        .upsert(source_id, &indexed)
        .await
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    on_progress(IngestionProgress {
        fraction: 0.98,
        step: "Finalizing indexing".into(),
    });
    state
        .db
        .query("UPDATE source SET index_status = 'done', page_count = $page_count WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .bind(("page_count", extracted.page_count as i64))
        .await
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    Ok(())
}

/// Extract text from raw PDF bytes using `pdf-extract`.
///
/// Returns an `ExtractedDoc` with per-page text content and the full merged text.
/// Falls back gracefully to flat extraction if per-page API fails.
pub async fn extract_text(data: &[u8]) -> Result<ExtractedDoc, IngestionError> {
    // Try per-page extraction first for best page tracking
    match pdf_extract::extract_text_from_mem_by_pages(data) {
        Ok(page_texts) => {
            let page_count = page_texts.len();
            let mut pages = Vec::with_capacity(page_count);
            let mut full_text = String::new();

            for (i, text) in page_texts.into_iter().enumerate() {
                let trimmed = text.trim().to_string();
                pages.push(PageContent {
                    page_num: i + 1,
                    text: trimmed.clone(),
                });
                if !full_text.is_empty() && !trimmed.is_empty() {
                    full_text.push('\n');
                }
                full_text.push_str(&trimmed);
            }

            Ok(ExtractedDoc {
                page_count,
                text: full_text,
                pages,
            })
        }
        Err(e) => {
            // Fallback to single-page extraction, split on form feeds
            eprintln!("Per-page PDF extraction failed, falling back to flat extraction: {e}");
            match pdf_extract::extract_text_from_mem(data) {
                Ok(text) => {
                    let trimmed = text.trim().to_string();
                    let page_texts: Vec<&str> = trimmed.split('\x0C').collect();
                    let page_count = page_texts.len().max(1);
                    let pages: Vec<PageContent> = page_texts
                        .into_iter()
                        .enumerate()
                        .map(|(i, t)| PageContent {
                            page_num: i + 1,
                            text: t.trim().to_string(),
                        })
                        .collect();

                    Ok(ExtractedDoc {
                        page_count,
                        text: trimmed,
                        pages,
                    })
                }
                Err(e2) => Err(IngestionError::PdfExtraction(format!(
                    "PDF extraction failed: {e2}"
                ))),
            }
        }
    }
}

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
struct SourceInfo {
    filename: String,
    campaign_id: Option<String>,
}

/// Embed chunks using the embedding provider, tagging each with the source's campaign.
async fn embed_chunks(
    provider: &Arc<dyn EmbeddingProvider>,
    chunks: Vec<RawChunk>,
    source_id: &str,
    campaign_id: &Option<String>,
) -> Result<Vec<IndexedChunk>, IngestionError> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let embeddings = provider
        .embed(texts)
        .await
        .map_err(|e| IngestionError::Embedding(e.to_string()))?;

    let embed_model = provider.model_name().to_string();
    let campaign = campaign_id.clone();

    Ok(chunks
        .into_iter()
        .zip(embeddings)
        .map(|(chunk, embedding)| IndexedChunk {
            chunk_id: format!("{}-{}", source_id, uuid::Uuid::new_v4()),
            campaign_id: campaign.clone(),
            text: chunk.text,
            page_start: chunk.page_start,
            page_end: chunk.page_end,
            section_heading: chunk.section_heading,
            source_type: String::new(),
            embedding,
            embed_model: embed_model.clone(),
        })
        .collect())
}

/// Read source filename and campaign_id from the database.
async fn get_source_info<C>(
    db: &surrealdb::Surreal<C>,
    source_id: &str,
) -> Result<SourceInfo, IngestionError>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT filename, campaign FROM source WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(format!("Failed to query source: {e}")))?;

    #[derive(serde::Deserialize)]
    struct Row {
        filename: String,
        campaign: Option<surrealdb::sql::Thing>,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    rows.into_iter()
        .next()
        .map(|r| SourceInfo {
            filename: r.filename,
            campaign_id: r.campaign.map(|c| c.id.to_string()),
        })
        .ok_or_else(|| IngestionError::Db(format!("Source '{source_id}' not found")))
}
