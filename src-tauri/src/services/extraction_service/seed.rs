//! Seed-anchored LLM extraction for a single named entity.

use std::sync::Arc;

use surrealdb::Connection;

use super::enrich::{enrich_entity, enrich_neighbors_enabled, search_passages};
use super::parse::parse_extraction_response;
use super::persist::persist_batch;
use super::prompts::build_seed_prompt;
use super::{
    batch_passages, llm_complete, ExtractionError, ExtractionPhase, ExtractionProgress,
    ExtractionResult, MAX_ENRICH,
};
use crate::services::entity_service::GraphNode;
use chronacle_providers::embedding::EmbeddingProvider;
use chronacle_providers::llm_provider::{ChatMessage, LlmProvider};
use chronacle_providers::vector_store::VectorStore;

/// Seed-anchored extraction: build the entity named `name` plus its relation
/// neighborhood from chunks across all collections linked to `campaign_id`.
///
/// For each linked collection it gathers candidate passages by the union of
/// semantic search (`VectorStore`) and a lexical `CONTAINS` scan, then runs the
/// seed-anchored prompt and persists collection-scoped (same dedup path as the
/// full sweep). Passing a single collection id to `search` guarantees every
/// semantic hit belongs to that collection, so scoping is unambiguous.
pub async fn extract_seed_anchored<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    vector_store: &Arc<dyn VectorStore>,
    campaign_id: &str,
    name: &str,
    on_progress: impl Fn(ExtractionProgress),
) -> Result<ExtractionResult, ExtractionError> {
    on_progress(ExtractionProgress {
        phase: ExtractionPhase::Resolving,
        detail: format!("Resolving \"{name}\""),
        entities_found: 0,
        relations_found: 0,
    });

    let collection_ids = crate::services::agent_service::resolve_collection_ids(db, campaign_id)
        .await
        .map_err(|e| ExtractionError::Db(e.to_string()))?;

    let query_vec = embed
        .embed_documents(vec![name.to_string()])
        .await
        .map_err(|e| ExtractionError::Embedding(e.to_string()))?
        .into_iter()
        .next()
        .unwrap_or_default();

    let needle = name.to_lowercase();
    let mut entities_created = 0usize;
    let mut relations_created = 0usize;
    let mut all_nodes: Vec<GraphNode> = Vec::new();
    let mut enrich_queue: Vec<GraphNode> = Vec::new();
    let mut total_passages = 0usize;

    let system_prompt = "You are a structured data extraction assistant. Return ONLY valid JSON.";

    for cid in &collection_ids {
        let passages = search_passages(db, vector_store, &query_vec, cid, &needle).await?;
        if passages.is_empty() {
            continue;
        }
        total_passages += passages.len();
        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Searching,
            detail: format!("Found {total_passages} passages"),
            entities_found: entities_created,
            relations_found: relations_created,
        });

        for chunk_text in &batch_passages(passages) {
            let messages = vec![ChatMessage {
                role: "user".to_string(),
                content: build_seed_prompt(name, chunk_text),
            }];
            let raw = llm_complete(llm.as_ref(), system_prompt, &messages).await?;
            let parsed = parse_extraction_response(&raw);
            on_progress(ExtractionProgress {
                phase: ExtractionPhase::Extracting,
                detail: format!("Building \"{name}\""),
                entities_found: entities_created,
                relations_found: relations_created,
            });
            let (ec, rc) =
                persist_batch(db, embed, cid, &parsed, &mut all_nodes, &mut enrich_queue).await?;
            entities_created += ec;
            relations_created += rc;
        }
    }

    // Second pass: enrich neighbor entities with their own entity-centric
    // profile. Opt-in and capped to bound the extra LLM/embedding cost.
    if total_passages > 0 && enrich_neighbors_enabled(db).await {
        for (i, node) in enrich_queue.iter().take(MAX_ENRICH).enumerate() {
            on_progress(ExtractionProgress {
                phase: ExtractionPhase::Enriching,
                detail: format!(
                    "Enriching \"{}\" ({}/{})",
                    node.name,
                    i + 1,
                    enrich_queue.len().min(MAX_ENRICH)
                ),
                entities_found: entities_created,
                relations_found: relations_created,
            });
            // Best-effort: a failed enrichment must not abort the whole run.
            if let Err(e) = enrich_entity(db, llm, embed, vector_store, node).await {
                eprintln!("extraction: enrichment failed for {}: {e}", node.name);
            }
        }
    }

    if total_passages == 0 {
        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Empty,
            detail: format!("No passages found for \"{name}\""),
            entities_found: 0,
            relations_found: 0,
        });
    } else {
        on_progress(ExtractionProgress {
            phase: ExtractionPhase::Done,
            detail: format!("Created {entities_created} entities, {relations_created} relations"),
            entities_found: entities_created,
            relations_found: relations_created,
        });
    }

    Ok(ExtractionResult {
        entities_created,
        relations_created,
        entities: all_nodes,
    })
}

#[cfg(test)]
#[path = "seed_tests.rs"]
mod tests;
