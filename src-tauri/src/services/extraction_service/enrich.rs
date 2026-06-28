//! Passage search and the opt-in second-pass enrichment that rewrites a
//! neighbour entity's summary/notes to be entity-centric.

use std::sync::Arc;

use surrealdb::Connection;

use super::parse::parse_profile_response;
use super::prompts::build_profile_prompt;
use super::{batch_passages, llm_complete, ExtractionError, SEED_SEARCH_K};
use crate::services::entity_service::{self, GraphNode};
use chronacle_providers::embedding::EmbeddingProvider;
use chronacle_providers::llm_provider::{ChatMessage, LlmProvider};
use chronacle_providers::vector_store::VectorStore;

/// Gather candidate passages for `needle_lower` within a single collection by
/// the union of semantic search (`query_vec`) and a lexical `CONTAINS` scan.
/// Deduplicates by bare chunk id (semantic ids are `chunk:<id>`, lexical `<id>`).
pub(super) async fn search_passages<C: Connection>(
    db: &surrealdb::Surreal<C>,
    vector_store: &Arc<dyn VectorStore>,
    query_vec: &[f32],
    collection_id: &str,
    needle_lower: &str,
) -> Result<Vec<String>, ExtractionError> {
    // 1. Semantic hits (all belong to `collection_id` — single id passed).
    let semantic = vector_store
        .search(
            query_vec,
            std::slice::from_ref(&collection_id.to_string()),
            SEED_SEARCH_K,
        )
        .await
        .map_err(|e| ExtractionError::Db(e.to_string()))?;

    // 2. Lexical hits within this collection.
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
        text: String,
    }
    let mut resp = db
        .query(
            "SELECT id, text FROM chunk \
             WHERE collection = type::thing('collection', $cid) \
             AND string::contains(string::lowercase(text), $needle)",
        )
        .bind(("cid", collection_id.to_owned()))
        .bind(("needle", needle_lower.to_owned()))
        .await
        .map_err(|e| ExtractionError::Db(e.to_string()))?;
    let lexical: Vec<Row> = resp
        .take(0)
        .map_err(|e| ExtractionError::Db(e.to_string()))?;

    // 3. Union by chunk id, preserving text.
    let mut seen = std::collections::HashSet::new();
    let mut passages: Vec<String> = Vec::new();
    for r in &semantic {
        if seen.insert(r.chunk_id.trim_start_matches("chunk:").to_string()) {
            passages.push(r.text.clone());
        }
    }
    for r in lexical {
        if seen.insert(r.id.id.to_raw()) {
            passages.push(r.text);
        }
    }
    Ok(passages)
}

/// Read the opt-in `extraction_enrich_neighbors` setting (defaults to false).
pub(super) async fn enrich_neighbors_enabled<C: Connection>(db: &surrealdb::Surreal<C>) -> bool {
    #[derive(serde::Deserialize)]
    struct Row {
        value: String,
    }
    let row: Option<Row> = db
        .query("SELECT * FROM setting:extraction_enrich_neighbors")
        .await
        .ok()
        .and_then(|mut r| r.take(0).ok())
        .and_then(|rows: Vec<Row>| rows.into_iter().next());
    matches!(row, Some(r) if r.value == "true")
}

/// Second-pass enrichment: re-search the collection for `node`'s own name,
/// build an entity-centric profile via the LLM, and update its summary/notes
/// in place (then re-embed). Best-effort — returns whether the node was updated.
pub(super) async fn enrich_entity<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    vector_store: &Arc<dyn VectorStore>,
    node: &GraphNode,
) -> Result<bool, ExtractionError> {
    let Some(collection_id) = node.collection_id.as_deref() else {
        return Ok(false);
    };

    let query_vec = embed
        .embed_documents(vec![node.name.clone()])
        .await
        .map_err(|e| ExtractionError::Embedding(e.to_string()))?
        .into_iter()
        .next()
        .unwrap_or_default();
    let needle = node.name.to_lowercase();

    let passages = search_passages(db, vector_store, &query_vec, collection_id, &needle).await?;
    if passages.is_empty() {
        return Ok(false);
    }

    let system_prompt = "You are a structured data extraction assistant. Return ONLY valid JSON.";
    // Longest non-empty wins across batches.
    let mut best_summary: Option<String> = None;
    let mut best_notes: Option<String> = None;
    for chunk_text in batch_passages(passages) {
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: build_profile_prompt(&node.name, &chunk_text),
        }];
        let raw = llm_complete(llm.as_ref(), system_prompt, &messages).await?;
        let fields = parse_profile_response(&raw);
        if let Some(s) = fields.summary.filter(|s| !s.trim().is_empty()) {
            if best_summary.as_ref().is_none_or(|b| s.len() > b.len()) {
                best_summary = Some(s);
            }
        }
        if let Some(n) = fields.notes.filter(|n| !n.trim().is_empty()) {
            if best_notes.as_ref().is_none_or(|b| n.len() > b.len()) {
                best_notes = Some(n);
            }
        }
    }

    // Nothing usable came back — leave the first-pass values untouched.
    if best_summary.is_none() && best_notes.is_none() {
        return Ok(false);
    }

    let new_summary = best_summary.or_else(|| node.summary.clone());
    let new_notes = best_notes.or_else(|| node.notes.clone());

    // Targeted update of only summary/notes — these graph-entity tables are
    // SCHEMAFULL with just name/summary/notes, so we must not SET unrelated
    // event/player-character fields. Bind explicit NULL (not NONE) for empty
    // fields: the schema types them `string | NULL`, which rejects NONE.
    use surrealdb::sql::Value;
    let summary_val = new_summary.clone().map_or(Value::Null, Value::from);
    let notes_val = new_notes.clone().map_or(Value::Null, Value::from);
    db.query(
        "UPDATE type::thing($table, $id) \
         SET summary = $summary, notes = $notes, updated_at = time::now()",
    )
    .bind(("table", node.kind.clone()))
    .bind(("id", node.id.clone()))
    .bind(("summary", summary_val))
    .bind(("notes", notes_val))
    .await
    .map_err(|e| ExtractionError::Db(e.to_string()))?
    .check()
    .map_err(|e| ExtractionError::Db(e.to_string()))?;

    // Re-embed with the enriched text (name + summary + notes).
    let updated = GraphNode {
        summary: new_summary,
        notes: new_notes,
        ..node.clone()
    };
    if let Err(e) = entity_service::embed_node(db, embed, &updated).await {
        eprintln!(
            "extraction: failed to re-embed enriched entity {} ({}): {e}",
            updated.name, updated.kind
        );
    }
    Ok(true)
}
