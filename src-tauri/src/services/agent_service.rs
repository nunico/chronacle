/// Agent service — orchestrates the RAG pipeline.
///
/// 1. Persists the user message.
/// 2. Embeds the query via fastembed.
/// 3. Retrieves relevant chunks from the vector store.
/// 4. Builds a context-augmented prompt with citation instructions.
/// 5. Streams the LLM response through an mpsc channel.
///
/// The caller receives token chunks. After the channel is exhausted
/// the caller should persist the assistant message with parsed citations.
use tokio::sync::mpsc;

use std::sync::Arc;

use crate::providers::llm_provider::{ChatMessage, LlmError};
use crate::AppState;
use surrealdb::Connection;

/// Errors from the agent pipeline.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Embedding error: {0}")]
    Embedding(String),
    #[error("Retrieval error: {0}")]
    Retrieval(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Database error: {0}")]
    Db(String),
}

/// Resolve the collection IDs that a campaign is subscribed to.
///
/// Queries the `subscribes_to` relation for the given `campaign_id` and
/// returns the bare IDs (no `table:` prefix) of all subscribed collections.
/// Returns an empty `Vec` when the campaign has no subscriptions.
pub async fn resolve_collection_ids<C>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<String>, AgentError>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT out FROM subscribes_to WHERE in = type::thing('campaign', $id)")
        .bind(("id", campaign_id.to_owned()))
        .await
        .map_err(|e| AgentError::Db(e.to_string()))?;

    #[derive(serde::Deserialize)]
    struct Row {
        out: surrealdb::sql::Thing,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(rows.into_iter().map(|r| r.out.id.to_raw()).collect())
}

/// Max characters of an entity/session note included in the context block.
/// Notes can be long; we include a leading excerpt so the LLM sees the GM's
/// own prose without letting a single entity dominate the prompt budget.
const NOTES_EXCERPT_LEN: usize = 280;

/// Format a notes field as a single-line context excerpt, or `None` when empty.
///
/// Newlines are collapsed to spaces so each entity stays on its own line, and
/// the text is truncated on a char boundary with an ellipsis when over budget.
fn notes_excerpt(notes: Option<&str>) -> Option<String> {
    let trimmed = notes?.trim();
    if trimmed.is_empty() {
        return None;
    }
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= NOTES_EXCERPT_LEN {
        Some(collapsed)
    } else {
        let truncated: String = collapsed.chars().take(NOTES_EXCERPT_LEN).collect();
        Some(format!("{truncated}…"))
    }
}

/// Query entity tables for a campaign (and optionally subscribed collections)
/// and format them as a context block.
///
/// Campaign-scoped entities are always included in full. Collection-scoped
/// entities are retrieved via MTREE KNN search when `query_embedding` is
/// `Some`, falling back to a full scan otherwise (tests, mock provider).
///
/// Returns an empty string when no entities are found.
pub async fn fetch_entity_context<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    collection_ids: &[String],
    query_embedding: Option<&[f32]>,
) -> Result<String, AgentError> {
    #[derive(serde::Deserialize)]
    struct BasicRow {
        name: String,
        summary: Option<String>,
        notes: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct PcRow {
        name: String,
        summary: Option<String>,
        notes: Option<String>,
        player_name: Option<String>,
        character_class: Option<String>,
        character_level: Option<i64>,
        status: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct EventRow {
        name: String,
        summary: Option<String>,
        notes: Option<String>,
        date_start: Option<String>,
        date_end: Option<String>,
    }

    #[derive(serde::Deserialize)]
    struct SessionRow {
        title: String,
        notes: Option<String>,
        date_played: Option<String>,
        session_number: Option<i64>,
    }

    // ── Campaign entities (always full scan) ─────────────────────────────────
    let mut resp = db
        .query("SELECT name, summary, notes, player_name, character_class, character_level, status FROM player_character WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM npc WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM location WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM faction WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM creature WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM item WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes, date_start, date_end FROM event WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT name, summary, notes FROM misc WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $cid)) ORDER BY name ASC")
        .query("SELECT title, notes, date_played, session_number FROM session WHERE campaign = type::thing('campaign', $cid) ORDER BY session_number ASC")
        .bind(("cid", campaign_id.to_owned()))
        .await
        .map_err(|e| AgentError::Db(e.to_string()))?;

    let pcs: Vec<PcRow> = resp.take(0).map_err(|e| AgentError::Db(e.to_string()))?;
    let npcs: Vec<BasicRow> = resp.take(1).map_err(|e| AgentError::Db(e.to_string()))?;
    let locations: Vec<BasicRow> = resp.take(2).map_err(|e| AgentError::Db(e.to_string()))?;
    let factions: Vec<BasicRow> = resp.take(3).map_err(|e| AgentError::Db(e.to_string()))?;
    let creatures: Vec<BasicRow> = resp.take(4).map_err(|e| AgentError::Db(e.to_string()))?;
    let items: Vec<BasicRow> = resp.take(5).map_err(|e| AgentError::Db(e.to_string()))?;
    let events: Vec<EventRow> = resp.take(6).map_err(|e| AgentError::Db(e.to_string()))?;
    let misc: Vec<BasicRow> = resp.take(7).map_err(|e| AgentError::Db(e.to_string()))?;
    let sessions: Vec<SessionRow> = resp.take(8).map_err(|e| AgentError::Db(e.to_string()))?;

    // ── Collection entities (top-k per table via MTREE, full scan as fallback) ─
    // Retrieved as a flat Vec<BasicRow> across all tables for the context block.
    let mut col_entities: Vec<(String, BasicRow)> = Vec::new(); // (kind, row)
    if !collection_ids.is_empty() {
        // Build a WHERE clause that matches entities in any of the given collections.
        // Each `collection:id->in_collection` traversal returns the entity IDs for
        // that collection; OR-ing them covers multiple subscriptions.
        let col_filter: String = collection_ids
            .iter()
            .map(|cid| {
                // Subquery form: find entity IDs via in_collection edges from the collection.
                let safe = cid.replace('\'', "\\'");
                format!("id IN (SELECT VALUE out FROM in_collection WHERE in = type::thing('collection', '{safe}'))")
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        for table in &[
            "npc",
            "location",
            "faction",
            "creature",
            "item",
            "event",
            "player_character",
            "misc",
        ] {
            let sql = if let Some(qv) = query_embedding {
                // MTREE KNN: order by cosine distance, top 10 per table.
                let vec_str = qv
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "SELECT name, summary, notes FROM {table} \
                     WHERE ({col_filter}) AND embedding IS NOT NONE \
                     ORDER BY embedding <|10|> [{vec_str}] LIMIT 10"
                )
            } else {
                // Full scan fallback (no embedding provider / test paths).
                format!("SELECT name, summary, notes FROM {table} WHERE {col_filter} LIMIT 50")
            };
            let mut r = db
                .query(sql)
                .await
                .map_err(|e| AgentError::Db(e.to_string()))?;
            let rows: Vec<BasicRow> = r.take(0).map_err(|e| AgentError::Db(e.to_string()))?;
            for row in rows {
                col_entities.push((table.to_string(), row));
            }
        }
    }

    if pcs.is_empty()
        && npcs.is_empty()
        && locations.is_empty()
        && factions.is_empty()
        && creatures.is_empty()
        && items.is_empty()
        && events.is_empty()
        && misc.is_empty()
        && sessions.is_empty()
        && col_entities.is_empty()
    {
        return Ok(String::new());
    }

    let mut out = String::from("Campaign notes (your GM records):\n");

    if !pcs.is_empty() {
        out.push('\n');
        for r in &pcs {
            out.push_str(&format!("[player_character] {}", r.name));
            if let Some(p) = &r.player_name {
                out.push_str(&format!(" · Player: {p}"));
            }
            if let Some(c) = &r.character_class {
                out.push_str(&format!(" · Class: {c}"));
            }
            if let Some(l) = r.character_level {
                out.push_str(&format!(" · Level: {l}"));
            }
            if let Some(s) = &r.status {
                out.push_str(&format!(" · Status: {s}"));
            }
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    for (rows, kind) in [
        (&npcs, "npc"),
        (&locations, "location"),
        (&factions, "faction"),
        (&creatures, "creature"),
        (&items, "item"),
    ] {
        if !rows.is_empty() {
            out.push('\n');
            for r in rows {
                out.push_str(&format!("[{kind}] {}", r.name));
                if let Some(s) = &r.summary {
                    if !s.trim().is_empty() {
                        out.push_str(&format!(" · {s}"));
                    }
                }
                if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                    out.push_str(&format!(" · Notes: {n}"));
                }
                out.push('\n');
            }
        }
    }

    if !events.is_empty() {
        out.push('\n');
        for r in &events {
            out.push_str(&format!("[event] {}", r.name));
            match (&r.date_start, &r.date_end) {
                (Some(s), Some(e)) if !s.trim().is_empty() && !e.trim().is_empty() => {
                    out.push_str(&format!(" · {s} → {e}"));
                }
                (Some(s), _) if !s.trim().is_empty() => {
                    out.push_str(&format!(" · {s}"));
                }
                _ => {}
            }
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    if !misc.is_empty() {
        out.push('\n');
        for r in &misc {
            out.push_str(&format!("[misc] {}", r.name));
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    if !sessions.is_empty() {
        out.push('\n');
        for r in &sessions {
            match r.session_number {
                Some(num) => out.push_str(&format!("[session {num}] {}", r.title)),
                None => out.push_str(&format!("[session] {}", r.title)),
            }
            if let Some(d) = &r.date_played {
                if !d.trim().is_empty() {
                    out.push_str(&format!(" · {d}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    // ── Collection entities section ──────────────────────────────────────────
    if !col_entities.is_empty() {
        out.push_str("\nCollection knowledge (from subscribed rulebooks):\n");
        for (kind, r) in &col_entities {
            out.push_str(&format!("[{kind}] {}", r.name));
            if let Some(s) = &r.summary {
                if !s.trim().is_empty() {
                    out.push_str(&format!(" · {s}"));
                }
            }
            if let Some(n) = notes_excerpt(r.notes.as_deref()) {
                out.push_str(&format!(" · Notes: {n}"));
            }
            out.push('\n');
        }
    }

    Ok(out)
}

/// Result of starting the streaming RAG pipeline: the token channel plus
/// whether the answer drew on any GM-secret retrieved material, so the caller
/// can flag the persisted assistant message and the chat UI can show a shield.
pub struct StreamHandle {
    pub rx: mpsc::Receiver<Result<String, LlmError>>,
    pub drew_from_gm_only: bool,
}

/// Run the full streaming RAG pipeline.
///
/// Returns a [`StreamHandle`]. Once its `rx` channel is exhausted, call
/// `persist_assistant_message` with the accumulated response, passing
/// `drew_from_gm_only` so the message is flagged.
pub async fn stream_response(
    state: &Arc<AppState>,
    message: &str,
    campaign_id: Option<&str>,
) -> Result<StreamHandle, AgentError> {
    // 1. Persist the user message
    persist_message(&state.db, "user", message, false, campaign_id).await?;

    // 2. Embed the query
    let embed_provider = state
        .embedding_provider
        .read()
        .map_err(|e| AgentError::Llm(format!("Embedding lock: {e}")))?
        .clone();
    let query_vector = embed_provider
        .embed_query(message)
        .await
        .map_err(|e| AgentError::Embedding(e.to_string()))?;

    // 3. Resolve collection IDs for the active campaign
    let collection_ids = match campaign_id {
        Some(cid) => resolve_collection_ids(&state.db, cid)
            .await
            .map_err(|e| AgentError::Retrieval(e.to_string()))?,
        None => Vec::new(),
    };

    let entity_context = match campaign_id {
        Some(cid) => fetch_entity_context(&state.db, cid, &collection_ids, Some(&query_vector))
            .await
            .unwrap_or_else(|e| {
                eprintln!("entity context fetch failed: {e}");
                String::new()
            }),
        None => String::new(),
    };

    // 4. Retrieve relevant chunks from the vector store.
    //
    // top_k = 15: chosen so a canonical enumeration chunk that ranks at
    // position 11-14 (e.g. a long list-of-regions section that spans pages
    // in a rulebook) still lands in context. Above ~20 the context gets
    // noisy and slows answer quality more than it helps recall.
    let results = state
        .vector_store
        .search(&query_vector, &collection_ids, 15)
        .await
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;

    // Flag the answer as GM-secret-derived if any retrieved chunk is GM-only.
    let drew_from_gm_only = results.iter().any(|r| r.is_gm_only);

    // Diagnostic: dump the retrieved chunks so quality issues are visible in
    // the dev console. Gate behind CHRONACLE_RAG_DEBUG=1 to keep prod quiet.
    if std::env::var("CHRONACLE_RAG_DEBUG").is_ok() {
        log_retrieval_debug(message, &embed_provider, &results);
    }

    // 5. Build context-augmented system prompt
    let context = build_context(&results);
    let system_prompt = build_system_prompt(&context, &entity_context);

    if std::env::var("CHRONACLE_RAG_DEBUG").is_ok() {
        let llm_type = state
            .llm_provider
            .read()
            .map(|p| p.provider_type().to_string())
            .unwrap_or_else(|_| "unknown".into());
        eprintln!("===RAG_DEBUG_PROMPT===");
        eprintln!("llm_provider: {llm_type}");
        eprintln!("system_prompt ({} chars):", system_prompt.chars().count());
        eprintln!("{system_prompt}");
        eprintln!("===RAG_DEBUG_END===");
    }

    // 6. Call the LLM with the augmented prompt
    let chat_messages = vec![ChatMessage {
        role: "user".to_string(),
        content: message.to_string(),
    }];

    // Clone the current provider out of the RwLock so the streaming task
    // doesn't hold the lock for the entire response.
    let llm = state
        .llm_provider
        .read()
        .map_err(|e| AgentError::Llm(format!("Lock error: {e}")))?
        .clone();

    let rx = llm
        .chat_stream(&system_prompt, &chat_messages)
        .await
        .map_err(|e| AgentError::Llm(e.to_string()))?;

    Ok(StreamHandle {
        rx,
        drew_from_gm_only,
    })
}

/// Dump retrieval diagnostics to stderr (gated by CHRONACLE_RAG_DEBUG=1).
///
/// Prints, between begin/end markers, the user query, embedding model, and
/// every retrieved chunk's score + source + page range + first ~300 chars of
/// text. Use this to verify whether the right chunk is being retrieved at all
/// before debugging the LLM's interpretation of it.
fn log_retrieval_debug(
    query: &str,
    embed_provider: &Arc<dyn crate::providers::embedding::EmbeddingProvider>,
    results: &[crate::providers::vector_store::SearchResult],
) {
    eprintln!("===RAG_DEBUG_BEGIN===");
    eprintln!("query: {query:?}");
    eprintln!(
        "embed_model: {} (dim={})",
        embed_provider.model_name(),
        embed_provider.dimension()
    );
    eprintln!("retrieved {} chunk(s):", results.len());
    for (i, r) in results.iter().enumerate() {
        let excerpt: String = r.text.chars().take(300).collect();
        eprintln!(
            "  [{i}] dist={:.4} source={:?} p.{}-{} heading={:?}\n      text: {:?}",
            r.distance, r.source_name, r.page_start, r.page_end, r.section_heading, excerpt
        );
    }
}

/// Build a context block from search results for the LLM prompt.
fn build_context(results: &[crate::providers::vector_store::SearchResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let mut ctx = String::from("Relevant source material:\n\n");
    for (i, r) in results.iter().enumerate() {
        let source = if r.source_name.is_empty() {
            &r.source_id
        } else {
            &r.source_name
        };
        ctx.push_str(&format!(
            "[{i}] Source: \"{source}\", p. {}-{} — \"{}\"\n{}\n\n",
            r.page_start, r.page_end, r.section_heading, r.text
        ));
    }
    ctx
}

/// Build the system prompt for the GM assistant.
///
/// Accepts separate RAG context (retrieved source passages) and entity context
/// (campaign notes). Either or both may be empty — the prompt adapts to include
/// only the relevant sections. This two-arg signature replaces the single-arg
/// `build_rag_system_prompt` to support hybrid retrieval (Task 3 wires in the
/// real entity context; for now callers pass `""` as the second argument).
fn build_system_prompt(rag_context: &str, entity_context: &str) -> String {
    let has_rag = !rag_context.is_empty();
    let has_entities = !entity_context.is_empty();

    if !has_rag && !has_entities {
        return "You are an expert Game Master assistant. \
            Answer the user's question to the best of your ability. \
            If you don't know the answer, say so — do not make up rules."
            .to_string();
    }

    let mut prompt = String::from("You are an expert Game Master assistant.\n\n");

    if has_rag {
        prompt.push_str(&format!("REFERENCE MATERIAL:\n{rag_context}\n"));
    }

    if has_entities {
        prompt.push_str(&format!(
            "CAMPAIGN NOTES (GM's own records):\n{entity_context}\n"
        ));
    }

    prompt.push_str("INSTRUCTIONS:\n");
    prompt.push_str("- Read every passage and note above carefully BEFORE answering.\n");

    if has_rag {
        prompt.push_str(
            "- Entity scope is critical. A passage is valid evidence ONLY when it \
             explicitly attributes a fact to the SAME entity the question is about. \
             A passage that lists the target entity alongside OTHER entities \
             (e.g. \"X dominates Vethara, Korim, Suthen and Marrowen\", or \
             \"in Vethara and in Korim\") does NOT attribute everything in the \
             list to the target — those are SEPARATE entities. Wording can vary \
             (synonyms and paraphrases are fine when they refer to the same entity, \
             e.g. \"factions\" ≈ \"groups\"), but a fact about a different but \
             co-mentioned entity is NOT evidence for the target.\n\
             - For list / enumeration questions (\"which are the...\", \"what are the...\", \
             \"list...\"), enumerate EVERY item the passages explicitly attribute to \
             the target entity. Do not compress to fit a sentence budget. If the \
             passages only cover some items, list those and acknowledge that the \
             reference material may be incomplete.\n",
        );
    }

    prompt.push_str(
        "- For other questions, answer in 1–3 sentences. Be concise — the GM is \
         running a table.\n",
    );
    if has_rag {
        prompt.push_str(
            "- Do NOT quote the passages verbatim in your answer text — the supporting \
             quote belongs INSIDE the citation marker.\n",
        );
    }

    if has_rag {
        prompt.push_str(
            "- Every factual claim from REFERENCE MATERIAL must cite its source using \
             this exact format, including a short verbatim quote (1 sentence) from the \
             passage that supports the claim:\n  \
               [Source: \"<source name>\", p.<page>, quote: \"<verbatim sentence>\"]\n  \
             Use the singular key `quote:` with exactly ONE sentence — never \
             `quotes:` or multiple excerpts. Emit a separate marker per source.\n  \
             Example: [Source: \"PHB\", p.72, quote: \"A fighter can use Action Surge once per rest.\"]\n  \
             The UI hides the quote from the visible reply and shows it in a popover \
             when the user clicks the citation badge.\n\
             - Only say \"the reference material does not contain this information\" if you \
             have scanned every passage and found no relevant content (paraphrase counts \
             only for the same entity).\n",
        );
    }

    if has_entities {
        prompt.push_str(
            "- Every factual claim from CAMPAIGN NOTES must cite the entity using \
             this exact format:\n  \
               [Entity: \"<entity name>\", kind: \"<kind>\"]\n  \
             where kind is the bracketed prefix in the campaign note line \
             (e.g. player_character, npc, location, faction, creature, item, event, misc).\n  \
             Example: [Entity: \"Nazirdijan\", kind: \"player_character\"]\n  \
             No verbatim quote is needed — entity records are the GM's own data.\n\
             - Entity names in CAMPAIGN NOTES are exact — use them verbatim in citations.\n",
        );
    }

    prompt
}

/// Insert a message record into the `message` table.
///
/// When `campaign_id` is `Some`, the message is bound to that campaign so
/// `get_chat_history` can filter per-campaign. `None` records a globally
/// scoped message (kept for the zero-campaign bootstrap window).
pub async fn persist_message<C>(
    db: &surrealdb::Surreal<C>,
    role: &str,
    content: &str,
    is_gm_only: bool,
    campaign_id: Option<&str>,
) -> Result<(), AgentError>
where
    C: Connection,
{
    let sql = match campaign_id {
        Some(_) => {
            "CREATE message SET
                role = $role,
                content = $content,
                citations = [],
                is_gm_only = $gm_only,
                campaign = type::thing('campaign', $cid),
                created_at = time::now()"
        }
        None => {
            "CREATE message SET
                role = $role,
                content = $content,
                citations = [],
                is_gm_only = $gm_only,
                created_at = time::now()"
        }
    };

    let mut q = db
        .query(sql)
        .bind(("role", role.to_owned()))
        .bind(("content", content.to_owned()))
        .bind(("gm_only", is_gm_only));
    if let Some(cid) = campaign_id {
        q = q.bind(("cid", cid.to_owned()));
    }
    q.await.map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(())
}

/// Persist an assistant message with parsed citations.
pub async fn persist_assistant_message<C>(
    db: &surrealdb::Surreal<C>,
    content: &str,
    is_gm_only: bool,
    campaign_id: Option<&str>,
) -> Result<(), AgentError>
where
    C: Connection,
{
    let citations = parse_citations(content);

    if citations.is_empty() {
        return persist_message(db, "assistant", content, is_gm_only, campaign_id).await;
    }

    // Build citations as SurrealQL inline objects (bind params lose field names
    // with serde_json::Value for array<object> types)
    let cit_parts: Vec<String> = citations
        .iter()
        .map(|c| {
            let name = c.source_name.replace('\'', "''");
            let excerpt = c.text_excerpt.replace('\'', "''");
            match c.page {
                Some(p) => {
                    format!("{{ source_name: '{name}', page: {p}, text_excerpt: '{excerpt}' }}")
                }
                None => format!("{{ source_name: '{name}', text_excerpt: '{excerpt}' }}"),
            }
        })
        .collect();
    let cit_surql = cit_parts.join(", ");

    let campaign_assign = if campaign_id.is_some() {
        ", campaign = type::thing('campaign', $cid)"
    } else {
        ""
    };

    let sql = format!(
        "CREATE message SET \
         role = 'assistant', \
         content = $content, \
         citations = [{cit_surql}], \
         is_gm_only = $gm_only\
         {campaign_assign}, \
         created_at = time::now()"
    );

    let mut q = db
        .query(sql)
        .bind(("content", content.to_owned()))
        .bind(("gm_only", is_gm_only));
    if let Some(cid) = campaign_id {
        q = q.bind(("cid", cid.to_owned()));
    }
    q.await.map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(())
}

/// A single citation extracted from an assistant response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Citation {
    pub source_name: String,
    pub page: Option<i64>,
    pub text_excerpt: String,
}

/// Parse citations from an assistant response.
///
/// Accepts:
///   `[Source: "Name", p.12]`                        – page only
///   `[Source: "Name", p.45-49]`                     – page range (start captured)
///   `[Source: "Name", p.9, quote: "verbatim text"]` – with inline supporting quote
///   `[Source: "Name"]`                              – source only
///
/// When a quote is present, it's stored as `text_excerpt`. When absent, the
/// 80 characters following the citation marker are used as a degraded fallback.
fn parse_citations(response: &str) -> Vec<Citation> {
    // Tolerant of model format drift: singular `quote:` or plural `quotes:`, and
    // any trailing content (a second excerpt, stray prose) up to the closing `]`
    // is consumed so the marker still parses. First quoted excerpt is captured.
    let re = regex::Regex::new(
        r#"(?s)\[Source:\s*"([^"]+)"(?:,\s*p\.\s*(\d+)(?:-\d+)?)?(?:,\s*quotes?:\s*"(.*?)")?[^\]]*\]"#,
    )
    .expect("valid citation regex");

    re.captures_iter(response)
        .map(|cap| {
            let source_name = cap[1].to_string();
            let page = cap.get(2).and_then(|m| m.as_str().parse::<i64>().ok());
            let text_excerpt = if let Some(q) = cap.get(3) {
                q.as_str().trim().to_string()
            } else {
                let marker_end = cap.get(0).map_or(0, |m| m.end());
                response
                    .chars()
                    .skip(marker_end)
                    .take(80)
                    .collect::<String>()
                    .trim()
                    .to_string()
            };

            Citation {
                source_name,
                page,
                text_excerpt,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Context building tests ───────────────────────────────────

    #[test]
    fn test_build_context_empty() {
        let ctx = build_context(&[]);
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_build_context_with_results() {
        use crate::providers::vector_store::SearchResult;

        let results = vec![SearchResult {
            chunk_id: "chunk-1".into(),
            source_id: "source:abc".into(),
            source_name: "PHB.pdf".into(),
            text: "A fighter can use Action Surge once per rest.".into(),
            page_start: 72,
            page_end: 72,
            section_heading: "Fighter Class Features".into(),
            source_type: "rules".into(),
            is_gm_only: false,
            distance: 0.15,
        }];

        let ctx = build_context(&results);
        assert!(!ctx.is_empty());
        assert!(ctx.contains("PHB.pdf"));
        assert!(ctx.contains("p. 72-72"));
        assert!(ctx.contains("Action Surge"));
    }

    // ── System prompt tests ─────────────────────────────────────

    #[test]
    fn test_system_prompt_without_context() {
        let prompt = build_system_prompt("", "");
        assert!(prompt.contains("Game Master assistant"));
        assert!(!prompt.contains("REFERENCE MATERIAL"));
    }

    #[test]
    fn test_system_prompt_with_context() {
        let ctx =
            "[0] Source: \"PHB.pdf\", p. 72 — \"Fighter Class Features\"\nAction Surge text.\n\n";
        let prompt = build_system_prompt(ctx, "");
        assert!(prompt.contains("REFERENCE MATERIAL"));
        assert!(prompt.contains("PHB.pdf"));
        assert!(prompt.contains("[Source: \"<source name>\""));
        assert!(prompt.contains("Do NOT quote the passages"));
        assert!(prompt.contains("1–3 sentences"));
        assert!(prompt.contains("synonyms"));
        assert!(prompt.contains("scanned every passage"));
        assert!(prompt.contains("quote: \""));
    }

    /// Regression for a cross-entity-contamination bug observed in production:
    /// the LLM listed sibling regions as part of the target continent because
    /// the prompt told it to treat "paraphrases and partial matches" as
    /// evidence, with no rule about preserving entity scope.
    ///
    /// The new prompt must (a) explicitly call out the "X dominates A, B, C and
    /// D" trap, (b) require enumeration questions to list every attributed item.
    #[test]
    fn test_system_prompt_guards_entity_scope_and_enumeration() {
        let prompt = build_system_prompt("[0] Source: \"x.pdf\", p. 1 — \"\"\ntext\n\n", "");
        // Entity-scope rule must be present.
        assert!(
            prompt.contains("Entity scope is critical"),
            "prompt should warn about cross-entity contamination"
        );
        // The specific failure shape we observed in production.
        assert!(
            prompt.contains("SEPARATE entities"),
            "prompt should explicitly say co-listed entities are SEPARATE"
        );
        // Enumeration questions must not be compressed to 1–3 sentences.
        assert!(
            prompt.contains("enumeration questions") || prompt.contains("list / enumeration"),
            "prompt should call out list / enumeration questions"
        );
        assert!(
            prompt.contains("Do not compress"),
            "prompt should forbid compressing lists into the 1-3 sentence budget"
        );
    }

    // ── Citation parsing tests ───────────────────────────────────

    #[test]
    fn test_parse_citations_empty() {
        let citations = parse_citations("Hello, I don't know the answer.");
        assert!(citations.is_empty());
    }

    #[test]
    fn test_parse_citations_single() {
        let text = "The fighter can use Action Surge [Source: \"PHB\", p.72].";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "PHB");
        assert_eq!(citations[0].page, Some(72));
    }

    #[test]
    fn test_parse_citations_multiple() {
        let text = "Combat has multiple actions [Source: \"PHB\", p.192]. \
                     Opportunity attacks are different [Source: \"DMG\", p.25].";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].source_name, "PHB");
        assert_eq!(citations[1].source_name, "DMG");
    }

    #[test]
    fn test_parse_citations_no_page() {
        let text = "See the basic rules [Source: \"SRD\"].";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "SRD");
        assert_eq!(citations[0].page, None);
    }

    #[test]
    fn test_parse_citations_with_inline_quote() {
        let text = "Lantern orbits Mirovia. [Source: \"Quickstart.pdf\", p.9, quote: \"The space station Lantern orbits the silver clouds of the planet Mirovia.\"]";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "Quickstart.pdf");
        assert_eq!(citations[0].page, Some(9));
        assert_eq!(
            citations[0].text_excerpt,
            "The space station Lantern orbits the silver clouds of the planet Mirovia."
        );
    }

    #[test]
    fn test_parse_citations_inline_quote_with_page_range() {
        let text = "[Source: \"PHB\", p.45-49, quote: \"Combat proceeds in rounds.\"]";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].page, Some(45));
        assert_eq!(citations[0].text_excerpt, "Combat proceeds in rounds.");
    }

    // Field regression: the model drifted to plural `quotes:` with two excerpts
    // joined by "and". The strict `quote:` + `]` anchor failed to match, so the
    // citation was dropped (and the raw marker leaked into the rendered reply).
    #[test]
    fn test_parse_citations_plural_quotes_with_multiple_excerpts() {
        let text = "[Source: \"Coriolis EN.pdf\", p.214-215, quotes: \"Secure dangerous artifacts for... the Draconites\" and \"Prevent the spread of dangerous bionics for... the Draconites\"]";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "Coriolis EN.pdf");
        assert_eq!(citations[0].page, Some(214));
        // The first excerpt is captured as the supporting quote.
        assert_eq!(
            citations[0].text_excerpt,
            "Secure dangerous artifacts for... the Draconites"
        );
    }

    #[test]
    fn test_parse_citations_page_range() {
        let cases = [
            (
                "[Source: \"Quickstart.pdf\", p.9-9]",
                "Quickstart.pdf",
                Some(9),
            ),
            (
                "[Source: \"Quickstart.pdf\", p.45-49]",
                "Quickstart.pdf",
                Some(45),
            ),
            ("[Source: \"PHB\", p. 72-72]", "PHB", Some(72)),
        ];
        for (input, expected_name, expected_page) in cases {
            let citations = parse_citations(input);
            assert_eq!(citations.len(), 1, "no match for {input:?}");
            assert_eq!(citations[0].source_name, expected_name);
            assert_eq!(citations[0].page, expected_page);
        }
    }

    // ── Message persistence tests ───────────────────────────────

    #[tokio::test]
    async fn test_persist_and_retrieve_messages() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        db.query(
            "DEFINE TABLE message SCHEMAFULL;
             DEFINE FIELD role ON message TYPE string;
             DEFINE FIELD content ON message TYPE string;
             DEFINE FIELD citations ON message TYPE array<object>;
             DEFINE FIELD created_at ON message TYPE datetime;",
        )
        .await
        .unwrap();

        persist_message(&db, "user", "question", false, None)
            .await
            .unwrap();
        persist_assistant_message(&db, "response with [Source: \"PHB\", p.72].", false, None)
            .await
            .unwrap();

        let mut response = db
            .query("SELECT count() FROM message GROUP ALL")
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        struct Count {
            count: i64,
        }
        let counts: Vec<Count> = response.take(0).unwrap();
        assert_eq!(counts[0].count, 2);

        let citations = parse_citations("response with [Source: \"PHB\", p.72].");
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "PHB");
        assert_eq!(citations[0].page, Some(72));

        let mut response = db
            .query("SELECT role, content FROM message WHERE role = $role")
            .bind(("role", "assistant"))
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        #[expect(dead_code)]
        struct Msg {
            role: String,
            content: String,
        }
        let msgs: Vec<Msg> = response.take(0).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].content.contains("PHB"));
    }

    #[tokio::test]
    async fn test_chat_history_empty_when_no_messages() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        db.query(
            "DEFINE TABLE message SCHEMAFULL;
             DEFINE FIELD role ON message TYPE string;
             DEFINE FIELD content ON message TYPE string;
             DEFINE FIELD citations ON message TYPE array<object>;
             DEFINE FIELD created_at ON message TYPE datetime;",
        )
        .await
        .unwrap();

        let mut response = db
            .query("SELECT role, content FROM message LIMIT 10")
            .await
            .unwrap();

        #[derive(serde::Deserialize)]
        #[expect(dead_code)]
        struct Row {
            role: String,
            content: String,
        }

        let rows: Vec<Row> = response.take(0).unwrap();
        assert!(rows.is_empty());
    }

    // ── Collection resolution tests ──────────────────────────────

    #[tokio::test]
    async fn resolve_collection_ids_returns_subscribed_ids() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE collection SET id='col1', name='C1', created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col2', name='C2', created_at=time::now(), updated_at=time::now(); \
             CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()"
        ).await.unwrap();
        db.query(
            "LET $in = type::thing('campaign','camp1');
             LET $out1 = type::thing('collection','col1');
             LET $out2 = type::thing('collection','col2');
             RELATE $in->subscribes_to->$out1 SET created_at=time::now();
             RELATE $in->subscribes_to->$out2 SET created_at=time::now()",
        )
        .await
        .unwrap();

        let ids = resolve_collection_ids(&db, "camp1").await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"col1".to_string()));
        assert!(ids.contains(&"col2".to_string()));
    }

    /// Regression for bug #3: messages must be bound to the active campaign so
    /// `get_chat_history` can filter per-campaign instead of returning every row
    /// when the user switches campaigns.
    #[tokio::test]
    async fn persist_message_binds_campaign_record_link() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        persist_message(&db, "user", "scoped to camp1", false, Some("camp1"))
            .await
            .unwrap();
        persist_assistant_message(&db, "reply [Source: \"PHB\", p.72].", false, Some("camp1"))
            .await
            .unwrap();
        // A separate "global" message that must not leak into the camp1 filter.
        persist_message(&db, "user", "unscoped", false, None)
            .await
            .unwrap();

        #[derive(serde::Deserialize)]
        struct Row {
            content: String,
        }
        let mut resp = db
            .query(
                "SELECT content, created_at FROM message \
                 WHERE campaign = type::thing('campaign', 'camp1') \
                 ORDER BY created_at ASC",
            )
            .await
            .unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        assert_eq!(rows.len(), 2, "exactly the two camp1-scoped messages");
        assert_eq!(rows[0].content, "scoped to camp1");
        assert!(rows[1].content.starts_with("reply"));
    }

    /// Regression for bug #5: history was lost on Oracle re-mount because
    /// `get_chat_history` filters using a literal `campaign:`<id>`` record link
    /// while persistence wrote `type::thing('campaign', $cid)`. The two MUST
    /// produce the same record id so the filter matches, especially for
    /// real-world hex-only campaign IDs (UUIDs with hyphens stripped).
    #[tokio::test]
    async fn chat_history_literal_filter_matches_persisted_messages() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        // Use the same id shape `campaign_service::create` produces — a UUID
        // with hyphens stripped, so the WHERE literal does not need backticks
        // around special characters.
        let cid = "d5a80195396844cb8b46270830df952f";
        db.query(format!(
            "CREATE campaign SET id='{cid}', name='T', system='5e', \
             created_at=time::now(), updated_at=time::now()"
        ))
        .await
        .unwrap();

        persist_message(&db, "user", "first", false, Some(cid))
            .await
            .unwrap();
        persist_assistant_message(&db, "reply", false, Some(cid))
            .await
            .unwrap();

        // Mirror the exact SQL `commands::get_chat_history` issues.
        let safe_id = cid.replace('`', "``");
        let sql = format!(
            "SELECT role, content, created_at FROM message \
             WHERE campaign = campaign:`{safe_id}` ORDER BY created_at ASC"
        );
        #[derive(serde::Deserialize)]
        struct Row {
            role: String,
            content: String,
        }
        let mut resp = db.query(sql).await.unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        assert_eq!(
            rows.len(),
            2,
            "literal-record-link filter must match `type::thing`-written messages"
        );
        assert_eq!(rows[0].role, "user");
        assert_eq!(rows[0].content, "first");
        assert_eq!(rows[1].role, "assistant");
    }

    #[tokio::test]
    async fn fetch_entity_context_returns_empty_when_no_entities() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(result.is_empty(), "expected empty string, got: {result:?}");
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_player_character_fields() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE player_character SET id='pc1', \
             name='Nazirdijan', player_name='Nico', character_class='Wizard', \
             character_level=5, status='active', summary=NULL, notes=NULL, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('player_character','pc1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[player_character] Nazirdijan"),
            "missing entity line: {result}"
        );
        assert!(
            result.contains("Player: Nico"),
            "missing player_name: {result}"
        );
        assert!(result.contains("Class: Wizard"), "missing class: {result}");
        assert!(result.contains("Level: 5"), "missing level: {result}");
        assert!(
            result.contains("Status: active"),
            "missing status: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_omits_empty_sections() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE npc SET id='npc1', \
             name='Aldric the Smith', summary='village blacksmith', notes=NULL, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('npc','npc1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[npc] Aldric the Smith"),
            "missing npc: {result}"
        );
        assert!(
            result.contains("village blacksmith"),
            "missing summary: {result}"
        );
        assert!(
            !result.contains("[player_character]"),
            "unexpected PC section: {result}"
        );
        assert!(
            !result.contains("[location]"),
            "unexpected location section: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_event_dates() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE event SET id='ev1', \
             name='Battle of Irongate', date_start='Year 312', date_end='Year 313', \
             summary=NULL, notes=NULL, is_ongoing=false, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('event','ev1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[event] Battle of Irongate"),
            "missing event: {result}"
        );
        assert!(
            result.contains("Year 312 → Year 313"),
            "missing dates: {result}"
        );
    }

    #[test]
    fn notes_excerpt_collapses_and_truncates() {
        assert_eq!(notes_excerpt(None), None);
        assert_eq!(notes_excerpt(Some("   ")), None);
        assert_eq!(
            notes_excerpt(Some("line one\n\nline  two")),
            Some("line one line two".to_string())
        );
        let long = "x ".repeat(400); // 400 single-char words
        let out = notes_excerpt(Some(&long)).unwrap();
        assert!(out.ends_with('…'), "expected ellipsis: {out}");
        assert_eq!(out.chars().count(), NOTES_EXCERPT_LEN + 1);
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_entity_notes() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE npc SET id='npc1', name='Seraphina', summary='archivist', \
             notes='She secretly guards the Sunstone beneath the Iron Tower.', \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('npc','npc1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("Notes: She secretly guards the Sunstone"),
            "entity notes should appear in context: {result}"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_includes_session_notes() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE session SET id='sess1', campaign=type::thing('campaign','camp1'), \
             session_number=4, title='Shadows of the Keep', date_played='2026-06-05', \
             notes='The party freed the prisoners and burned the granary.', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[session 4] Shadows of the Keep"),
            "session line should appear in context: {result}"
        );
        assert!(
            result.contains("Notes: The party freed the prisoners"),
            "session notes should appear in context: {result}"
        );
    }

    #[tokio::test]
    async fn resolve_collection_ids_empty_for_no_subscriptions() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let ids = resolve_collection_ids(&db, "camp1").await.unwrap();
        assert!(ids.is_empty());
    }

    // ── build_system_prompt (two-arg) tests ─────────────────────

    #[test]
    fn build_system_prompt_both_contexts_includes_both_sections() {
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
        let prompt = build_system_prompt(rag, ent);
        assert!(prompt.contains("REFERENCE MATERIAL"), "missing RAG section");
        assert!(prompt.contains("CAMPAIGN NOTES"), "missing entity section");
        assert!(
            prompt.contains("[Entity:"),
            "missing entity citation instruction"
        );
        assert!(
            prompt.contains("[Source:"),
            "missing source citation instruction"
        );
    }

    #[test]
    fn build_system_prompt_entity_only_omits_rag_section() {
        let ent = "Campaign notes (your GM records):\n\n[npc] Aldric\n";
        let prompt = build_system_prompt("", ent);
        assert!(prompt.contains("CAMPAIGN NOTES"), "missing entity section");
        assert!(
            !prompt.contains("REFERENCE MATERIAL"),
            "unexpected RAG section"
        );
        assert!(
            prompt.contains("[Entity:"),
            "missing entity citation instruction"
        );
        assert!(
            !prompt.contains("Entity scope is critical"),
            "unexpected RAG-only instruction"
        );
    }

    #[test]
    fn build_system_prompt_rag_only_regression() {
        // Regression: existing behaviour must be preserved when entity_context is empty.
        let rag = "[0] Source: \"PHB\", p.1 — \"Intro\"\nSome rules.\n\n";
        let prompt = build_system_prompt(rag, "");
        assert!(prompt.contains("REFERENCE MATERIAL"), "missing RAG section");
        assert!(
            !prompt.contains("CAMPAIGN NOTES"),
            "unexpected entity section"
        );
        assert!(
            prompt.contains("Entity scope is critical"),
            "missing scope guard"
        );
        assert!(
            prompt.contains("SEPARATE entities"),
            "missing entity contamination guard"
        );
        assert!(
            prompt.contains("list / enumeration"),
            "missing enumeration instruction"
        );
        assert!(
            prompt.contains("Do not compress"),
            "missing list-compression guard"
        );
    }

    #[test]
    fn build_system_prompt_neither_returns_fallback() {
        let prompt = build_system_prompt("", "");
        assert!(
            !prompt.contains("REFERENCE MATERIAL"),
            "unexpected RAG section"
        );
        assert!(
            !prompt.contains("CAMPAIGN NOTES"),
            "unexpected entity section"
        );
        assert!(
            prompt.contains("Game Master assistant"),
            "missing base identity"
        );
    }

    #[tokio::test]
    async fn fetch_entity_context_event_empty_date_end_no_arrow() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE event SET id='ev1', \
             name='Siege of Dawnwall', date_start='Year 400', date_end='', \
             summary=NULL, notes=NULL, is_ongoing=false, \
             created_at=time::now(), updated_at=time::now(); \
             LET $src = type::thing('campaign','camp1'); \
             LET $dst = type::thing('event','ev1'); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .await
        .unwrap();

        let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
        assert!(
            result.contains("[event] Siege of Dawnwall"),
            "missing event: {result}"
        );
        assert!(result.contains("Year 400"), "missing date_start: {result}");
        assert!(
            !result.contains("→"),
            "unexpected arrow when date_end is empty: {result}"
        );
    }
}
