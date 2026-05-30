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
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

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

/// Subdirectory of the cache dir where hf-hub stores the model data.
const HF_HUB_MODEL_DIR: &str = "models--nomic-ai--nomic-embed-text-v1.5";

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
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// Embed multiple texts as a batch. Returns one vector per input.
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Embed a single query string.
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(self
            .embed(vec![text.to_string()])
            .await?
            .into_iter()
            .next()
            .unwrap_or_default())
    }

    /// The dimension of vectors produced by this provider.
    fn dimension(&self) -> usize;

    /// A human-readable model identifier (e.g. `"nomic-embed-text-v1.5"`).
    fn model_name(&self) -> &str;
}

// ── FastEmbed implementation ─────────────────────────────────────────

/// FastEmbed-backed implementation using nomic-embed-text-v1.5.
pub struct FastEmbedProvider {
    inner: tokio::sync::Mutex<TextEmbedding>,
    dim: usize,
    name: &'static str,
}

impl FastEmbedProvider {
    /// Try to create the provider using the standard fastembed mechanism.
    ///
    /// When `cache_dir` is `Some`, hf-hub uses that directory for model caching
    /// (expected structure: `{cache_dir}/models--nomic-ai--nomic-embed-text-v1.5/...`).
    /// When `None`, the default hf-hub cache is used (`~/.cache/huggingface/`).
    pub fn try_new(cache_dir: Option<&std::path::Path>) -> Result<Self, EmbeddingError> {
        let model_kind = EmbeddingModel::NomicEmbedTextV15;
        let dim = Self::model_dimension(&model_kind);
        let name = Self::model_to_name(&model_kind);

        let mut opts = InitOptions::new(model_kind).with_show_download_progress(false);
        if let Some(dir) = cache_dir {
            opts = opts.with_cache_dir(dir.to_path_buf());
        }

        let inner =
            TextEmbedding::try_new(opts).map_err(|e| EmbeddingError::Init(e.to_string()))?;

        Ok(Self {
            inner: tokio::sync::Mutex::new(inner),
            dim,
            name,
        })
    }

    /// Try to create from the small test model (all-MiniLM-L6-v2, ~80 MB).
    pub fn try_new_small() -> Result<Self, EmbeddingError> {
        let model_kind = EmbeddingModel::AllMiniLML6V2;
        let dim = Self::model_dimension(&model_kind);
        let name = Self::model_to_name(&model_kind);

        let inner =
            TextEmbedding::try_new(InitOptions::new(model_kind).with_show_download_progress(false))
                .map_err(|e| EmbeddingError::Init(e.to_string()))?;

        Ok(Self {
            inner: tokio::sync::Mutex::new(inner),
            dim,
            name,
        })
    }

    /// Check if model files are already cached in the given directory.
    pub fn is_cached(cache_dir: &std::path::Path) -> bool {
        let snapshot = cache_dir
            .join(HF_HUB_MODEL_DIR)
            .join("snapshots/download")
            .join("onnx/model.onnx");
        snapshot.exists()
    }

    /// The default cache directory under the given app data dir.
    pub fn cache_dir(app_data_dir: &std::path::Path) -> std::path::PathBuf {
        app_data_dir.join("embedding_model")
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
        let mut model = self.inner.lock().await;
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

// ── Mock provider (for tests / fallback) ──────────────────────────────

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
        Ok(texts.into_iter().map(|_| vec![0.0; self.dim]).collect())
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        &self.name
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

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
    async fn test_is_cached_returns_false_for_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!FastEmbedProvider::is_cached(dir.path()));
    }

    #[tokio::test]
    async fn test_fastembed_try_new_small() {
        match FastEmbedProvider::try_new_small() {
            Ok(provider) => {
                assert_eq!(provider.dimension(), 384);
                assert_eq!(provider.model_name(), "all-MiniLM-L6-v2");

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
