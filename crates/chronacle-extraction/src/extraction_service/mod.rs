//! Extraction service — LLM-powered entity extraction from collection chunks.
//!
//! Reads chunks belonging to a collection, batches them by token budget, calls
//! the LLM per batch to extract structured entity data (2 levels deep), then
//! persists entities and relations via `entity_service`. Collection entities are
//! embedded immediately after creation for later vector retrieval in
//! `agent_service::fetch_entity_context`.
//!
//! Split by stage:
//! - [`prompts`] — prompt construction + shared kind/relationship vocab
//! - [`parse`] — the LLM JSON schema and tolerant parsers
//! - [`persist`] — dedup-or-create entities/relations from a parsed batch
//! - [`enrich`] — passage search and the opt-in second-pass enrichment
//! - [`collection`] — `extract_from_collection` (full-collection sweep)
//! - [`seed`] — `extract_seed_anchored` (single named entity + neighbors)

mod collection;
mod enrich;
mod parse;
mod persist;
mod prompts;
mod seed;

#[cfg(test)]
pub(crate) mod test_support;

pub use collection::extract_from_collection;
pub use seed::extract_seed_anchored;

use serde::Serialize;

use crate::entity_service::GraphNode;
use chronacle_core::llm::{ChatMessage, LlmProvider};

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

// ── Shared constants ──────────────────────────────────────────────────────────

/// Approximate characters per token for batching heuristic (~4 chars/token).
const CHARS_PER_TOKEN: usize = 4;
/// Target token budget per LLM batch.
const BATCH_TOKEN_BUDGET: usize = 4000;
const BATCH_CHAR_BUDGET: usize = BATCH_TOKEN_BUDGET * CHARS_PER_TOKEN;

/// Maximum number of neighbor entities to enrich in the second pass. Caps the
/// extra LLM + embedding cost of seed-anchored enrichment (opt-in via the
/// `extraction_enrich_neighbors` setting).
pub(crate) const MAX_ENRICH: usize = 20;

/// Number of semantic neighbours to fetch per collection for seed extraction.
const SEED_SEARCH_K: u64 = 12;

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Drain the streaming LLM channel into a complete response string.
pub(crate) async fn llm_complete(
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
