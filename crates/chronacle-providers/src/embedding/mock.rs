use async_trait::async_trait;

use super::{EmbeddingError, EmbeddingProvider};

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
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.into_iter().map(|_| vec![0.0; self.dim]).collect())
    }

    async fn embed_query(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![0.0; self.dim])
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        &self.name
    }
}
