use async_trait::async_trait;

use super::{EmbeddingError, EmbeddingProvider};
use super::{CLOUD_EMBED_DIM, OPENAI_DEFAULT_EMBED_MODEL};

/// Normalise an OpenAI-compatible base URL to the API root (no trailing slash,
/// no `/embeddings` suffix). Empty input yields the public OpenAI endpoint.
pub(super) fn normalize_openai_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return "https://api.openai.com/v1".to_string();
    }
    trimmed
        .trim_end_matches("/embeddings")
        .trim_end_matches('/')
        .to_string()
}

/// Embedding provider backed by an OpenAI-compatible `/embeddings` endpoint.
///
/// Works with OpenAI directly and any compatible gateway (Azure OpenAI, proxies)
/// via `base_url`. Requests `dimensions: 768` so output matches the local
/// `nomic-embed-text-v1.5` index width. OpenAI embeddings are symmetric, so no
/// document/query prefixes are applied (unlike [`FastEmbedProvider`]).
pub struct OpenAiEmbeddingProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
    /// Stable identity persisted to `embed_model`, e.g.
    /// `openai:text-embedding-3-small:768`. A change here is detected by
    /// [`check_embedding_model_consistency`] and triggers re-indexing.
    name: String,
}

impl OpenAiEmbeddingProvider {
    /// Construct a provider. Empty `model`/`base_url` fall back to the OpenAI
    /// defaults. An empty `api_key` is allowed at construction; calls then fail
    /// with a clear configuration error.
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        let model = if model.trim().is_empty() {
            OPENAI_DEFAULT_EMBED_MODEL.to_string()
        } else {
            model.trim().to_string()
        };
        let base_url = normalize_openai_base_url(&base_url);
        let name = format!("openai:{model}:{CLOUD_EMBED_DIM}");
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
            base_url,
            name,
        }
    }

    async fn embed(&self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if self.api_key.trim().is_empty() {
            return Err(EmbeddingError::Init(
                "OpenAI embedding API key is not configured".to_string(),
            ));
        }
        let url = format!("{}/embeddings", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "input": inputs,
            "dimensions": CLOUD_EMBED_DIM,
            "encoding_format": "float",
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| EmbeddingError::Embed(format!("embeddings request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let detail = resp.text().await.unwrap_or_default();
            return Err(EmbeddingError::Embed(format!(
                "OpenAI embeddings HTTP {status}: {detail}"
            )));
        }

        #[derive(serde::Deserialize)]
        struct Item {
            embedding: Vec<f32>,
            index: usize,
        }
        #[derive(serde::Deserialize)]
        struct Body {
            data: Vec<Item>,
        }

        let mut parsed: Body = resp
            .json()
            .await
            .map_err(|e| EmbeddingError::Embed(format!("failed to parse embeddings: {e}")))?;
        // The API does not guarantee order; sort by the echoed input index.
        parsed.data.sort_by_key(|d| d.index);

        if let Some(first) = parsed.data.first() {
            if first.embedding.len() != CLOUD_EMBED_DIM {
                return Err(EmbeddingError::Embed(format!(
                    "expected {CLOUD_EMBED_DIM}-dim vectors, got {}",
                    first.embedding.len()
                )));
            }
        }
        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAiEmbeddingProvider {
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        self.embed(texts).await
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let mut vecs = self.embed(vec![text.to_string()]).await?;
        vecs.pop()
            .ok_or_else(|| EmbeddingError::Embed("empty embeddings response".to_string()))
    }

    fn dimension(&self) -> usize {
        CLOUD_EMBED_DIM
    }

    fn model_name(&self) -> &str {
        &self.name
    }
}
