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
    let extracted = state
        .pdf_extractor
        .extract(&pdf_data)
        .await
        .map_err(|e| IngestionError::PdfExtraction(e.to_string()))?;
    let extracted = normalize_extracted(&extracted);

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
        embed_chunks(&embed_provider, chunks, source_id, &source_info.collection_id).await?;

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
}

/// Embed chunks using the embedding provider, tagging each with the source's collection.
async fn embed_chunks(
    provider: &Arc<dyn EmbeddingProvider>,
    chunks: Vec<RawChunk>,
    source_id: &str,
    collection_id: &str,
) -> Result<Vec<IndexedChunk>, IngestionError> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let embeddings = provider
        .embed_documents(texts)
        .await
        .map_err(|e| IngestionError::Embedding(e.to_string()))?;

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
        assert!(msg.contains("not found") || msg.contains("does-not-exist"), "Got: {msg}");
    }
}
