//! Full-collection LLM extraction sweep.

use std::sync::Arc;

use surrealdb::Connection;

use super::parse::parse_extraction_response;
use super::persist::persist_batch;
use super::prompts::build_extraction_prompt;
use super::{
    llm_complete, ExtractionError, ExtractionPhase, ExtractionProgress, ExtractionResult,
    BATCH_CHAR_BUDGET,
};
use crate::entity_service::GraphNode;
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmProvider};

/// Run LLM extraction on all chunks of `collection_id`.
///
/// - Reads ALL chunks (not vector search — full coverage is required).
/// - Batches by character budget (~4000 tokens per batch).
/// - Deduplicates entities by name+kind within the collection.
/// - Embeds every created entity immediately (ADR-003 pattern).
/// - Calls `on_progress` with batch progress after each batch.
pub async fn extract_from_collection<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    collection_id: &str,
    on_progress: impl Fn(ExtractionProgress),
) -> Result<ExtractionResult, ExtractionError> {
    #[derive(serde::Deserialize)]
    struct ChunkRow {
        text: String,
    }
    let mut resp = db
        .query("SELECT text FROM chunk WHERE collection = type::thing('collection', $cid)")
        .bind(("cid", collection_id.to_owned()))
        .await
        .map_err(|e| ExtractionError::Db(e.to_string()))?;
    let chunks: Vec<ChunkRow> = resp
        .take(0)
        .map_err(|e| ExtractionError::Db(e.to_string()))?;

    if chunks.is_empty() {
        return Ok(ExtractionResult {
            entities_created: 0,
            relations_created: 0,
            entities: vec![],
        });
    }

    let mut batches: Vec<String> = Vec::new();
    let mut current = String::new();
    for chunk in chunks {
        if !current.is_empty() && current.len() + chunk.text.len() > BATCH_CHAR_BUDGET {
            batches.push(std::mem::take(&mut current));
        }
        current.push_str(&chunk.text);
        current.push('\n');
    }
    if !current.is_empty() {
        batches.push(current);
    }
    let total_batches = batches.len();

    let system_prompt = "You are a structured data extraction assistant. Return ONLY valid JSON.";
    let mut entities_created = 0usize;
    let mut relations_created = 0usize;
    let mut all_nodes: Vec<GraphNode> = Vec::new();
    // Full sweep processes every chunk, so neighbors get their own passages —
    // no second-pass enrichment needed here. Discarded.
    let mut enrich_queue: Vec<GraphNode> = Vec::new();

    for (batch_idx, chunk_text) in batches.iter().enumerate() {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: build_extraction_prompt(chunk_text),
        }];
        let raw = llm_complete(llm.as_ref(), system_prompt, &messages).await?;
        let parsed = parse_extraction_response(&raw);
        let (ec, rc) = persist_batch(
            db,
            embed,
            collection_id,
            &parsed,
            &mut all_nodes,
            &mut enrich_queue,
        )
        .await?;
        entities_created += ec;
        relations_created += rc;
        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Extracting,
            detail: format!("Batch {}/{}", batch_idx + 1, total_batches),
            entities_found: entities_created,
            relations_found: relations_created,
        });
    }

    on_progress(ExtractionProgress {
        phase: ExtractionPhase::Done,
        detail: format!("Created {entities_created} entities, {relations_created} relations"),
        entities_found: entities_created,
        relations_found: relations_created,
    });

    Ok(ExtractionResult {
        entities_created,
        relations_created,
        entities: all_nodes,
    })
}

#[cfg(test)]
#[path = "collection_tests.rs"]
mod tests;
