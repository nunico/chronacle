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
pub async fn process_message(
    state: &Arc<AppState>,
    message: &str,
    _campaign_id: Option<&str>,
) -> Result<String, AgentError> {
    let query_vector = embed_query(message).await?;
    let _results = state
        .vector_store
        .search(&query_vector, None, 10)
        .await
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;
    let _context = build_context(&_results);
    let response = format!(
        "[Agent response not yet implemented — Phase 1 stub. \
         Your question was: \"{message}\"]"
    );
    persist_message(&state.db, "user", message).await?;
    persist_message(&state.db, "assistant", &response).await?;
    Ok(response)
}

async fn embed_query(_text: &str) -> Result<Vec<f32>, AgentError> {
    Ok(vec![0.0; 768])
}

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
            campaign = NONE,
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

    #[tokio::test]
    async fn test_persist_and_retrieve_messages() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        // Create relaxed message table for testing
        db.query(
            "DEFINE TABLE message SCHEMAFULL;
             DEFINE FIELD role ON message TYPE string;
             DEFINE FIELD content ON message TYPE string;
             DEFINE FIELD citations ON message TYPE array<object>;
             DEFINE FIELD created_at ON message TYPE datetime;",
        )
        .await
        .unwrap();

        persist_message(&db, "user", "question").await.unwrap();
        persist_message(&db, "assistant", "response").await.unwrap();

        let mut response = db
            .query("SELECT role, content, created_at FROM message ORDER BY created_at ASC")
            .await
            .unwrap();

        #[derive(serde::Deserialize)]
        struct Row {
            role: String,
            content: String,
        }

        let rows: Vec<Row> = response.take(0).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(&rows[0].role, "user");
        assert_eq!(&rows[0].content, "question");
        assert_eq!(&rows[1].role, "assistant");
        assert_eq!(&rows[1].content, "response");
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
            .query("SELECT role, content, created_at FROM message ORDER BY created_at ASC")
            .await
            .unwrap();

        #[derive(serde::Deserialize)]
        struct Row {
            role: String,
            content: String,
        }

        let rows: Vec<Row> = response.take(0).unwrap();
        assert!(rows.is_empty());
    }
}
