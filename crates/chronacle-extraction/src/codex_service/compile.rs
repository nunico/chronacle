//! Setting-compile pipeline: stale entities → grounded codex articles.
//!
//! For each entity needing compile (stale, unset-stale, or article-less),
//! gathers scoped source passages via the vector store plus 1-hop graph
//! neighbors, asks the LLM to write grounded prose citing every claim, then
//! persists the article, its provenance (`codex_sources`), and re-embeds the
//! entity so retrieval picks up the richer text.

use std::sync::Arc;

use surrealdb::Connection;

use super::prompts::build_article_prompt;
use super::{CodexError, CodexPhase, CompileProgress, CompileResult, MAX_COMPILE_PER_RUN};
use crate::entity_service::{self, EntityKind, GraphNode};
use crate::extraction_service::llm_complete;
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmProvider};
use chronacle_core::vector_store::VectorStore;

/// The eight node tables, in a stable order for deterministic compile runs.
const ALL_KINDS: [EntityKind; 8] = [
    EntityKind::Npc,
    EntityKind::Location,
    EntityKind::Faction,
    EntityKind::Creature,
    EntityKind::Item,
    EntityKind::Event,
    EntityKind::PlayerCharacter,
    EntityKind::Misc,
];

/// Number of semantic passage hits fetched per entity compile.
const COMPILE_SEARCH_K: u64 = 8;

/// Cap on 1-hop neighbors surfaced to the article prompt.
const MAX_NEIGHBORS: usize = 12;

/// Resolve the bare collection ids a compiled article for `collection_id` may
/// cite: the owner campaign's full subscription set for a campaign-owned
/// collection (ADR-009's auto-owned collection is itself one of those
/// subscriptions); just the collection itself for a regular (shareable) one.
async fn provenance_scope<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
) -> Result<Vec<String>, CodexError> {
    #[derive(serde::Deserialize)]
    struct Row {
        owner_campaign: Option<surrealdb::sql::Thing>,
    }
    let mut resp = db
        .query("SELECT owner_campaign FROM type::thing('collection', $cid)")
        .bind(("cid", collection_id.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let row: Option<Row> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    match row.and_then(|r| r.owner_campaign) {
        Some(cam) => subscribed_collection_ids(db, &cam.id.to_raw()).await,
        None => Ok(vec![collection_id.to_string()]),
    }
}

/// Bare collection ids a campaign subscribes to (mirrors
/// `chronacle_retrieval::agent_service::resolve_collection_ids`, duplicated
/// here since this crate does not depend on chronacle-retrieval).
async fn subscribed_collection_ids<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<String>, CodexError> {
    #[derive(serde::Deserialize)]
    struct Row {
        out: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query("SELECT out FROM subscribes_to WHERE in = type::thing('campaign', $cam)")
        .bind(("cam", campaign_id.to_owned()))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let rows: Vec<Row> = resp.take(0).map_err(|e| CodexError::Db(e.to_string()))?;
    Ok(rows.into_iter().map(|r| r.out.id.to_raw()).collect())
}

/// Entities in the collection needing compile (stale, unset, or article-less),
/// capped at `MAX_COMPILE_PER_RUN`; returns `(targets, remaining_count)`.
///
/// Fetches every kind via the already-tested `get_by_collection` (rather than
/// hand-rolling a cross-table SurrealQL query) and filters/caps in Rust.
async fn compile_targets<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
) -> Result<(Vec<GraphNode>, usize), CodexError> {
    compile_targets_with_cap(db, collection_id, MAX_COMPILE_PER_RUN).await
}

/// Same as [`compile_targets`] but with an explicit cap, so tests can pin
/// cap-overflow behavior (`remaining_stale` honesty) without creating
/// `MAX_COMPILE_PER_RUN + 1` entities.
pub(super) async fn compile_targets_with_cap<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
    cap: usize,
) -> Result<(Vec<GraphNode>, usize), CodexError> {
    let mut stale: Vec<GraphNode> = Vec::new();
    for kind in ALL_KINDS {
        let nodes = entity_service::get_by_collection(db, collection_id, kind)
            .await
            .map_err(|e| CodexError::Db(e.to_string()))?;
        stale.extend(
            nodes
                .into_iter()
                .filter(|n| n.codex_stale != Some(false) || n.codex_article.is_none()),
        );
    }
    // Deterministic order across the merged tables.
    stale.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.cmp(&b.name)));

    let total = stale.len();
    let targets: Vec<GraphNode> = stale.into_iter().take(cap).collect();
    let remaining = total.saturating_sub(targets.len());
    Ok((targets, remaining))
}

/// Compile one entity: gather scoped passages + neighbors, prompt, persist,
/// re-embed. Returns false when no passage context exists (skip, not error) —
/// this is a deliberate guard against hallucinated articles with no grounding.
async fn compile_one<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    vector_store: &Arc<dyn VectorStore>,
    node: &GraphNode,
    scope: &[String],
    outbound: &dyn chronacle_core::VaultOutbound,
) -> Result<bool, CodexError> {
    let query_text =
        entity_service::embed_text(&node.name, node.summary.as_deref(), node.notes.as_deref());
    let query_vec = embed
        .embed_documents(vec![query_text])
        .await
        .map_err(|e| CodexError::Embedding(e.to_string()))?
        .into_iter()
        .next()
        .unwrap_or_default();

    let hits = vector_store
        .search(&query_vec, scope, COMPILE_SEARCH_K)
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    if hits.is_empty() {
        return Ok(false);
    }

    let passages = hits
        .iter()
        .map(|h| {
            format!(
                "[Source: \"{}\", p.{}-{}]\n{}",
                h.source_name, h.page_start, h.page_end, h.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let relations = entity_service::get_entity_relations(db, &node.id, &node.kind)
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    let neighbors: Vec<(String, String)> = relations
        .into_iter()
        .take(MAX_NEIGHBORS)
        .map(|r| (r.name, r.rel_type))
        .collect();

    let prompt = build_article_prompt(
        &node.name,
        &node.kind,
        node.summary.as_deref(),
        node.notes.as_deref(),
        &neighbors,
        &passages,
    );
    let system_prompt =
        "You are a meticulous TTRPG lore archivist. Ground every statement in the provided sources.";
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];
    let article = llm_complete(llm.as_ref(), system_prompt, &messages)
        .await
        .map_err(|e| CodexError::Llm(e.to_string()))?;

    // An empty LLM response must not clear staleness or persist a blank
    // article — leave the entity flagged stale so the next run retries it.
    if article.trim().is_empty() {
        return Ok(false);
    }

    /// One cited chunk's provenance, persisted verbatim into `codex_sources`.
    ///
    /// A plain `Serialize` struct — NEVER `serde_json::Value` — so the bind
    /// round-trips correctly into the FLEXIBLE `array<object>` schema field.
    #[derive(serde::Serialize)]
    struct ChunkSource {
        kind: &'static str,
        chunk: String,
        source_name: String,
        page_start: i64,
        page_end: i64,
    }
    let sources: Vec<ChunkSource> = hits
        .iter()
        .map(|h| ChunkSource {
            kind: "chunk",
            chunk: h.chunk_id.trim_start_matches("chunk:").to_string(),
            source_name: h.source_name.clone(),
            page_start: h.page_start,
            page_end: h.page_end,
        })
        .collect();

    db.query(
        "UPDATE type::thing($table, $id) SET \
             codex_article = $article, \
             codex_compiled_at = time::now(), \
             codex_stale = false, \
             codex_sources = $sources",
    )
    .bind(("table", node.kind.clone()))
    .bind(("id", node.id.clone()))
    .bind(("article", article.clone()))
    .bind(("sources", sources))
    .await
    .map_err(|e| CodexError::Db(e.to_string()))?
    .check()
    .map_err(|e| CodexError::Db(e.to_string()))?;

    outbound.enqueue(chronacle_core::VaultRef {
        table: node.kind.clone(),
        id: node.id.clone(),
    });

    let updated = GraphNode {
        codex_article: Some(article.clone()),
        codex_stale: Some(false),
        ..node.clone()
    };
    embed_entity_with_article(db, embed, &updated).await?;

    Ok(true)
}

/// See module docs. `on_progress` receives Resolving → Compiling (per entity)
/// → Done|Empty.
pub async fn compile_collection<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    vector_store: &Arc<dyn VectorStore>,
    collection_id: &str,
    on_progress: impl Fn(CompileProgress),
    outbound: &dyn chronacle_core::VaultOutbound,
) -> Result<CompileResult, CodexError> {
    on_progress(CompileProgress {
        phase: CodexPhase::Resolving,
        detail: "Resolving compile scope".to_string(),
        compiled: 0,
        total: 0,
    });
    let scope = provenance_scope(db, collection_id).await?;
    let (targets, remaining) = compile_targets(db, collection_id).await?;
    let total = targets.len();

    if total == 0 {
        on_progress(CompileProgress {
            phase: CodexPhase::Empty,
            detail: "No entities need compiling".to_string(),
            compiled: 0,
            total: 0,
        });
        return Ok(CompileResult {
            articles_compiled: 0,
            remaining_stale: remaining,
        });
    }

    let mut compiled = 0usize;
    for node in &targets {
        on_progress(CompileProgress {
            phase: CodexPhase::Compiling,
            detail: format!("Compiling {}", node.name),
            compiled,
            total,
        });
        // Best-effort: a failed compile must not abort the whole run.
        match compile_one(db, llm, embed, vector_store, node, &scope, outbound).await {
            Ok(true) => {
                compiled += 1;
                on_progress(CompileProgress {
                    phase: CodexPhase::Embedding,
                    detail: format!("Embedding {}", node.name),
                    compiled,
                    total,
                });
            }
            Ok(false) => {}
            Err(e) => {
                eprintln!(
                    "codex: compile failed for {} ({}): {e}",
                    node.name, node.kind
                );
            }
        }
    }

    on_progress(CompileProgress {
        phase: CodexPhase::Done,
        detail: format!("Compiled {compiled} of {total} articles"),
        compiled,
        total,
    });

    Ok(CompileResult {
        articles_compiled: compiled,
        remaining_stale: remaining,
    })
}

/// Compile a single entity by table + id (per-entity Recompile in the UI).
pub async fn compile_entity<C: Connection>(
    db: &surrealdb::Surreal<C>,
    llm: &Arc<dyn LlmProvider>,
    embed: &Arc<dyn EmbeddingProvider>,
    vector_store: &Arc<dyn VectorStore>,
    kind: &str,
    id: &str,
    outbound: &dyn chronacle_core::VaultOutbound,
) -> Result<bool, CodexError> {
    let entity_kind = EntityKind::from_table(kind).map_err(|e| CodexError::Db(e.to_string()))?;
    let node = entity_service::get_by_id(db, id, entity_kind)
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;

    let scope = if let Some(col) = node.collection_id.as_deref() {
        provenance_scope(db, col).await?
    } else if let Some(cam) = node.campaign_id.as_deref() {
        subscribed_collection_ids(db, cam).await?
    } else {
        Vec::new()
    };

    compile_one(db, llm, embed, vector_store, &node, &scope, outbound).await
}

/// Embed name + summary + notes + article; zero-length-vector no-op like
/// `entity_service::embed_node` (guards mock/unavailable embedding providers).
pub(crate) async fn embed_entity_with_article<C: Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    node: &GraphNode,
) -> Result<(), CodexError> {
    let mut text =
        entity_service::embed_text(&node.name, node.summary.as_deref(), node.notes.as_deref());
    if let Some(article) = node.codex_article.as_deref() {
        let article = article.trim();
        if !article.is_empty() {
            text.push('\n');
            text.push_str(article);
        }
    }

    let vecs = embed
        .embed_documents(vec![text])
        .await
        .map_err(|e| CodexError::Embedding(e.to_string()))?;
    let vec = vecs.into_iter().next().unwrap_or_default();
    if vec.is_empty() {
        return Ok(());
    }

    let model = embed.model_name().to_owned();
    db.query("UPDATE type::thing($table, $id) SET embedding = $vec, embed_model = $model")
        .bind(("table", node.kind.clone()))
        .bind(("id", node.id.clone()))
        .bind(("vec", vec))
        .bind(("model", model))
        .await
        .map_err(|e| CodexError::Db(e.to_string()))?;
    Ok(())
}
