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

/// Run the full streaming RAG pipeline.
///
/// Returns a channel of streaming tokens. Once the channel is exhausted,
/// call `persist_assistant_message` with the accumulated response.
pub async fn stream_response(
    state: &Arc<AppState>,
    message: &str,
    campaign_id: Option<&str>,
) -> Result<mpsc::Receiver<Result<String, LlmError>>, AgentError> {
    // 1. Persist the user message
    persist_message(&state.db, "user", message, campaign_id).await?;

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

    // 4. Retrieve relevant chunks from the vector store
    let results = state
        .vector_store
        .search(&query_vector, &collection_ids, 10)
        .await
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;

    // Diagnostic: dump the retrieved chunks so quality issues are visible in
    // the dev console. Gate behind CHRONACLE_RAG_DEBUG=1 to keep prod quiet.
    if std::env::var("CHRONACLE_RAG_DEBUG").is_ok() {
        log_retrieval_debug(message, &embed_provider, &results);
    }

    // 5. Build context-augmented system prompt
    let context = build_context(&results);
    let system_prompt = build_rag_system_prompt(&context);

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

    llm.chat_stream(&system_prompt, &chat_messages)
        .await
        .map_err(|e| AgentError::Llm(e.to_string()))
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

/// Build the system prompt for the GM assistant, with or without RAG context.
fn build_rag_system_prompt(context: &str) -> String {
    if context.is_empty() {
        return "You are an expert Game Master assistant. \
            Answer the user's question to the best of your ability. \
            If you don't know the answer, say so — do not make up rules."
            .to_string();
    }

    format!(
        "You are an expert Game Master assistant.\n\n\
         REFERENCE MATERIAL:\n{context}\n\
         INSTRUCTIONS:\n\
         - Read every passage above carefully BEFORE deciding whether the answer is present.\n\
         - The reference passages may use different wording than the user's question \
           (e.g. the question says \"factions\", the passage says \"groups\" or \"organizations\"). \
           Treat synonyms, paraphrases, and partial matches as valid evidence.\n\
         - Answer the question directly in 1–3 sentences. Do NOT quote the passages \
           verbatim in your answer — the supporting quote belongs inside the citation.\n\
         - Every factual claim must cite its source using this exact format, including \
           a short verbatim quote (1 sentence) from the passage that supports the claim:\n  \
             [Source: \"<source name>\", p.<page>, quote: \"<verbatim sentence>\"]\n  \
           Example: [Source: \"PHB\", p.72, quote: \"A fighter can use Action Surge once per rest.\"]\n  \
           The UI hides the quote from the visible reply and shows it in a popover \
           when the user clicks the citation badge.\n\
         - Only say \"the reference material does not contain this information\" if you \
           have scanned every passage and found no relevant content, even by paraphrase.\n\
         - Be concise. The GM is running a table."
    )
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
                campaign = type::thing('campaign', $cid),
                created_at = time::now()"
        }
        None => {
            "CREATE message SET
                role = $role,
                content = $content,
                citations = [],
                created_at = time::now()"
        }
    };

    let mut q = db
        .query(sql)
        .bind(("role", role.to_owned()))
        .bind(("content", content.to_owned()));
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
    campaign_id: Option<&str>,
) -> Result<(), AgentError>
where
    C: Connection,
{
    let citations = parse_citations(content);

    if citations.is_empty() {
        return persist_message(db, "assistant", content, campaign_id).await;
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
         citations = [{cit_surql}]\
         {campaign_assign}, \
         created_at = time::now()"
    );

    let mut q = db.query(sql).bind(("content", content.to_owned()));
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
    let re = regex::Regex::new(
        r#"(?s)\[Source:\s*"([^"]+)"(?:,\s*p\.\s*(\d+)(?:-\d+)?)?(?:,\s*quote:\s*"(.*?)")?\s*\]"#,
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
        let prompt = build_rag_system_prompt("");
        assert!(prompt.contains("Game Master assistant"));
        assert!(!prompt.contains("REFERENCE MATERIAL"));
    }

    #[test]
    fn test_system_prompt_with_context() {
        let ctx =
            "[0] Source: \"PHB.pdf\", p. 72 — \"Fighter Class Features\"\nAction Surge text.\n\n";
        let prompt = build_rag_system_prompt(ctx);
        assert!(prompt.contains("REFERENCE MATERIAL"));
        assert!(prompt.contains("PHB.pdf"));
        assert!(prompt.contains("[Source: \"<source name>\""));
        assert!(prompt.contains("Do NOT quote the passages"));
        assert!(prompt.contains("1–3 sentences"));
        assert!(prompt.contains("synonyms"));
        assert!(prompt.contains("scanned every passage"));
        assert!(prompt.contains("quote: \""));
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
        let text = "Coriolis orbits Kua. [Source: \"Quickstart.pdf\", p.9, quote: \"The space station Coriolis orbits the green jungles of the planet Kua.\"]";
        let citations = parse_citations(text);
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].source_name, "Quickstart.pdf");
        assert_eq!(citations[0].page, Some(9));
        assert_eq!(
            citations[0].text_excerpt,
            "The space station Coriolis orbits the green jungles of the planet Kua."
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

        persist_message(&db, "user", "question", None).await.unwrap();
        persist_assistant_message(&db, "response with [Source: \"PHB\", p.72].", None)
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

        persist_message(&db, "user", "scoped to camp1", Some("camp1"))
            .await
            .unwrap();
        persist_assistant_message(&db, "reply [Source: \"PHB\", p.72].", Some("camp1"))
            .await
            .unwrap();
        // A separate "global" message that must not leak into the camp1 filter.
        persist_message(&db, "user", "unscoped", None).await.unwrap();

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

        persist_message(&db, "user", "first", Some(cid)).await.unwrap();
        persist_assistant_message(&db, "reply", Some(cid)).await.unwrap();

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
}
