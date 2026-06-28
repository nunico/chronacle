use async_trait::async_trait;

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
