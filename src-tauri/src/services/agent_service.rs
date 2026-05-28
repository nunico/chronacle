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

/// Run the full RAG pipeline for a user message.
///
/// Returns a string with the LLM's response (including citations). In Phase 2
/// this will return a full `ChatResponse` with structured citation data.
pub async fn process_message(
    state: &Arc<AppState>,
    message: &str,
    _campaign_id: Option<&str>,
) -> Result<String, AgentError> {
    // ── 1. Embed the query ─────────────────────────────────────
    let query_vector = embed_query(message).await?;

    // ── 2. Retrieve relevant chunks ────────────────────────────
    let _results = state
        .vector_store
        .search(&query_vector, None, 10)
        .await
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;

    // ── 3. Build context ───────────────────────────────────────
    let _context = build_context(&_results);

    // ── 4. Call LLM ────────────────────────────────────────────
    // TODO: Phase 2 — build system prompt with context, call LLM,
    //       stream response back to caller.
    let response = format!(
        "[Agent response not yet implemented — Phase 1 stub. \
         Your question was: \"{message}\"]"
    );

    // ── 5. Persist message ─────────────────────────────────────
    persist_message(&state.db, "user", message).await?;
    persist_message(&state.db, "assistant", &response).await?;

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
async fn persist_message<C>(
    db: &surrealdb::Surreal<C>,
    role: &str,
    content: &str,
) -> Result<(), AgentError>
where
    C: Connection,
{
    db.query(
        "CREATE message SET
            role = $role,
            content = $content,
            citations = [],
            created_at = time::now()",
    )
    .bind(("role", role.to_owned()))
    .bind(("content", content.to_owned()))
    .await
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
}
