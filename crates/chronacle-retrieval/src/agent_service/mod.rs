//! Agent service — orchestrates the RAG pipeline.
//!
//! 1. Persists the user message.
//! 2. Embeds the query via the active embedding provider.
//! 3. Retrieves relevant chunks from the vector store + campaign/collection notes.
//! 4. Builds a context-augmented prompt with citation instructions.
//! 5. Streams the LLM response through an mpsc channel.
//!
//! The caller receives token chunks. After the channel is exhausted the caller
//! should persist the assistant message with parsed citations.
//!
//! Submodules split the pipeline by concern:
//! - [`context`] — collection resolution, entity/note gathering, RAG context block
//! - [`prompt`] — system-prompt assembly
//! - [`citation`] — parsing `[Source: ...]` markers out of responses
//! - [`persistence`] — writing chat messages (with citations) to the DB

mod citation;
mod context;
mod persistence;
mod prompt;
mod rules_block;

pub use citation::Citation;
pub use context::{fetch_entity_context, resolve_collection_ids};
pub use persistence::{persist_assistant_message, persist_message};

use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmError, LlmProvider};
use chronacle_core::vector_store::VectorStore;

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

/// Run the full streaming RAG pipeline.
///
/// Returns a channel of streaming tokens. Once the channel is exhausted,
/// call `persist_assistant_message` with the accumulated response.
pub async fn stream_response<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    embedding_provider: &RwLock<Arc<dyn EmbeddingProvider>>,
    vector_store: &Arc<dyn VectorStore>,
    llm_provider: &RwLock<Arc<dyn LlmProvider>>,
    message: &str,
    campaign_id: Option<&str>,
) -> Result<mpsc::Receiver<Result<String, LlmError>>, AgentError> {
    // 1. Persist the user message
    persist_message(db, "user", message, campaign_id).await?;

    // 2. Embed the query
    let embed_provider = embedding_provider
        .read()
        .map_err(|e| AgentError::Llm(format!("Embedding lock: {e}")))?
        .clone();
    let query_vector = embed_provider
        .embed_query(message)
        .await
        .map_err(|e| AgentError::Embedding(e.to_string()))?;

    // 3. Resolve collection IDs for the active campaign
    let collection_ids = match campaign_id {
        Some(cid) => resolve_collection_ids(db, cid)
            .await
            .map_err(|e| AgentError::Retrieval(e.to_string()))?,
        None => Vec::new(),
    };

    let entity_context = match campaign_id {
        Some(cid) => fetch_entity_context(db, cid, &collection_ids, Some(&query_vector))
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
    let results = vector_store
        .search(&query_vector, &collection_ids, 15)
        .await
        .map_err(|e| AgentError::Retrieval(e.to_string()))?;

    // Diagnostic: dump the retrieved chunks so quality issues are visible in
    // the dev console. Gate behind CHRONACLE_RAG_DEBUG=1 to keep prod quiet.
    if std::env::var("CHRONACLE_RAG_DEBUG").is_ok() {
        log_retrieval_debug(message, &embed_provider, &results);
    }

    // 5. Build context-augmented system prompt
    let context = context::build_context(&results);
    let system_prompt = prompt::build_system_prompt(&context, &entity_context);

    if std::env::var("CHRONACLE_RAG_DEBUG").is_ok() {
        let llm_type = llm_provider
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
    let llm = llm_provider
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
    embed_provider: &Arc<dyn chronacle_core::embedding::EmbeddingProvider>,
    results: &[chronacle_core::vector_store::SearchResult],
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
