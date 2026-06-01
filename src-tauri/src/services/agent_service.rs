/// Agent service — orchestrates the RAG pipeline.
///
/// 1. Receives a user question.
/// 2. Embeds the question (via `fastembed` — stubbed in Phase 1).
/// 3. Retrieves relevant chunks from the vector store.
/// 4. Builds a context-augmented prompt.
/// 5. Streams the LLM response.
/// 6. Persists the conversation to the `message` table.
use std::sync::Arc;

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

/// Run the full RAG pipeline for a user message.
///
/// Returns a string with the LLM's response (including citations). In Phase 2
/// this will return a full `ChatResponse` with structured citation data.
///
/// `campaign_id` scopes retrieval to the collections the campaign subscribes to.
/// Pass `None` to skip retrieval entirely (e.g. during Phase 1 stub or when no
/// campaign is active).
pub async fn process_message(
    state: &Arc<AppState>,
    message: &str,
    campaign_id: Option<&str>,
) -> Result<String, AgentError> {
    // ── 1. Embed the query ─────────────────────────────────────
    let query_vector = embed_query(message).await?;

    // ── 2. Resolve collection IDs for the active campaign ──────
    let collection_ids = match campaign_id {
        Some(cid) => resolve_collection_ids(&state.db, cid).await?,
        None => Vec::new(),
    };

    // ── 3. Retrieve relevant chunks ────────────────────────────
    let _results = state
        .vector_store
        .search(&query_vector, &collection_ids, 10)
        .await
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;

    // ── 4. Build context ───────────────────────────────────────
    let _context = build_context(&_results);

    // ── 5. Call LLM ────────────────────────────────────────────
    // TODO: Phase 2 — build system prompt with context, call LLM,
    //       stream response back to caller.
    let response = format!(
        "[Agent response not yet implemented — Phase 1 stub. \
         Your question was: \"{message}\"]"
    );

    // ── 6. Persist message ─────────────────────────────────────
    persist_message(&state.db, "user", message, campaign_id).await?;
    persist_message(&state.db, "assistant", &response, campaign_id).await?;

    Ok(response)
}

/// Embed a query string using `fastembed` (nomic-embed-text-v1.5).
async fn embed_query(_text: &str) -> Result<Vec<f32>, AgentError> {
    // TODO: Phase 2 — implement with fastembed
    Ok(vec![0.0; 768])
}

/// Build a context-augmented prompt from search results.
fn build_context(results: &[crate::providers::vector_store::SearchResult]) -> String {
    if results.is_empty() {
        return String::new();
    }

    let mut ctx = String::from("Relevant source material:\n\n");
    for (i, r) in results.iter().enumerate() {
        ctx.push_str(&format!(
            "[{i}] — \"{}\" (p. {}-{})\n{}\n\n",
            r.section_heading, r.page_start, r.page_end, r.text
        ));
    }
    ctx
}

/// Insert a message record into the `message` table.
///
/// `campaign_id` is `Some` when a campaign is active; `None` produces
/// `campaign = NULL` which is valid because the schema declares
/// `DEFAULT NULL` on that field.
async fn persist_message<C>(
    db: &surrealdb::Surreal<C>,
    role: &str,
    content: &str,
    campaign_id: Option<&str>,
) -> Result<(), AgentError>
where
    C: surrealdb::Connection,
{
    let campaign: Option<surrealdb::sql::Thing> = campaign_id
        .map(|cid| surrealdb::sql::Thing::from(("campaign", cid)));

    db.query(
        "CREATE message SET
            campaign   = $campaign,
            role       = $role,
            content    = $content,
            citations  = [],
            created_at = time::now()",
    )
    .bind(("campaign", campaign))
    .bind(("role", role.to_owned()))
    .bind(("content", content.to_owned()))
    .await
    .map_err(|e| AgentError::Db(e.to_string()))?
    .check()
    .map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embed_query_returns_768_dims() {
        let vec = embed_query("test").await.unwrap();
        assert_eq!(vec.len(), 768);
    }

    #[tokio::test]
    async fn test_build_context_empty() {
        let ctx = build_context(&[]);
        assert!(ctx.is_empty());
    }

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
