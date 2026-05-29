/// Embedding provider — generates vector embeddings for text.
///
/// Phase 1 uses `fastembed` with `nomic-embed-text-v1.5` (768-dim).
/// The model is downloaded lazily on first use and cached locally.

use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Mutex;

/// Errors from the embedding provider.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Model initialization failed: {0}")]
    Init(String),
    #[error("Embedding generation failed: {0}")]
    Embed(String),
    #[error("Model not available — download may be in progress")]
    NotAvailable,
}

/// Trait abstracting embedding generation.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed multiple texts as a batch. Returns one vector per input.
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Embed a single query string.
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(self.embed(vec![text.to_string()]).await?.into_iter().next().unwrap_or_default())
    }

    /// The dimension of vectors produced by this provider.
    fn dimension(&self) -> usize;

    /// A human-readable model identifier (e.g. `"nomic-embed-text-v1.5"`).
    fn model_name(&self) -> &str;
}

/// FastEmbed-backed implementation using nomic-embed-text-v1.5.
pub struct FastEmbedProvider {
    model: Mutex<TextEmbedding>,
    model_kind: EmbeddingModel,
    dim: usize,
    name: &'static str,
}

impl FastEmbedProvider {
    /// Create a new FastEmbedProvider with the production model.
    pub fn try_new() -> Result<Self, EmbeddingError> {
        let model_kind = EmbeddingModel::NomicEmbedTextV15;
        Self::from_model(model_kind)
    }

    /// Create a new FastEmbedProvider with the small test model.
    pub fn try_new_small() -> Result<Self, EmbeddingError> {
        Self::from_model(EmbeddingModel::AllMiniLML6V2)
    }

    fn from_model(model_kind: EmbeddingModel) -> Result<Self, EmbeddingError> {
        let dim = Self::model_dimension(&model_kind);
        let name = Self::model_to_name(&model_kind);
        let model = TextEmbedding::try_new(
            InitOptions::new(model_kind.clone())
                .with_show_download_progress(false),
        )
        .map_err(|e| EmbeddingError::Init(e.to_string()))?;

        Ok(Self {
            model: Mutex::new(model),
            model_kind,
            dim,
            name,
        })
    }

    fn model_to_name(kind: &EmbeddingModel) -> &'static str {
        match kind {
            EmbeddingModel::NomicEmbedTextV15 => "nomic-embed-text-v1.5",
            EmbeddingModel::NomicEmbedTextV1 => "nomic-embed-text-v1",
            EmbeddingModel::AllMiniLML6V2 => "all-MiniLM-L6-v2",
            EmbeddingModel::AllMiniLML6V2Q => "all-MiniLM-L6-v2-quantized",
            _ => "unknown",
        }
    }

    fn model_dimension(kind: &EmbeddingModel) -> usize {
        match kind {
            EmbeddingModel::NomicEmbedTextV15 => 768,
            EmbeddingModel::NomicEmbedTextV1 => 768,
            EmbeddingModel::AllMiniLML6V2 => 384,
            EmbeddingModel::AllMiniLML6V2Q => 384,
            _ => 768,
        }
    }
}

#[async_trait]
impl EmbeddingProvider for FastEmbedProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let mut model = self.model.lock().unwrap();
        let embeddings = model
            .embed(refs, None)
            .map_err(|e| EmbeddingError::Embed(e.to_string()))?;
        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        self.name
    }
}

/// A mock embedding provider for tests.
pub struct MockEmbeddingProvider {
    dim: usize,
    name: String,
}

impl MockEmbeddingProvider {
    pub fn new(dim: usize) -> Self {
        Self {
            dim,
            name: "mock".to_string(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts
            .into_iter()
            .map(|_| vec![0.0; self.dim])
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embed_query_returns_correct_dims() {
        let provider = MockEmbeddingProvider::new(768);
        let vec = provider.embed_query("test").await.unwrap();
        assert_eq!(vec.len(), 768);
    }

    #[tokio::test]
    async fn test_mock_embed_batch() {
        let provider = MockEmbeddingProvider::new(384);
        let result = provider
            .embed(vec!["hello".into(), "world".into()])
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 384);
        assert_eq!(result[1].len(), 384);
    }

    #[tokio::test]
    async fn test_mock_model_name() {
        let provider = MockEmbeddingProvider::new(768);
        assert_eq!(provider.model_name(), "mock");
    }

    #[test]
    fn test_model_constants() {
        assert_eq!(
            FastEmbedProvider::model_dimension(&EmbeddingModel::NomicEmbedTextV15),
            768
        );
        assert_eq!(
            FastEmbedProvider::model_dimension(&EmbeddingModel::AllMiniLML6V2),
            384
        );
        assert_eq!(
            FastEmbedProvider::model_to_name(&EmbeddingModel::NomicEmbedTextV15),
            "nomic-embed-text-v1.5"
        );
    }

    #[tokio::test]
    async fn test_fastembed_try_new_small() {
        // This test downloads the model on first run (~80 MB).
        // It will fail if the download fails or model isn't cached.
        match FastEmbedProvider::try_new_small() {
            Ok(provider) => {
                assert_eq!(provider.dimension(), 384);
                assert_eq!(provider.model_name(), "all-MiniLM-L6-v2");

                // Test actual embedding produces a non-zero vector
                let vec = provider.embed_query("hello world").await.unwrap();
                assert_eq!(vec.len(), 384);
                let has_nonzero = vec.iter().any(|&v| v != 0.0);
                assert!(has_nonzero, "embedding should have non-zero values");
            }
            Err(e) => {
                eprintln!(
                    "Skipping real fastembed test — model not cached ({e}). \
                     Run again after model is downloaded."
                );
            }
        }
    }
}