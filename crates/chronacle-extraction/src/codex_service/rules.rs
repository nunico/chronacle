//! Rules-compile pipeline: rules/supplement passages → distilled, categorized
//! rule entries (`rule_entry` table), deduplicated by `(collection, name)`.
//!
//! Mirrors `compile.rs`'s shape (progress events, per-item failure isolation,
//! zero-vector embedding no-op guard) but operates over batches of labeled
//! chunks rather than one entity at a time, since a single rules PDF yields
//! many discrete rules per passage.

use std::sync::Arc;

use serde::Deserialize;
use surrealdb::sql::Thing;
use surrealdb::Connection;

use super::prompts::{build_rules_prompt, build_rules_redo_prompt};
use super::{
    CodexError, CodexPhase, CompileProgress, RuleEntry, RulePageRef, RulesCompileResult,
    MAX_RULE_BATCHES_PER_RUN, RULE_CATEGORIES,
};
use crate::extraction_service::llm_complete;
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmProvider};

/// Character budget per LLM batch (mirrors `extraction_service`'s token-based
/// budget; duplicated here rather than exposed cross-module since the two
/// batchers keep per-chunk labels differently).
const BATCH_CHAR_BUDGET: usize = 16_000;

const SYSTEM_PROMPT: &str =
    "You are a meticulous TTRPG rules editor. Extract only what the passages actually say.";

// ── Tolerant JSON parsing (mirrors extraction_service::parse's fence-stripping) ──

fn strip_code_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(s) = trimmed.strip_prefix("```json") {
        s.trim_end_matches("```").trim()
    } else if let Some(s) = trimmed.strip_prefix("```") {
        s.trim_end_matches("```").trim()
    } else {
        trimmed
    }
}

#[derive(Debug, Default, Deserialize)]
struct RulesLlmResponse {
    #[serde(default)]
    entries: Vec<RulesLlmEntry>,
}

#[derive(Debug, Deserialize)]
struct RulesLlmEntry {
    name: String,
    #[serde(default)]
    category: String,
    body: String,
    #[serde(default)]
    page_refs: Vec<RulePageRef>,
}

fn parse_rules_response(raw: &str) -> RulesLlmResponse {
    serde_json::from_str(strip_code_fences(raw)).unwrap_or_else(|e| {
        eprintln!("codex: rules JSON parse failed ({e}), returning empty result");
        RulesLlmResponse::default()
    })
}

fn normalize_category(category: &str) -> String {
    if RULE_CATEGORIES.contains(&category) {
        category.to_string()
    } else {
        "entry".to_string()
    }
}

// ── Chunk gathering + batching ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ChunkRow {
    text: String,
    page_start: i64,
    page_end: i64,
    source_name: String,
}

fn label_chunk(c: &ChunkRow) -> String {
    format!(
        "[Source: \"{}\", p.{}-{}]\n{}",
        c.source_name, c.page_start, c.page_end, c.text
    )
}

/// Fetch rules/supplement chunks for a collection, labeled with their source
/// and page range.
async fn rules_chunks<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
) -> Result<Vec<ChunkRow>, CodexError> {
    let mut resp = db
        .query(
            "SELECT text, page_start, page_end, source_type, \
                 source.display_name AS source_name FROM chunk \
             WHERE collection = type::thing('collection', $cid) \
                 AND source_type IN ['rules', 'supplement']",
        )
        .bind(("cid", collection_id.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    resp.take(0).map_err(|e| CodexError::Db(e.to_string()))
}

/// Split labeled chunk strings into character-budgeted batches, one
/// LLM call each; each chunk keeps its own `[Source: …]` label.
fn batch_labeled_chunks(labeled: Vec<String>) -> Vec<String> {
    let mut batches: Vec<String> = Vec::new();
    let mut current = String::new();
    for c in labeled {
        if !current.is_empty() && current.len() + c.len() > BATCH_CHAR_BUDGET {
            batches.push(std::mem::take(&mut current));
        }
        current.push_str(&c);
        current.push_str("\n\n---\n\n");
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

// ── Embedding ────────────────────────────────────────────────────────────────

/// Embed `name (category): body`; zero-length-vector no-op like
/// `compile::embed_entity_with_article` (guards mock/unavailable providers).
async fn embed_rule_entry<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    entry_id: &str,
    name: &str,
    category: &str,
    body: &str,
) -> Result<(), CodexError> {
    let text = format!("{name} ({category}): {body}");
    let vecs = embed
        .embed_documents(vec![text])
        .await
        .map_err(|e| CodexError::Embedding(e.to_string()))?;
    let vec = vecs.into_iter().next().unwrap_or_default();
    if vec.is_empty() {
        return Ok(());
    }
    let model = embed.model_name().to_owned();
    db.query("UPDATE type::thing('rule_entry', $id) SET embedding = $vec, embed_model = $model")
        .bind(("id", entry_id.to_owned()))
        .bind(("vec", vec))
        .bind(("model", model))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    Ok(())
}

// ── Dedup-merge persistence ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ExistingRow {
    id: Thing,
}

/// Persist one parsed LLM entry: UPDATE (preserving notes/sources, merging
/// page_refs) if `(collection, name)` already exists, else CREATE.
/// Returns `(entry_id, was_update)`.
async fn upsert_rule_entry<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
    entry: &RulesLlmEntry,
) -> Result<(String, bool), CodexError> {
    let category = normalize_category(&entry.category);

    let mut resp = db
        .query(
            "SELECT id FROM rule_entry \
             WHERE collection = type::thing('collection', $cid) AND name = $name LIMIT 1",
        )
        .bind(("cid", collection_id.to_owned()))
        .bind(("name", entry.name.clone()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let existing: Vec<ExistingRow> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;

    if let Some(row) = existing.into_iter().next() {
        let id = row.id.id.to_raw();
        db.query(
            "UPDATE type::thing('rule_entry', $id) SET \
                 body = $body, category = $category, compiled_at = time::now(), \
                 stale = false, page_refs = array::union(page_refs, $page_refs)",
        )
        .bind(("id", id.clone()))
        .bind(("body", entry.body.clone()))
        .bind(("category", category))
        .bind(("page_refs", entry.page_refs.clone()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?
        .check()
        .map_err(|e| CodexError::Db(e.to_string()))?;
        Ok((id, true))
    } else {
        let mut resp = db
            .query(
                "CREATE rule_entry SET collection = type::thing('collection', $cid), \
                     name = $name, category = $category, body = $body, \
                     page_refs = $page_refs, compiled_at = time::now(), stale = false \
                     RETURN VALUE id",
            )
            .bind(("cid", collection_id.to_owned()))
            .bind(("name", entry.name.clone()))
            .bind(("category", category))
            .bind(("body", entry.body.clone()))
            .bind(("page_refs", entry.page_refs.clone()))
            .await
            .map_err(|e| CodexError::Db(e.to_string()))?;
        let ids: Vec<Thing> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
        let id = ids
            .into_iter()
            .next()
            .ok_or_else(|| CodexError::Db("rule_entry create returned no id".to_string()))?
            .id
            .to_raw();
        Ok((id, false))
    }
}

/// Compile rules for a collection: gather rules/supplement chunks, batch them,
/// ask the LLM to extract discrete rules per batch, dedup-merge by name, and
/// embed each created/updated entry. `on_progress` receives Resolving →
/// Compiling (per batch) → Done|Empty.
pub async fn compile_rules<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    collection_id: &str,
    on_progress: impl Fn(CompileProgress),
) -> Result<RulesCompileResult, CodexError> {
    compile_rules_with_cap(
        db,
        llm,
        embed,
        collection_id,
        MAX_RULE_BATCHES_PER_RUN,
        on_progress,
    )
    .await
}

/// Same as [`compile_rules`] but with an explicit batch cap, so tests can pin
/// cap-overflow behavior (`remaining_batches` honesty) without generating
/// `MAX_RULE_BATCHES_PER_RUN + 1` batches worth of chunk text.
pub(super) async fn compile_rules_with_cap<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    collection_id: &str,
    cap: usize,
    on_progress: impl Fn(CompileProgress),
) -> Result<RulesCompileResult, CodexError> {
    on_progress(CompileProgress {
        phase: CodexPhase::Resolving,
        detail: "Resolving rules content".to_string(),
        compiled: 0,
        total: 0,
    });

    let chunks = rules_chunks(db, collection_id).await?;
    if chunks.is_empty() {
        on_progress(CompileProgress {
            phase: CodexPhase::Empty,
            detail: "No rules/supplement content to compile".to_string(),
            compiled: 0,
            total: 0,
        });
        return Ok(RulesCompileResult {
            entries_created: 0,
            entries_updated: 0,
            remaining_batches: 0,
        });
    }

    let labeled: Vec<String> = chunks.iter().map(label_chunk).collect();
    let all_batches = batch_labeled_chunks(labeled);
    let total_batches = all_batches.len();
    let remaining_batches = total_batches.saturating_sub(cap);
    let batches: Vec<String> = all_batches.into_iter().take(cap).collect();
    let total = batches.len();

    let mut created = 0usize;
    let mut updated = 0usize;
    for (i, batch) in batches.iter().enumerate() {
        on_progress(CompileProgress {
            phase: CodexPhase::Compiling,
            detail: format!("Compiling rules batch {}/{total}", i + 1),
            compiled: created + updated,
            total,
        });

        let prompt = build_rules_prompt(batch);
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }];
        // Best-effort: a failed batch must not abort the whole run.
        let raw = match llm_complete(llm.as_ref(), SYSTEM_PROMPT, &messages).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("codex: rules batch {} failed: {e}", i + 1);
                continue;
            }
        };
        let parsed = parse_rules_response(&raw);
        for entry in &parsed.entries {
            if entry.name.trim().is_empty() || entry.body.trim().is_empty() {
                continue;
            }
            match upsert_rule_entry(db, collection_id, entry).await {
                Ok((id, was_update)) => {
                    if was_update {
                        updated += 1;
                    } else {
                        created += 1;
                    }
                    if let Err(e) = embed_rule_entry(
                        db,
                        embed,
                        &id,
                        &entry.name,
                        &normalize_category(&entry.category),
                        &entry.body,
                    )
                    .await
                    {
                        eprintln!("codex: embedding rule entry '{}' failed: {e}", entry.name);
                    }
                }
                Err(e) => {
                    eprintln!("codex: persisting rule entry '{}' failed: {e}", entry.name);
                }
            }
        }
    }

    on_progress(CompileProgress {
        phase: CodexPhase::Done,
        detail: format!("Compiled {} rule entries", created + updated),
        compiled: created + updated,
        total,
    });

    Ok(RulesCompileResult {
        entries_created: created,
        entries_updated: updated,
        remaining_batches,
    })
}

// ── Redo (objection-driven regeneration) ─────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SourceEntry {
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuleEntryRow {
    id: Thing,
    name: String,
    body: String,
    #[serde(default)]
    sources: Vec<SourceEntry>,
    collection: Thing,
}

/// Regenerate one rule entry, honoring every stored GM objection plus the new
/// one. Appends the new objection to `sources`; preserves `notes`.
pub async fn redo_rule_entry<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    rule_entry_id: &str,
    objection: &str,
) -> Result<(), CodexError> {
    let mut resp = db
        .query(
            "SELECT id, name, body, sources, collection \
             FROM type::thing('rule_entry', $id)",
        )
        .bind(("id", rule_entry_id.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let rows: Vec<RuleEntryRow> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    let entry = rows
        .into_iter()
        .next()
        .ok_or_else(|| CodexError::Db(format!("rule_entry {rule_entry_id} not found")))?;
    let current_body = entry.body.clone();

    let mut objections: Vec<String> = entry
        .sources
        .iter()
        .filter(|s| s.kind == "objection")
        .filter_map(|s| s.text.clone())
        .collect();
    objections.push(objection.to_string());

    let collection_id = entry.collection.id.to_raw();
    let name_lower = entry.name.to_lowercase();
    let mut resp = db
        .query(
            "SELECT text, page_start, page_end, source_type, \
                 source.display_name AS source_name FROM chunk \
             WHERE collection = type::thing('collection', $cid) \
                 AND source_type IN ['rules', 'supplement'] \
                 AND string::contains(string::lowercase(text), $needle)",
        )
        .bind(("cid", collection_id))
        .bind(("needle", name_lower))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let chunks: Vec<ChunkRow> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    let labeled = chunks
        .iter()
        .map(label_chunk)
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let prompt = build_rules_redo_prompt(&entry.name, &current_body, &objections, &labeled);
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];
    let raw = llm_complete(llm.as_ref(), SYSTEM_PROMPT, &messages)
        .await
        .map_err(|e| CodexError::Llm(e.to_string()))?;
    let parsed = parse_rules_response(&raw);
    let regenerated = parsed.entries.into_iter().next().ok_or_else(|| {
        CodexError::Llm(format!(
            "redo produced no entry for rule '{}': {raw}",
            entry.name
        ))
    })?;
    let category = normalize_category(&regenerated.category);

    let id = entry.id.id.to_raw();
    db.query(
        "UPDATE type::thing('rule_entry', $id) SET \
             body = $body, category = $category, page_refs = $page_refs, \
             compiled_at = time::now(), stale = false, \
             sources = array::append(sources, { kind: 'objection', text: $objection, at: time::now() })",
    )
    .bind(("id", id.clone()))
    .bind(("body", regenerated.body.clone()))
    .bind(("category", category.clone()))
    .bind(("page_refs", regenerated.page_refs.clone()))
    .bind(("objection", objection.to_owned()))
    .await
    .map_err(|e| CodexError::Db(e.to_string()))?
    .check()
    .map_err(|e| CodexError::Db(e.to_string()))?;

    embed_rule_entry(db, embed, &id, &entry.name, &category, &regenerated.body).await?;

    Ok(())
}

// ── Simple queries ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RuleEntryListRow {
    id: Thing,
    name: String,
    category: String,
    body: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    page_refs: Vec<RulePageRef>,
    stale: bool,
}

impl From<RuleEntryListRow> for RuleEntry {
    fn from(r: RuleEntryListRow) -> Self {
        Self {
            id: r.id.id.to_raw(),
            name: r.name,
            category: r.category,
            body: r.body,
            notes: r.notes,
            page_refs: r.page_refs,
            stale: r.stale,
        }
    }
}

/// List all rule entries for a collection, ordered by name.
pub async fn list_rule_entries<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
) -> Result<Vec<RuleEntry>, String> {
    let mut resp = db
        .query(
            "SELECT id, name, category, body, notes, page_refs, stale FROM rule_entry \
             WHERE collection = type::thing('collection', $cid) ORDER BY name",
        )
        .bind(("cid", collection_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to list rule entries: {e}"))?;
    let rows: Vec<RuleEntryListRow> = resp
        .take(0)
        .map_err(|e| format!("Failed to parse rule entries: {e}"))?;
    Ok(rows.into_iter().map(RuleEntry::from).collect())
}

/// Update a rule entry's GM notes (freeform, not LLM-derived).
pub async fn update_rule_notes<C: Connection>(
    db: &surrealdb::Surreal<C>,
    rule_entry_id: &str,
    notes: Option<String>,
) -> Result<(), String> {
    db.query("UPDATE type::thing('rule_entry', $id) SET notes = $notes")
        .bind(("id", rule_entry_id.to_owned()))
        .bind(("notes", notes))
        .await
        .map_err(|e| format!("Failed to update rule notes: {e}"))?
        .check()
        .map_err(|e| format!("Failed to update rule notes: {e}"))?;
    Ok(())
}
