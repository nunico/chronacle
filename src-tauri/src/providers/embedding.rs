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

// Nomic asymmetric prefixes — applied internally by FastEmbedProvider.
const NOMIC_DOC_PREFIX: &str = "search_document: ";
const NOMIC_QUERY_PREFIX: &str = "search_query: ";

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

    /// True for Nomic embed models, which require asymmetric task prefixes.
    fn uses_nomic_prefixes(&self) -> bool {
        self.name.starts_with("nomic-embed-text")
    }
}

#[async_trait]
impl EmbeddingProvider for FastEmbedProvider {
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let prefixed: Vec<String> = if self.uses_nomic_prefixes() {
            texts
                .into_iter()
                .map(|t| format!("{NOMIC_DOC_PREFIX}{t}"))
                .collect()
        } else {
            texts
        };
        let refs: Vec<&str> = prefixed.iter().map(|s| s.as_str()).collect();
        let mut model = self.inner.lock().await;
        model
            .embed(refs, None)
            .map_err(|e| EmbeddingError::Embed(e.to_string()))
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

// ── Model identity check ─────────────────────────────────────────────

/// Report of sources indexed with a different embedding model than the active one.
///
/// Returned by [`check_embedding_model_consistency`]. The `stale_models` field
/// lists the distinct `embed_model` values found in the `source` table that
/// disagree with the active embedding provider's model ID, along with the count
/// of affected sources per model.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbeddingModelMismatch {
    /// The model ID currently active in the embedding provider.
    pub active_model: String,
    /// Per stale model: how many sources are indexed with it.
    pub stale: Vec<StaleModelCount>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StaleModelCount {
    pub embed_model: String,
    pub source_count: u64,
}

impl EmbeddingModelMismatch {
    pub fn is_clean(&self) -> bool {
        self.stale.is_empty()
    }

    pub fn total_stale_sources(&self) -> u64 {
        self.stale.iter().map(|s| s.source_count).sum()
    }
}

/// Check whether any indexed sources were embedded with a different model than
/// the active embedding provider.
///
/// Returns the report describing affected sources. An empty `stale` list means
/// every indexed source matches the active model (or there are no sources yet).
///
/// See ADR-003: silently changing embedding models corrupts retrieval because
/// query vectors and indexed vectors live in different latent spaces.
pub async fn check_embedding_model_consistency<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    active_model: &str,
) -> Result<EmbeddingModelMismatch, surrealdb::Error> {
    #[derive(serde::Deserialize)]
    struct Row {
        embed_model: Option<String>,
    }
    let mut response = db.query("SELECT embed_model FROM source").await?.check()?;
    let rows: Vec<Row> = response.take(0)?;

    let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for row in rows {
        let model = match row.embed_model {
            Some(m) if !m.is_empty() => m,
            _ => continue,
        };
        if model == active_model {
            continue;
        }
        *counts.entry(model).or_insert(0) += 1;
    }

    let mut stale: Vec<StaleModelCount> = counts
        .into_iter()
        .map(|(embed_model, source_count)| StaleModelCount {
            embed_model,
            source_count,
        })
        .collect();
    stale.sort_by(|a, b| a.embed_model.cmp(&b.embed_model));

    Ok(EmbeddingModelMismatch {
        active_model: active_model.to_string(),
        stale,
    })
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
            .embed_documents(vec!["hello".into(), "world".into()])
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 384);
        assert_eq!(result[1].len(), 384);
    }

    #[tokio::test]
    async fn test_mock_provider_implements_split_trait() {
        let provider = MockEmbeddingProvider::new(384);
        let docs = provider
            .embed_documents(vec!["hello".into(), "world".into()])
            .await
            .unwrap();
        assert_eq!(docs.len(), 2);
        let q = provider.embed_query("hello").await.unwrap();
        assert_eq!(q.len(), 384);
    }

    #[tokio::test]
    async fn test_fastembed_document_and_query_paths_compile() {
        // Confirms the trait surface is wired correctly. Returns same-dimension
        // vectors for both methods. all-MiniLM-L6-v2 doesn't use prefixes, but
        // the Nomic-prefix logic is gated on the model name (see
        // uses_nomic_prefixes), so both paths exercise the trait shape.
        let Ok(provider) = FastEmbedProvider::try_new_small() else {
            eprintln!("Skipping — small model not cached");
            return;
        };
        let raw = "Lantern orbits the planet Mirovia";
        let as_doc = provider
            .embed_documents(vec![raw.to_string()])
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let as_query = provider.embed_query(raw).await.unwrap();
        assert_eq!(as_doc.len(), as_query.len());
        assert!(as_doc.iter().any(|&v| v != 0.0));
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

    // ── Model identity mismatch tests ────────────────────────────────

    async fn seed_db_with_sources(
        models: &[&str],
    ) -> surrealdb::Surreal<surrealdb::engine::local::Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("t").use_db("t").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE collection SET id='col1', name='Test', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap()
        .check()
        .unwrap();
        for (i, model) in models.iter().enumerate() {
            let q = format!(
                "CREATE source SET id='s{i}', filename='f{i}.pdf', display_name='F{i}', \
                 source_type='rules', page_count=1, indexed_at=time::now(), index_status='done', \
                 embed_model='{model}', collection=type::thing('collection','col1')"
            );
            db.query(q).await.unwrap().check().unwrap();
        }
        db
    }

    #[tokio::test]
    async fn mismatch_check_returns_clean_when_all_sources_match() {
        let db = seed_db_with_sources(&["nomic-embed-text-v1.5", "nomic-embed-text-v1.5"]).await;
        let report = check_embedding_model_consistency(&db, "nomic-embed-text-v1.5")
            .await
            .unwrap();
        assert!(report.is_clean());
        assert_eq!(report.total_stale_sources(), 0);
        assert_eq!(report.active_model, "nomic-embed-text-v1.5");
    }

    #[tokio::test]
    async fn mismatch_check_returns_clean_when_no_sources_indexed() {
        let db = seed_db_with_sources(&[]).await;
        let report = check_embedding_model_consistency(&db, "nomic-embed-text-v1.5")
            .await
            .unwrap();
        assert!(report.is_clean());
    }

    #[tokio::test]
    async fn mismatch_check_lists_stale_models_with_counts() {
        let db = seed_db_with_sources(&[
            "nomic-embed-text-v1.5",
            "all-MiniLM-L6-v2",
            "all-MiniLM-L6-v2",
            "nomic-embed-text-v1",
        ])
        .await;
        let report = check_embedding_model_consistency(&db, "nomic-embed-text-v1.5")
            .await
            .unwrap();
        assert!(!report.is_clean());
        assert_eq!(report.total_stale_sources(), 3);
        // Stale entries are sorted by model name for deterministic UI display.
        assert_eq!(report.stale.len(), 2);
        assert_eq!(report.stale[0].embed_model, "all-MiniLM-L6-v2");
        assert_eq!(report.stale[0].source_count, 2);
        assert_eq!(report.stale[1].embed_model, "nomic-embed-text-v1");
        assert_eq!(report.stale[1].source_count, 1);
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
