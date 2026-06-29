//! Persisting a parsed LLM batch into a collection: dedup-or-create each entity
//! and its relation neighbours, embed new nodes, and write canonical
//! `relates_to` edges.

use std::sync::Arc;

use surrealdb::Connection;

use super::parse::{parse_kind, LlmResponse};
use super::ExtractionError;
use crate::entity_service::{self, EntityInput, GraphNode, RelType};
use chronacle_core::embedding::EmbeddingProvider;

/// Embed an entity and store the vector + model ID on the record.
///
/// Thin wrapper over [`entity_service::embed_node`] (the single source of truth
/// for entity embedding, which embeds name + summary + notes), adapting its
/// error into [`ExtractionError`].
pub(super) async fn embed_entity<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    node: &GraphNode,
) -> Result<(), ExtractionError> {
    entity_service::embed_node(db, embed, node)
        .await
        .map_err(|e| ExtractionError::Embedding(e.to_string()))
}

/// Persist one parsed LLM batch into `collection_id`, deduplicating by
/// name+kind within the collection. Returns (entities_created, relations_created)
/// and pushes any newly created nodes onto `all_nodes`. Newly created *relation*
/// (neighbor) nodes are additionally pushed onto `enrich_queue` so the caller can
/// run the second-pass enrichment on them.
pub(super) async fn persist_batch<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    collection_id: &str,
    parsed: &LlmResponse,
    all_nodes: &mut Vec<GraphNode>,
    enrich_queue: &mut Vec<GraphNode>,
) -> Result<(usize, usize), ExtractionError> {
    let mut entities_created = 0usize;
    let mut relations_created = 0usize;

    for ent in &parsed.entities {
        let kind = parse_kind(&ent.kind);
        let existing =
            entity_service::find_by_name_and_collection(db, collection_id, &ent.name, kind.clone())
                .await
                .map_err(|e| ExtractionError::Db(e.to_string()))?;

        let origin_node = if let Some(node) = existing {
            node
        } else {
            let node = entity_service::create(
                db,
                None,
                Some(collection_id),
                kind,
                EntityInput {
                    name: ent.name.clone(),
                    summary: ent.summary.clone(),
                    notes: ent.notes.clone(),
                    date_start: None,
                    date_end: None,
                    is_ongoing: None,
                    sequence_index: None,
                    era: None,
                    duration_label: None,
                    session_id: None,
                    player_name: None,
                    character_class: None,
                    character_level: None,
                    status: None,
                },
            )
            .await
            .map_err(|e| ExtractionError::Db(e.to_string()))?;
            if let Err(e) = embed_entity(db, embed, &node).await {
                eprintln!(
                    "extraction: failed to embed entity {} ({}); it will be missing from semantic search: {e}",
                    node.name, node.kind
                );
            }
            entities_created += 1;
            all_nodes.push(node.clone());
            node
        };

        for rel in &ent.relations {
            let rel_kind = parse_kind(&rel.kind);
            let existing_rel = entity_service::find_by_name_and_collection(
                db,
                collection_id,
                &rel.name,
                rel_kind.clone(),
            )
            .await
            .map_err(|e| ExtractionError::Db(e.to_string()))?;

            let rel_node = if let Some(node) = existing_rel {
                node
            } else {
                let node = entity_service::create(
                    db,
                    None,
                    Some(collection_id),
                    rel_kind,
                    EntityInput {
                        name: rel.name.clone(),
                        summary: rel.summary.clone(),
                        notes: rel.notes.clone(),
                        date_start: None,
                        date_end: None,
                        is_ongoing: None,
                        sequence_index: None,
                        era: None,
                        duration_label: None,
                        session_id: None,
                        player_name: None,
                        character_class: None,
                        character_level: None,
                        status: None,
                    },
                )
                .await
                .map_err(|e| ExtractionError::Db(e.to_string()))?;
                if let Err(e) = embed_entity(db, embed, &node).await {
                    eprintln!(
                    "extraction: failed to embed entity {} ({}); it will be missing from semantic search: {e}",
                    node.name, node.kind
                );
                }
                entities_created += 1;
                all_nodes.push(node.clone());
                enrich_queue.push(node.clone());
                node
            };

            if rel_node.campaign_id.is_some() {
                eprintln!(
                    "extraction: skipping cross-link {} → {} (collection→campaign forbidden)",
                    origin_node.name, rel_node.name
                );
                continue;
            }

            // Normalize to canonical direction: inverse rel_types (e.g. "led_by")
            // flip the edge so storage holds only canonical keys; "Other" values
            // are stored verbatim, unflipped.
            let (canonical, flip) = RelType::from_llm(&rel.rel_type).canonical();
            let (from_id, from_kind, to_id, to_kind) = if flip {
                (
                    &rel_node.id,
                    &rel_node.kind,
                    &origin_node.id,
                    &origin_node.kind,
                )
            } else {
                (
                    &origin_node.id,
                    &origin_node.kind,
                    &rel_node.id,
                    &rel_node.kind,
                )
            };
            // relate_collapsing enforces the tier rule: a specific relationship
            // drops any pre-existing generic (`related_to`/`knows`) or noise
            // (`mentioned`, e.g. from this entity's own wikilinks created moments
            // earlier) edges for the pair, and a generic edge is skipped entirely
            // when a specific one already exists.
            let result = entity_service::relate_collapsing(
                db,
                from_id,
                from_kind,
                to_id,
                to_kind,
                canonical.as_str(),
                None,
            )
            .await;
            match result {
                Ok(true) => relations_created += 1,
                Ok(false) => {} // redundant edge collapsed away — not counted
                Err(e) => eprintln!(
                    "extraction: failed to relate {} -> {} ({}): {e}",
                    origin_node.name,
                    rel_node.name,
                    canonical.as_str()
                ),
            }
        }
    }

    Ok((entities_created, relations_created))
}
