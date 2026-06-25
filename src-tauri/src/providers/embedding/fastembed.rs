use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use super::{EmbeddingError, EmbeddingProvider};

// Subdirectory of the cache dir where hf-hub stores the model data.
const HF_HUB_MODEL_DIR: &str = "models--nomic-ai--nomic-embed-text-v1.5";

// Nomic asymmetric prefixes — applied internally here.
const NOMIC_DOC_PREFIX: &str = "search_document: ";
const NOMIC_QUERY_PREFIX: &str = "search_query: ";

/// FastEmbed-backed embedding provider using nomic-embed-text-v1.5.
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
        super::ensure_ort_dylib_path();
        let model_kind = EmbeddingModel::NomicEmbedTextV15;
        let dim = Self::model_dimension(&model_kind);
        let name = Self::model_to_name(&model_kind);

        let mut opts = InitOptions::new(model_kind).with_show_download_progress(false);
        if let Some(dir) = cache_dir {
            opts = opts.with_cache_dir(dir.to_path_buf());
        }

        let inner =
            TextEmbedding::try_new(opts).map_err(|e| EmbeddingError::Init(e.to_string()))?;
        Ok(Self { inner: tokio::sync::Mutex::new(inner), dim, name })
    }

    /// Try to create from the small test model (all-MiniLM-L6-v2, ~80 MB).
    pub fn try_new_small() -> Result<Self, EmbeddingError> {
        super::ensure_ort_dylib_path();
        let model_kind = EmbeddingModel::AllMiniLML6V2;
        let dim = Self::model_dimension(&model_kind);
        let name = Self::model_to_name(&model_kind);

        let inner =
            TextEmbedding::try_new(InitOptions::new(model_kind).with_show_download_progress(false))
                .map_err(|e| EmbeddingError::Init(e.to_string()))?;
        Ok(Self { inner: tokio::sync::Mutex::new(inner), dim, name })
    }

    /// Check if model files are already cached in the given directory.
    pub fn is_cached(cache_dir: &std::path::Path) -> bool {
        cache_dir
            .join(HF_HUB_MODEL_DIR)
            .join("snapshots/download")
            .join("onnx/model.onnx")
            .exists()
    }

    /// The default cache directory under the given app data dir.
    pub fn cache_dir(app_data_dir: &std::path::Path) -> std::path::PathBuf {
        app_data_dir.join("embedding_model")
    }

    pub fn model_to_name(kind: &EmbeddingModel) -> &'static str {
        match kind {
            EmbeddingModel::NomicEmbedTextV15 => "nomic-embed-text-v1.5",
            EmbeddingModel::NomicEmbedTextV1 => "nomic-embed-text-v1",
            EmbeddingModel::AllMiniLML6V2 => "all-MiniLM-L6-v2",
            EmbeddingModel::AllMiniLML6V2Q => "all-MiniLM-L6-v2-quantized",
            _ => "unknown",
        }
    }

    pub fn model_dimension(kind: &EmbeddingModel) -> usize {
        match kind {
            EmbeddingModel::NomicEmbedTextV15 => 768,
            EmbeddingModel::NomicEmbedTextV1 => 768,
            EmbeddingModel::AllMiniLML6V2 => 384,
            EmbeddingModel::AllMiniLML6V2Q => 384,
            _ => 768,
        }
    }

    fn uses_nomic_prefixes(&self) -> bool {
        self.name.starts_with("nomic-embed-text")
    }
}

#[async_trait]
impl EmbeddingProvider for FastEmbedProvider {
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let prefixed: Vec<String> = if self.uses_nomic_prefixes() {
            texts.into_iter().map(|t| format!("{NOMIC_DOC_PREFIX}{t}")).collect()
        } else {
            texts
        };
        let refs: Vec<&str> = prefixed.iter().map(|s| s.as_str()).collect();
        let mut model = self.inner.lock().await;
        model.embed(refs, None).map_err(|e| EmbeddingError::Embed(e.to_string()))
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let prefixed = if self.uses_nomic_prefixes() {
            format!("{NOMIC_QUERY_PREFIX}{text}")
        } else {
            text.to_string()
        };
        let mut model = self.inner.lock().await;
        let mut out = model
            .embed(vec![prefixed.as_str()], None)
            .map_err(|e| EmbeddingError::Embed(e.to_string()))?;
        Ok(out.pop().unwrap_or_default())
    }

    fn dimension(&self) -> usize {
        self.dim
    }

    fn model_name(&self) -> &str {
        self.name
    }
}
