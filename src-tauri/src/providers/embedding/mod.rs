/// Embedding provider — generates vector embeddings for text.
///
/// Phase 1 uses `fastembed` with `nomic-embed-text-v1.5` (768-dim).
///
/// ## Model download
///
/// The ONNX model is downloaded on first use from HuggingFace. Progress is
/// reported via a callback so the UI can display a progressive download bar.
/// Downloaded files are cached under the app data directory and reused on
/// subsequent starts. The cache follows hf-hub's directory layout so that
/// fastembed's native `try_new()` finds them without re-downloading.
use async_trait::async_trait;

mod consistency;
mod local;
mod mock;
mod openai;

pub use consistency::{check_embedding_model_consistency, EmbeddingModelMismatch, StaleModelCount};
pub use local::{local_embeddings_available, FastEmbedProvider};
pub use mock::MockEmbeddingProvider;
pub use openai::OpenAiEmbeddingProvider;

/// HuggingFace repo ID for the embedding model.
pub const MODEL_REPO: &str = "nomic-ai/nomic-embed-text-v1.5";

/// Files to download from the model repo.
pub const MODEL_FILES: &[(&str, &str)] = &[
    ("tokenizer.json", "tokenizer.json"),
    ("config.json", "config.json"),
    ("special_tokens_map.json", "special_tokens_map.json"),
    ("tokenizer_config.json", "tokenizer_config.json"),
    ("onnx/model.onnx", "onnx/model.onnx"),
];

/// Embedding output dimension for cloud providers.
///
/// Pinned to 768 to match the SurrealDB `MTREE DIMENSION 768` indexes, so cloud
/// vectors drop into the existing schema with no migration. OpenAI v3 models
/// honour the `dimensions` request parameter (Matryoshka), producing native
/// 768-dim output rather than a naive truncation.
pub const CLOUD_EMBED_DIM: usize = 768;

/// Default OpenAI embedding model.
pub const OPENAI_DEFAULT_EMBED_MODEL: &str = "text-embedding-3-small";

// ── Errors ───────────────────────────────────────────────────────────

/// Errors from the embedding provider.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Model initialization failed: {0}")]
    Init(String),
    #[error("Embedding generation failed: {0}")]
    Embed(String),
    #[error("Model not available — download may be in progress")]
    NotAvailable,
    #[error("Download failed: {0}")]
    Download(String),
}

// ── Trait ────────────────────────────────────────────────────────────

/// Trait abstracting embedding generation.
///
/// Document-side and query-side embedding go through distinct methods because
/// some models (notably `nomic-embed-text-v1.5`) are asymmetric and require
/// different task prefixes (`search_document: ` vs `search_query: `).
/// Callers MUST pass un-prefixed text — prefixes are applied internally by
/// each implementation. See ADR-003.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed multiple documents (chunks) for indexing.
    /// Implementations MUST apply any model-specific document prefix.
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Embed a single query for search.
    /// Implementations MUST apply any model-specific query prefix.
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// The dimension of vectors produced by this provider.
    fn dimension(&self) -> usize;

    /// A human-readable model identifier (e.g. `"nomic-embed-text-v1.5"`).
    fn model_name(&self) -> &str;
}

#[cfg(test)]
#[path = "embedding_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "embedding_openai_tests.rs"]
mod openai_tests;

#[cfg(test)]
#[path = "embedding_consistency_tests.rs"]
mod consistency_tests;
