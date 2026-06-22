/// Extraction service — LLM-powered entity extraction from collection chunks.
///
/// Reads all chunks belonging to a collection, batches them by token budget,
/// calls the LLM per batch to extract structured entity data (2 levels deep),
/// then persists entities and relations via `entity_service`.  Collection
/// entities are embedded immediately after creation for later vector retrieval
/// in `agent_service::fetch_entity_context`.
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use surrealdb::Connection;

use crate::providers::embedding::EmbeddingProvider;
use crate::providers::llm_provider::{ChatMessage, LlmProvider};
use crate::providers::vector_store::VectorStore;
use crate::services::entity_service::{self, EntityInput, EntityKind, GraphNode};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Database error: {0}")]
    Db(String),
    #[error("Embedding error: {0}")]
    Embedding(String),
    #[error("JSON parse error: {0}")]
    Parse(String),
}

// ── Public result types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ExtractionResult {
    pub entities_created: usize,
    pub relations_created: usize,
    pub entities: Vec<GraphNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionPhase {
    Resolving,
    Searching,
    Extracting,
    Relating,
    Embedding,
    Enriching,
    Done,
    Empty,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractionProgress {
    pub phase: ExtractionPhase,
    /// Human-readable, e.g. "Found 12 passages".
    pub detail: String,
    /// Running total across the whole extraction run.
    pub entities_found: usize,
    /// Running total across the whole extraction run.
    pub relations_found: usize,
}

// ── LLM response schema ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LlmResponse {
    #[serde(default)]
    entities: Vec<LlmEntity>,
}

#[derive(Debug, Deserialize)]
struct LlmEntity {
    name: String,
    kind: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    relations: Vec<LlmRelation>,
}

#[derive(Debug, Deserialize)]
struct LlmRelation {
    name: String,
    kind: String,
    rel_type: String,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

// ── Internals ─────────────────────────────────────────────────────────────────

/// Approximate characters per token for batching heuristic (~4 chars/token).
const CHARS_PER_TOKEN: usize = 4;
/// Target token budget per LLM batch.
const BATCH_TOKEN_BUDGET: usize = 4000;
const BATCH_CHAR_BUDGET: usize = BATCH_TOKEN_BUDGET * CHARS_PER_TOKEN;

/// Maximum number of neighbor entities to enrich in the second pass. Caps the
/// extra LLM + embedding cost of seed-anchored enrichment (opt-in via the
/// `extraction_enrich_neighbors` setting).
const MAX_ENRICH: usize = 20;

/// Drain the streaming LLM channel into a complete response string.
async fn llm_complete(
    llm: &dyn LlmProvider,
    system_prompt: &str,
    messages: &[ChatMessage],
) -> Result<String, ExtractionError> {
    let mut rx = llm
        .chat_stream(system_prompt, messages)
        .await
        .map_err(|e| ExtractionError::Llm(e.to_string()))?;
    let mut buf = String::new();
    while let Some(token_result) = rx.recv().await {
        let token = token_result.map_err(|e| ExtractionError::Llm(e.to_string()))?;
        buf.push_str(&token);
    }
    Ok(buf)
}

/// Build the system prompt that instructs the LLM to extract entities as JSON.
fn build_extraction_prompt(chunk_text: &str) -> String {
    format!(
        r#"You are an expert at extracting structured game entities from TTRPG source material.

Extract all named entities from the following text. For each entity:
- Identify its kind (one of: npc, location, faction, creature, item, event, player_character, misc)
- For entities directly related to a level-0 entity, include them in that entity's "relations" array
- For entities mentioned only in passing (level 2+), write their names as [[wikilinks]] inside the notes field — do NOT extract them as separate entities

Field rules (apply to BOTH top-level entities and entities in "relations"):
- "summary": a short, concise description of the entity ITSELF — who or what it is — in 1 sentence. Do NOT describe how it relates to any other entity here. The ONLY exception is when the entity is inherently about a relationship (e.g. an association, alliance, or pact between parties); then the relationship is its identity and belongs in the summary.
- "notes": a more thorough description, including how this entity relates to others (its role, ties, and the connection to the entity it was extracted alongside). May contain [[wikilinks]]. Leave empty if there is nothing beyond the summary.

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):

{{
  "entities": [
    {{
      "name": "string",
      "kind": "npc|location|faction|creature|item|event|player_character|misc",
      "summary": "short, concise description of the entity itself",
      "notes": "optional longer description incl. relationships, may contain [[wikilinks]]",
      "relations": [
        {{
          "name": "string",
          "kind": "string",
          "rel_type": "string (e.g. leads, commands, located_in, allied_with)",
          "summary": "short, concise description of this entity itself — NOT its relation to the parent",
          "notes": "optional longer description incl. how it relates to the parent entity"
        }}
      ]
    }}
  ]
}}

Source text:
{chunk_text}"#
    )
}

/// Build a seed-anchored extraction prompt: focus on `name` and the entities
/// directly related to it, rather than extracting everything in the text.
fn build_seed_prompt(name: &str, chunk_text: &str) -> String {
    format!(
        r#"You are an expert at extracting structured game entities from TTRPG source material.

Build a complete profile of the entity named "{name}" using ONLY the source text below.
- Output "{name}" as a single level-0 entity with its kind, summary, and notes.
- Include entities DIRECTLY related to "{name}" in its "relations" array (allies, members, locations, leaders, etc.).
- For entities mentioned only in passing, write their names as [[wikilinks]] inside notes — do NOT extract them separately.
- If "{name}" is not described in the text, return an empty "entities" array.

Field rules (apply to BOTH "{name}" and entities in "relations"):
- "summary": a short, concise description of the entity ITSELF — who or what it is — in 1 sentence. Do NOT describe how a related entity connects to "{name}" here. The ONLY exception is when the entity is inherently about a relationship (e.g. an association, alliance, or pact between parties); then the relationship is its identity and belongs in the summary.
- "notes": a more thorough description, including how the entity relates to "{name}" (its role, ties, and connection). May contain [[wikilinks]]. Leave empty if there is nothing beyond the summary.

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):

{{
  "entities": [
    {{
      "name": "string",
      "kind": "npc|location|faction|creature|item|event|player_character|misc",
      "summary": "short, concise description of the entity itself",
      "notes": "optional longer description incl. relationships, may contain [[wikilinks]]",
      "relations": [
        {{ "name": "string", "kind": "string", "rel_type": "string", "summary": "short, concise description of this entity itself — NOT its relation to {name}", "notes": "optional longer description incl. how it relates to {name}" }}
      ]
    }}
  ]
}}

Source text:
{chunk_text}"#
    )
}

/// Build a profile prompt for the second enrichment pass: describe ONE entity
/// from its own passages, with no relations (depth-1, description only).
fn build_profile_prompt(name: &str, chunk_text: &str) -> String {
    format!(
        r#"You are an expert at describing game entities from TTRPG source material.

Describe ONLY the entity named "{name}" using the source text below.
- "summary": a short, concise description of "{name}" ITSELF — who or what it is — in 1 sentence. Do NOT describe how it relates to other entities, UNLESS "{name}" is inherently about a relationship (e.g. an association, alliance, or pact between parties), in which case that relationship is its identity and belongs here.
- "notes": a more thorough description, including how "{name}" relates to others. May contain [[wikilinks]]. Use an empty string if there is nothing beyond the summary.
- If "{name}" is not described in the text, return empty strings.

Do NOT extract any other entities or relations.

Return ONLY valid JSON matching this exact schema (no markdown, no explanation):

{{ "summary": "string", "notes": "string" }}

Source text:
{chunk_text}"#
    )
}

/// Summary/notes returned by the profile (enrichment) pass.
#[derive(Debug, Default, Deserialize)]
struct ProfileFields {
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

/// Parse a profile-pass response, tolerating markdown fences and malformed JSON.
fn parse_profile_response(raw: &str) -> ProfileFields {
    let trimmed = raw.trim();
    let json_str = if let Some(s) = trimmed.strip_prefix("```json") {
        s.trim_end_matches("```").trim()
    } else if let Some(s) = trimmed.strip_prefix("```") {
        s.trim_end_matches("```").trim()
    } else {
        trimmed
    };
    serde_json::from_str(json_str).unwrap_or_default()
}

/// Parse the LLM response, tolerating truncated or partially-valid JSON.
fn parse_extraction_response(raw: &str) -> LlmResponse {
    // Strip markdown code fences if present.
    let trimmed = raw.trim();
    let json_str = if let Some(s) = trimmed.strip_prefix("```json") {
        s.trim_end_matches("```").trim()
    } else if let Some(s) = trimmed.strip_prefix("```") {
        s.trim_end_matches("```").trim()
    } else {
        trimmed
    };

    serde_json::from_str(json_str).unwrap_or_else(|e| {
        eprintln!("extraction: JSON parse failed ({e}), returning empty result");
        LlmResponse { entities: vec![] }
    })
}

/// Convert a kind string from the LLM to an EntityKind, defaulting to Misc.
fn parse_kind(kind: &str) -> EntityKind {
    EntityKind::from_table(kind).unwrap_or(EntityKind::Misc)
}

/// Embed an entity and store the vector + model ID on the record.
///
/// Thin wrapper over [`entity_service::embed_node`] (the single source of truth
/// for entity embedding, which embeds name + summary + notes), adapting its
/// error into [`ExtractionError`].
async fn embed_entity<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    node: &GraphNode,
) -> Result<(), ExtractionError> {
    entity_service::embed_node(db, embed, node)
        .await
        .map_err(|e| ExtractionError::Embedding(e.to_string()))
}

// ── Batch persistence helper ──────────────────────────────────────────────────

/// Persist one parsed LLM batch into `collection_id`, deduplicating by
/// name+kind within the collection. Returns (entities_created, relations_created)
/// and pushes any newly created nodes onto `all_nodes`. Newly created *relation*
/// (neighbor) nodes are additionally pushed onto `enrich_queue` so the caller can
/// run the second-pass enrichment on them.
async fn persist_batch<C: Connection>(
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

            let result = entity_service::relate(
                db,
                &origin_node.id,
                &origin_node.kind,
                &rel_node.id,
                &rel_node.kind,
                &rel.rel_type,
                None,
            )
            .await;
            match result {
                Ok(_) => relations_created += 1,
                Err(e) => eprintln!(
                    "extraction: failed to relate {} -> {} ({}): {e}",
                    origin_node.name, rel_node.name, rel.rel_type
                ),
            }
        }
    }

    Ok((entities_created, relations_created))
}

// ── Passage search + second-pass enrichment ────────────────────────────────────

/// Gather candidate passages for `needle_lower` within a single collection by
/// the union of semantic search (`query_vec`) and a lexical `CONTAINS` scan.
/// Deduplicates by bare chunk id (semantic ids are `chunk:<id>`, lexical `<id>`).
async fn search_passages<C: Connection>(
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
async fn enrich_neighbors_enabled<C: Connection>(db: &surrealdb::Surreal<C>) -> bool {
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

/// Split passages into character-budgeted batches for LLM calls.
fn batch_passages(passages: Vec<String>) -> Vec<String> {
    let mut batches: Vec<String> = Vec::new();
    let mut current = String::new();
    for p in passages {
        if !current.is_empty() && current.len() + p.len() > BATCH_CHAR_BUDGET {
            batches.push(std::mem::take(&mut current));
        }
        current.push_str(&p);
        current.push('\n');
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

/// Second-pass enrichment: re-search the collection for `node`'s own name,
/// build an entity-centric profile via the LLM, and update its summary/notes
/// in place (then re-embed). Best-effort — returns whether the node was updated.
async fn enrich_entity<C: Connection>(
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

// ── Public API ────────────────────────────────────────────────────────────────

/// Number of semantic neighbours to fetch per collection for seed extraction.
const SEED_SEARCH_K: u64 = 12;

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

        // Batch passages by char budget and run the seed prompt per batch.
        let batches = batch_passages(passages);

        let system_prompt =
            "You are a structured data extraction assistant. Return ONLY valid JSON.";
        for chunk_text in &batches {
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
    // 1. Fetch all chunks for this collection (full scan, ordered for reproducibility).
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

    // 2. Batch chunks by character budget.
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
    // The full sweep extracts every chunk, so neighbors already get their own
    // passages — no second-pass enrichment is needed. Discarded.
    let mut enrich_queue: Vec<GraphNode> = Vec::new();

    // 3. Process each batch.
    for (batch_idx, chunk_text) in batches.iter().enumerate() {
        let user_prompt = build_extraction_prompt(chunk_text);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: user_prompt,
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::embedding::MockEmbeddingProvider;
    use crate::providers::vector_store::{
        IndexedChunk, SearchResult, VectorStore, VectorStoreError,
    };

    // ── Unit: prompt building ────────────────────────────────────────────────

    #[test]
    fn build_extraction_prompt_contains_chunk_text() {
        let prompt = build_extraction_prompt("The Iron Fist faction rules the docks.");
        assert!(prompt.contains("The Iron Fist faction rules the docks."));
        assert!(prompt.contains("entities"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn build_seed_prompt_anchors_on_entity_name() {
        let prompt = build_seed_prompt("Commander Varn", "Varn leads the Iron Fist.");
        assert!(prompt.contains("Commander Varn"));
        assert!(prompt.contains("Varn leads the Iron Fist."));
        assert!(prompt.contains("entities"));
        assert!(prompt.contains("JSON"));
    }

    // ── Unit: response parsing ───────────────────────────────────────────────

    #[test]
    fn parse_extraction_response_deserializes_well_formed_json() {
        let json = r#"{
            "entities": [
                {
                    "name": "The Iron Fist",
                    "kind": "faction",
                    "summary": "Militant faction.",
                    "notes": "Key figure: [[Commander Varn]].",
                    "relations": [
                        {
                            "name": "Commander Varn",
                            "kind": "npc",
                            "rel_type": "commands",
                            "summary": "Ruthless leader.",
                            "notes": null
                        }
                    ]
                }
            ]
        }"#;
        let result = parse_extraction_response(json);
        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "The Iron Fist");
        assert_eq!(result.entities[0].relations.len(), 1);
        assert_eq!(result.entities[0].relations[0].name, "Commander Varn");
    }

    #[test]
    fn parse_extraction_response_returns_empty_on_malformed_json() {
        let result = parse_extraction_response("not valid json {{{");
        assert!(result.entities.is_empty());
    }

    #[test]
    fn parse_extraction_response_strips_markdown_code_fences() {
        let json = "```json\n{\"entities\":[]}\n```";
        let result = parse_extraction_response(json);
        assert!(result.entities.is_empty());
    }

    #[test]
    fn parse_extraction_response_tolerates_truncated_response() {
        let truncated = r#"{"entities": [{"name": "Foo", "kind": "#;
        let result = parse_extraction_response(truncated);
        assert!(result.entities.is_empty()); // graceful fallback
    }

    // ── Unit: kind parsing ───────────────────────────────────────────────────

    #[test]
    fn parse_kind_falls_back_to_misc_for_unknown() {
        assert_eq!(parse_kind("dragon_lord"), EntityKind::Misc);
        assert_eq!(parse_kind("npc"), EntityKind::Npc);
    }

    // ── Integration: full round-trip ─────────────────────────────────────────

    /// MockLlmProvider that returns a fixed JSON response containing one
    /// level-0 entity with one level-1 relation.
    struct MockLlm {
        response: String,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlm {
        fn provider_type(&self) -> &'static str {
            "mock_extraction"
        }

        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _messages: &[ChatMessage],
        ) -> Result<
            tokio::sync::mpsc::Receiver<Result<String, crate::providers::llm_provider::LlmError>>,
            crate::providers::llm_provider::LlmError,
        > {
            let (tx, rx) = tokio::sync::mpsc::channel(4);
            let resp = self.response.clone();
            tokio::spawn(async move {
                let _ = tx.send(Ok(resp)).await;
            });
            Ok(rx)
        }
    }

    async fn setup_db_with_collection() -> (surrealdb::Surreal<surrealdb::engine::local::Db>, String)
    {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        let mut resp = db
            .query(
                "CREATE collection SET name='PHB', description=NULL, \
                 created_at=time::now(), updated_at=time::now()",
            )
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        struct Row {
            id: surrealdb::sql::Thing,
        }
        let rows: Vec<Row> = resp.take(0).unwrap();
        let col_id = rows.into_iter().next().unwrap().id.id.to_raw();

        // Create a source record, then a chunk that references both source and collection.
        db.query(
            "CREATE source SET id='src1', filename='test.pdf', display_name='Test', \
             source_type='lore', page_count=1, indexed_at=time::now(), index_status='done', \
             embed_model='mock', collection=type::thing('collection',$cid)",
        )
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
        let zeros = std::iter::repeat_n("0.0", 768)
            .collect::<Vec<_>>()
            .join(",");
        db.query(format!(
            "CREATE chunk SET id='chunk1', \
             text='The Iron Fist controls the eastern docks. Commander Varn leads them.', \
             page_start=1, page_end=1, section_heading='Factions', source_type='lore', \
             source=type::thing('source','src1'), \
             collection=type::thing('collection',$cid), \
             embedding=[{zeros}], embed_model='mock'",
        ))
        .bind(("cid", col_id.clone()))
        .await
        .unwrap()
        .check()
        .unwrap();

        (db, col_id)
    }

    #[tokio::test]
    async fn extract_creates_entities_with_collection_edge() {
        let (db, col_id) = setup_db_with_collection().await;

        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: r#"{
                "entities": [{
                    "name": "The Iron Fist",
                    "kind": "faction",
                    "summary": "Militant faction.",
                    "notes": null,
                    "relations": [{
                        "name": "Commander Varn",
                        "kind": "npc",
                        "rel_type": "commands",
                        "summary": "Leader.",
                        "notes": null
                    }]
                }]
            }"#
            .to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

        let result = extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
            .await
            .unwrap();

        assert_eq!(result.entities_created, 2); // faction + npc
        assert_eq!(result.relations_created, 1);

        // Verify in_collection edges exist
        let mut resp = db
            .query("SELECT count() FROM in_collection WHERE in = type::thing('collection', $cid) GROUP ALL")
            .bind(("cid", col_id.clone()))
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        struct C {
            count: i64,
        }
        let counts: Vec<C> = resp.take(0).unwrap();
        assert_eq!(counts.first().map(|c| c.count).unwrap_or(0), 2);
    }

    #[tokio::test]
    async fn extract_deduplicates_on_second_run() {
        let (db, col_id) = setup_db_with_collection().await;

        let fixed_json = r#"{
            "entities": [{
                "name": "The Iron Fist",
                "kind": "faction",
                "summary": "Militant faction.",
                "notes": null,
                "relations": []
            }]
        }"#
        .to_string();

        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: fixed_json.clone(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

        let r1 = extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
            .await
            .unwrap();
        assert_eq!(r1.entities_created, 1);

        // Second run with same response — should dedup
        let llm2: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: fixed_json,
        });
        let r2 = extract_from_collection(&db, &llm2, &embed, &col_id, |_| {})
            .await
            .unwrap();
        assert_eq!(
            r2.entities_created, 0,
            "duplicate entity must not be re-created"
        );
    }

    #[tokio::test]
    async fn extract_level2_refs_stay_as_wikilinks_not_entities() {
        let (db, col_id) = setup_db_with_collection().await;

        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: r#"{
                "entities": [{
                    "name": "The Iron Fist",
                    "kind": "faction",
                    "summary": "Militant faction.",
                    "notes": "Allied with [[The Emperor's Court]].",
                    "relations": []
                }]
            }"#
            .to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

        let result = extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
            .await
            .unwrap();

        // Only the faction is created — the wikilink ref stays as text, no extra entity
        assert_eq!(result.entities_created, 1);

        let factions = entity_service::get_by_collection(&db, &col_id, EntityKind::Faction)
            .await
            .unwrap();
        assert!(factions[0]
            .notes
            .as_deref()
            .unwrap_or("")
            .contains("[[The Emperor's Court]]"));
    }

    #[tokio::test]
    async fn extract_from_collection_emits_done_phase_with_cumulative_counts() {
        let (db, col_id) = setup_db_with_collection().await;
        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: r#"{"entities":[{"name":"The Iron Fist","kind":"faction","summary":"x","notes":null,"relations":[{"name":"Commander Varn","kind":"npc","rel_type":"commands","summary":"y","notes":null}]}]}"#.to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

        let phases = std::sync::Mutex::new(Vec::<ExtractionProgress>::new());
        extract_from_collection(&db, &llm, &embed, &col_id, |p| {
            phases.lock().unwrap().push(p);
        })
        .await
        .unwrap();

        let phases = phases.into_inner().unwrap();
        let done = phases.last().expect("at least one progress event");
        assert_eq!(done.phase, ExtractionPhase::Done);
        assert_eq!(done.entities_found, 2);
        assert_eq!(done.relations_found, 1);
    }

    // ── MockVectorStore ──────────────────────────────────────────────────────

    struct MockVectorStore {
        results: Vec<SearchResult>,
    }

    #[async_trait::async_trait]
    impl VectorStore for MockVectorStore {
        async fn upsert(&self, _s: &str, _c: &[IndexedChunk]) -> Result<(), VectorStoreError> {
            Ok(())
        }
        async fn search(
            &self,
            _q: &[f32],
            _cids: &[String],
            _limit: u64,
        ) -> Result<Vec<SearchResult>, VectorStoreError> {
            Ok(self.results.clone())
        }
        async fn delete_by_source(&self, _s: &str) -> Result<(), VectorStoreError> {
            Ok(())
        }
    }

    // ── Integration: seed-anchored extraction ────────────────────────────────

    #[tokio::test]
    async fn seed_anchored_builds_named_entity_and_relations_collection_scoped() {
        let (db, col_id) = setup_db_with_collection().await;

        db.query(
            "CREATE campaign SET id='camp1', name='C', system='5e', created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "LET $in  = type::thing('campaign',   $campaign_id); \
             LET $out = type::thing('collection', $collection_id); \
             RELATE $in->subscribes_to->$out SET created_at=time::now()",
        )
        .bind(("campaign_id", "camp1"))
        .bind(("collection_id", col_id.clone()))
        .await
        .unwrap();

        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: r#"{"entities":[{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{"name":"The Iron Fist","kind":"faction","rel_type":"commands","summary":"Militia.","notes":null}]}]}"#.to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
        let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

        let result =
            extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
                .await
                .unwrap();

        assert_eq!(result.entities_created, 2);
        assert_eq!(result.relations_created, 1);

        let npcs = entity_service::get_by_collection(&db, &col_id, EntityKind::Npc)
            .await
            .unwrap();
        assert!(npcs.iter().any(|n| n.name == "Commander Varn"));
    }

    #[tokio::test]
    async fn seed_anchored_emits_empty_phase_when_no_passages() {
        let (db, col_id) = setup_db_with_collection().await;
        db.query(
            "CREATE campaign SET id='camp1', name='C', system='5e', created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "LET $in  = type::thing('campaign',   $campaign_id); \
             LET $out = type::thing('collection', $collection_id); \
             RELATE $in->subscribes_to->$out SET created_at=time::now()",
        )
        .bind(("campaign_id", "camp1"))
        .bind(("collection_id", col_id.clone()))
        .await
        .unwrap();

        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: "{}".to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
        let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

        let phases = std::sync::Mutex::new(Vec::<ExtractionProgress>::new());
        let result =
            extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Nonexistent Entity", |p| {
                phases.lock().unwrap().push(p);
            })
            .await
            .unwrap();

        assert_eq!(result.entities_created, 0);
        let phases = phases.into_inner().unwrap();
        assert_eq!(phases.last().unwrap().phase, ExtractionPhase::Empty);
    }

    #[tokio::test]
    async fn extract_cross_link_collection_to_campaign_is_skipped() {
        let (db, col_id) = setup_db_with_collection().await;

        // Pre-create a campaign entity with the same name as the relation target
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        entity_service::create(
            &db,
            Some("camp1"),
            None,
            EntityKind::Npc,
            EntityInput {
                name: "Campaign NPC".to_string(),
                summary: None,
                notes: None,
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
        .unwrap();

        // Extraction should create the collection entity but NOT create a
        // relates_to edge pointing to the campaign entity.
        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: r#"{
                "entities": [{
                    "name": "Collection Faction",
                    "kind": "faction",
                    "summary": "A faction.",
                    "notes": null,
                    "relations": []
                }]
            }"#
            .to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

        let result = extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
            .await
            .unwrap();
        assert_eq!(result.entities_created, 1);
        assert_eq!(result.relations_created, 0);
    }

    #[tokio::test]
    async fn seed_anchored_uses_semantic_hits_without_lexical_match() {
        let (db, col_id) = setup_db_with_collection().await;
        db.query(
            "CREATE campaign SET id='camp1', name='C', system='5e', created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query("LET $in = type::thing('campaign','camp1'); LET $out = type::thing('collection', $cid); RELATE $in->subscribes_to->$out SET created_at=time::now()")
            .bind(("cid", col_id.clone()))
            .await
            .unwrap();

        // Semantic result whose text does NOT contain the seed name "Mystery Lord".
        let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
            response: r#"{"entities":[{"name":"Mystery Lord","kind":"npc","summary":"A figure.","notes":null,"relations":[]}]}"#.to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
        let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
            results: vec![SearchResult {
                chunk_id: "chunk:semchunk".to_string(),
                source_id: "source:s1".to_string(),
                source_name: "Book".to_string(),
                text: "An enigmatic ruler governs from the shadows.".to_string(),
                page_start: 1,
                page_end: 1,
                section_heading: "Lore".to_string(),
                source_type: "lore".to_string(),
                distance: 0.1,
            }],
        });

        let result = extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Mystery Lord", |_| {})
            .await
            .unwrap();

        assert_eq!(
            result.entities_created, 1,
            "semantic-only hit should still extract"
        );
    }

    // ── Unit: profile prompt + parsing (second-pass enrichment) ──────────────

    #[test]
    fn build_profile_prompt_anchors_on_name_and_omits_relations() {
        let prompt = build_profile_prompt("The Iron Fist", "The Iron Fist rules the docks.");
        assert!(prompt.contains("The Iron Fist"));
        assert!(prompt.contains("The Iron Fist rules the docks."));
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("notes"));
        // The profile pass must NOT ask for relations (depth-1, description only).
        assert!(!prompt.contains("\"relations\""));
    }

    #[test]
    fn parse_profile_response_extracts_summary_and_notes() {
        let json = r#"{"summary":"A militant faction.","notes":"Led by [[Varn]]."}"#;
        let fields = parse_profile_response(json);
        assert_eq!(fields.summary.as_deref(), Some("A militant faction."));
        assert_eq!(fields.notes.as_deref(), Some("Led by [[Varn]]."));
    }

    #[test]
    fn parse_profile_response_returns_empty_on_malformed_json() {
        let fields = parse_profile_response("not json {{{");
        assert!(fields.summary.is_none());
        assert!(fields.notes.is_none());
    }

    // ── MockLlm that branches between the seed prompt and the profile prompt ──

    /// Returns `seed` for prompts that request relations (seed/extraction
    /// schema) and `profile` for the description-only profile prompt.
    struct BranchingLlm {
        seed: String,
        profile: String,
    }

    #[async_trait::async_trait]
    impl LlmProvider for BranchingLlm {
        fn provider_type(&self) -> &'static str {
            "mock_branching"
        }
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            messages: &[ChatMessage],
        ) -> Result<
            tokio::sync::mpsc::Receiver<Result<String, crate::providers::llm_provider::LlmError>>,
            crate::providers::llm_provider::LlmError,
        > {
            let is_seed = messages
                .first()
                .map(|m| m.content.contains("\"relations\""))
                .unwrap_or(false);
            let resp = if is_seed {
                self.seed.clone()
            } else {
                self.profile.clone()
            };
            let (tx, rx) = tokio::sync::mpsc::channel(4);
            tokio::spawn(async move {
                let _ = tx.send(Ok(resp)).await;
            });
            Ok(rx)
        }
    }

    async fn link_campaign_to_collection(
        db: &surrealdb::Surreal<surrealdb::engine::local::Db>,
        col_id: &str,
    ) {
        db.query(
            "CREATE campaign SET id='camp1', name='C', system='5e', created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "LET $in  = type::thing('campaign',   'camp1'); \
             LET $out = type::thing('collection', $cid); \
             RELATE $in->subscribes_to->$out SET created_at=time::now()",
        )
        .bind(("cid", col_id.to_owned()))
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn seed_anchored_enriches_neighbor_when_setting_enabled() {
        let (db, col_id) = setup_db_with_collection().await;
        link_campaign_to_collection(&db, &col_id).await;
        crate::services::settings_service::upsert(&db, "extraction_enrich_neighbors", "true")
            .await
            .unwrap();

        // Seed pass: neighbor "The Iron Fist" gets a relation-flavored summary.
        let llm: Arc<dyn LlmProvider> = Arc::new(BranchingLlm {
            seed: r#"{"entities":[{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{"name":"The Iron Fist","kind":"faction","rel_type":"commands","summary":"The militia Varn commands.","notes":null}]}]}"#.to_string(),
            profile: r#"{"summary":"A militant faction controlling the eastern docks.","notes":"Led by [[Commander Varn]]."}"#.to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
        let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

        extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
            .await
            .unwrap();

        let factions = entity_service::get_by_collection(&db, &col_id, EntityKind::Faction)
            .await
            .unwrap();
        let fist = factions
            .iter()
            .find(|n| n.name == "The Iron Fist")
            .expect("neighbor should exist");
        assert_eq!(
            fist.summary.as_deref(),
            Some("A militant faction controlling the eastern docks."),
            "enrichment should replace the relation-flavored summary with an entity-centric one"
        );
        assert_eq!(fist.notes.as_deref(), Some("Led by [[Commander Varn]]."));
    }

    #[tokio::test]
    async fn seed_anchored_skips_enrichment_when_setting_disabled() {
        let (db, col_id) = setup_db_with_collection().await;
        link_campaign_to_collection(&db, &col_id).await;
        // Setting left unset → defaults to off.

        let llm: Arc<dyn LlmProvider> = Arc::new(BranchingLlm {
            seed: r#"{"entities":[{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{"name":"The Iron Fist","kind":"faction","rel_type":"commands","summary":"The militia Varn commands.","notes":null}]}]}"#.to_string(),
            profile: r#"{"summary":"SHOULD NOT BE USED","notes":null}"#.to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
        let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

        extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
            .await
            .unwrap();

        let factions = entity_service::get_by_collection(&db, &col_id, EntityKind::Faction)
            .await
            .unwrap();
        let fist = factions.iter().find(|n| n.name == "The Iron Fist").unwrap();
        assert_eq!(
            fist.summary.as_deref(),
            Some("The militia Varn commands."),
            "without the setting, the first-pass summary must be left untouched"
        );
    }

    #[tokio::test]
    async fn seed_anchored_caps_enrichment_at_max() {
        let (db, col_id) = setup_db_with_collection().await;
        link_campaign_to_collection(&db, &col_id).await;
        crate::services::settings_service::upsert(&db, "extraction_enrich_neighbors", "true")
            .await
            .unwrap();

        // Seed response with MAX_ENRICH + 1 distinct neighbors.
        let mut rels = String::new();
        for i in 0..(MAX_ENRICH + 1) {
            if i > 0 {
                rels.push(',');
            }
            rels.push_str(&format!(
                r#"{{"name":"Neighbor{i}","kind":"npc","rel_type":"knows","summary":"rel{i}","notes":null}}"#
            ));
        }
        let seed = format!(
            r#"{{"entities":[{{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{rels}]}}]}}"#
        );

        // Non-empty semantic results so every neighbor search yields passages.
        let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
            results: vec![SearchResult {
                chunk_id: "chunk:sem".to_string(),
                source_id: "source:s1".to_string(),
                source_name: "Book".to_string(),
                text: "Some descriptive passage about a figure.".to_string(),
                page_start: 1,
                page_end: 1,
                section_heading: "Lore".to_string(),
                source_type: "lore".to_string(),
                distance: 0.1,
            }],
        });
        let llm: Arc<dyn LlmProvider> = Arc::new(BranchingLlm {
            seed,
            profile: r#"{"summary":"PROFILED","notes":null}"#.to_string(),
        });
        let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

        extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
            .await
            .unwrap();

        let npcs = entity_service::get_by_collection(&db, &col_id, EntityKind::Npc)
            .await
            .unwrap();
        let enriched = npcs
            .iter()
            .filter(|n| n.summary.as_deref() == Some("PROFILED"))
            .count();
        assert_eq!(
            enriched, MAX_ENRICH,
            "enrichment must be capped at MAX_ENRICH neighbors"
        );
    }
}
