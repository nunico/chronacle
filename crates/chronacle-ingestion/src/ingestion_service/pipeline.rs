use std::sync::Arc;

use crate::chunker::{chunk_document, ExtractedDoc, PageContent};
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::vector_store::IndexedChunk;

use super::types::{IngestionError, IngestionProgress, RawChunk};

/// Text extraction reports per-page progress across this fraction range,
/// interpolated linearly by page number.
pub(super) const EXTRACT_FRACTION_START: f32 = 0.08;
pub(super) const EXTRACT_FRACTION_END: f32 = 0.20;

/// Chunks are embedded in batches of this size so the UI sees steady progress.
pub(super) const EMBED_BATCH_SIZE: usize = 32;
/// The embedding stage spans this fraction range; per-batch progress is
/// interpolated linearly across it.
pub(super) const EMBED_FRACTION_START: f32 = 0.30;
pub(super) const EMBED_FRACTION_END: f32 = 0.85;

/// Apply [`text_normalizer::normalize`] to every page and rebuild the merged
/// full text. This repairs PDF extraction artifacts (soft hyphens, intra-paragraph
/// newlines) before chunking so that embeddings see clean prose.
pub(super) fn normalize_extracted(doc: &ExtractedDoc) -> ExtractedDoc {
    use crate::text_normalizer::normalize;
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

pub(super) fn chunk_text(
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

/// Embed chunks using the embedding provider, tagging each with the source's collection.
///
/// Embedding is the longest stage of ingestion, so chunks are processed in
/// batches of [`EMBED_BATCH_SIZE`] and `on_progress` is called after each batch
/// with a running `current`/`total` count.
pub(super) async fn embed_chunks(
    provider: &Arc<dyn EmbeddingProvider>,
    chunks: Vec<RawChunk>,
    source_id: &str,
    collection_id: &str,
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
            embedding,
            embed_model: embed_model.clone(),
        })
        .collect())
}
