use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use super::{EmbeddingError, EmbeddingProvider};

/// Platform-specific filename of the bundled ONNX Runtime dynamic library.
///
/// `build.rs` downloads the correct binary for the target into
/// `resources/onnxruntime/<this name>` and Tauri bundles it as a resource.
pub(super) const ORT_DYLIB_NAME: &str = if cfg!(target_os = "macos") {
    "libonnxruntime.dylib"
} else if cfg!(target_os = "windows") {
    "onnxruntime.dll"
} else {
    "libonnxruntime.so"
};

// Nomic asymmetric prefixes — applied internally by FastEmbedProvider.
const NOMIC_DOC_PREFIX: &str = "search_document: ";
const NOMIC_QUERY_PREFIX: &str = "search_query: ";
const E5_DOC_PREFIX: &str = "passage: ";
const E5_QUERY_PREFIX: &str = "query: ";

/// The local embedding models Chronacle supports.
///
/// Both modes intentionally produce 768-dimensional vectors, matching the
/// vector index schema. The model name is persisted with every indexed source,
/// so switching modes is detected by the existing stale-index check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalEmbeddingMode {
    /// Small, English-focused Nomic model.
    Nomic,
    /// Larger multilingual E5 Base model for German, French, Spanish and
    /// cross-language retrieval.
    MultilingualE5Base,
}

impl LocalEmbeddingMode {
    /// Canonical persisted embedding-mode setting value.
    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::Nomic => "local_nomic",
            Self::MultilingualE5Base => "local_multilingual",
        }
    }

    /// Stable model identity stored in source and chunk metadata.
    pub const fn model_name(self) -> &'static str {
        match self {
            Self::Nomic => "nomic-embed-text-v1.5",
            Self::MultilingualE5Base => "multilingual-e5-base",
        }
    }

    /// Hugging Face repository used by FastEmbed.
    pub const fn model_repo(self) -> &'static str {
        match self {
            Self::Nomic => "nomic-ai/nomic-embed-text-v1.5",
            Self::MultilingualE5Base => "intfloat/multilingual-e5-base",
        }
    }

    /// FastEmbed model variant.
    const fn model_kind(self) -> EmbeddingModel {
        match self {
            Self::Nomic => EmbeddingModel::NomicEmbedTextV15,
            Self::MultilingualE5Base => EmbeddingModel::MultilingualE5Base,
        }
    }

    /// Both supported local models match the 768-dimensional vector schema.
    pub const fn dimension(self) -> usize {
        768
    }

    fn cache_key(self) -> &'static str {
        match self {
            // Keep Nomic at its original location so an existing download
            // remains usable after migrating from embedding_backend.
            Self::Nomic => "",
            Self::MultilingualE5Base => "multilingual-e5-base",
        }
    }

    /// hf-hub's cache directory name for this model.
    pub fn hf_hub_model_dir(self) -> String {
        format!("models--{}", self.model_repo().replace('/', "--"))
    }

    /// Files needed by both supported FastEmbed text models.
    pub const fn model_files() -> &'static [(&'static str, &'static str)] {
        &[
            ("tokenizer.json", "tokenizer.json"),
            ("config.json", "config.json"),
            ("special_tokens_map.json", "special_tokens_map.json"),
            ("tokenizer_config.json", "tokenizer_config.json"),
            ("onnx/model.onnx", "onnx/model.onnx"),
        ]
    }

    /// Formats text for indexing according to the selected model's retrieval
    /// training convention.
    pub fn document_text(self, text: &str) -> String {
        let prefix = match self {
            Self::Nomic => NOMIC_DOC_PREFIX,
            Self::MultilingualE5Base => E5_DOC_PREFIX,
        };
        format!("{prefix}{text}")
    }

    /// Formats a user query according to the selected model's retrieval
    /// training convention.
    pub fn query_text(self, text: &str) -> String {
        let prefix = match self {
            Self::Nomic => NOMIC_QUERY_PREFIX,
            Self::MultilingualE5Base => E5_QUERY_PREFIX,
        };
        format!("{prefix}{text}")
    }
}

/// The platform-specific ONNX Runtime library filename, for callers (e.g. the
/// desktop shell) that resolve the bundled resource path themselves.
pub fn ort_dylib_name() -> &'static str {
    ORT_DYLIB_NAME
}

/// Locate the ONNX Runtime dynamic library **shipped beside the executable** in
/// a packaged app.
///
/// `fastembed` is built with the `ort-load-dynamic` feature, so ONNX Runtime is
/// loaded at runtime rather than linked. In a packaged app Tauri copies the lib
/// into the platform resource dir next to the binary.
///
/// A *source checkout* (dev, `tauri build --no-bundle`) has no such adjacent
/// copy — resolving it via `CARGO_MANIFEST_DIR` here would point at
/// `crates/chronacle-providers`, which holds no `resources/`. The desktop shell
/// bridges that gap by setting `ORT_DYLIB_PATH` from its own crate's resource
/// dir (see `resolve_onnxruntime_library_path`).
fn bundled_onnxruntime_library_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    // macOS bundle: <exe>/../Resources/resources/onnxruntime/<lib>.
    let mac = exe_dir
        .join("../Resources/resources/onnxruntime")
        .join(ORT_DYLIB_NAME);
    if mac.exists() {
        return Some(mac);
    }
    // Other platforms: <exe>/resources/onnxruntime/<lib>.
    let other = exe_dir.join("resources/onnxruntime").join(ORT_DYLIB_NAME);
    if other.exists() {
        return Some(other);
    }
    None
}

/// Locate a system- or Homebrew-installed ONNX Runtime library.
///
/// Fallback for targets with no bundled binary (notably macOS x86_64, which
/// Microsoft no longer ships): a user who runs `brew install onnxruntime` gets
/// real local embeddings with no extra configuration. Also picks up an
/// Apple-Silicon Homebrew install on dev machines.
///
/// The library is **unpinned** — we do not control its version — so this relies
/// on ONNX Runtime's ABI forward-compatibility (`GetApi(N)` succeeds on any
/// runtime ≥ N). The bundled path remains the version-controlled default.
fn system_onnxruntime_library_path() -> Option<std::path::PathBuf> {
    // Homebrew prefixes (Apple-Silicon `/opt/homebrew`, Intel `/usr/local`,
    // Linuxbrew) plus conventional system library directories.
    const SEARCH_DIRS: &[&str] = &[
        "/opt/homebrew/lib",
        "/usr/local/lib",
        "/home/linuxbrew/.linuxbrew/lib",
        "/usr/lib",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
    ];
    SEARCH_DIRS
        .iter()
        .map(|dir| std::path::Path::new(dir).join(ORT_DYLIB_NAME))
        .find(|p| p.exists())
}

/// Resolve an ONNX Runtime library.
///
/// Order: an explicit `ORT_DYLIB_PATH` (how the desktop shell points at its own
/// bundled resource in source checkouts / `--no-bundle` builds, where no
/// exe-adjacent copy exists) → the version-controlled lib beside the executable
/// in a packaged app → a system/Homebrew install.
pub(super) fn resolve_onnxruntime_library_path() -> Option<std::path::PathBuf> {
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH").map(std::path::PathBuf::from) {
        if path.exists() {
            return Some(path);
        }
    }
    bundled_onnxruntime_library_path().or_else(system_onnxruntime_library_path)
}

/// Point `ort` at an ONNX Runtime library before any session is built.
///
/// `ort` reads `ORT_DYLIB_PATH` once, lazily, when the first inference session is
/// created. We set it from the bundled resource (or a system/Homebrew install)
/// unless the caller already provided one (e.g. a developer override). Without
/// this, `ort` cannot find a dynamic library and every `try_new` fails at session
/// creation. Safe to call repeatedly — it is a no-op once the variable is set.
fn ensure_ort_dylib_path() {
    if std::env::var_os("ORT_DYLIB_PATH").is_some() {
        return;
    }
    if let Some(path) = resolve_onnxruntime_library_path() {
        // Edition 2021: `set_var` is safe. Called at startup before worker
        // threads touch the environment.
        std::env::set_var("ORT_DYLIB_PATH", path);
    }
}

/// Whether an ONNX Runtime library is available for this platform — i.e. whether
/// local `fastembed` embeddings can run at all. Checks the bundled binary first,
/// then a system/Homebrew install (`brew install onnxruntime`). Returns `false`
/// on targets with neither (notably a stock macOS x86_64, which Microsoft no
/// longer ships a binary for); the UI uses this to steer such users to a cloud
/// backend.
pub fn local_embeddings_available() -> bool {
    resolve_onnxruntime_library_path().is_some()
}

// ── FastEmbed implementation ─────────────────────────────────────────
/// FastEmbed-backed implementation using nomic-embed-text-v1.5.
pub struct FastEmbedProvider {
    inner: tokio::sync::Mutex<TextEmbedding>,
    dim: usize,
    mode: Option<LocalEmbeddingMode>,
    name: &'static str,
}

impl FastEmbedProvider {
    /// Try to create the provider using the standard fastembed mechanism.
    ///
    /// When `cache_dir` is `Some`, hf-hub uses that directory for model caching
    /// (expected structure: `{cache_dir}/models--nomic-ai--nomic-embed-text-v1.5/...`).
    /// When `None`, the default hf-hub cache is used (`~/.cache/huggingface/`).
    pub fn try_new(
        mode: LocalEmbeddingMode,
        cache_dir: Option<&std::path::Path>,
    ) -> Result<Self, EmbeddingError> {
        ensure_ort_dylib_path();
        let model_kind = mode.model_kind();

        let mut opts = InitOptions::new(model_kind).with_show_download_progress(false);
        if let Some(dir) = cache_dir {
            opts = opts.with_cache_dir(dir.to_path_buf());
        }

        let inner =
            TextEmbedding::try_new(opts).map_err(|e| EmbeddingError::Init(e.to_string()))?;

        Ok(Self {
            inner: tokio::sync::Mutex::new(inner),
            dim: mode.dimension(),
            mode: Some(mode),
            name: mode.model_name(),
        })
    }

    /// Try to create from the small test model (all-MiniLM-L6-v2, ~80 MB).
    pub fn try_new_small() -> Result<Self, EmbeddingError> {
        ensure_ort_dylib_path();
        let model_kind = EmbeddingModel::AllMiniLML6V2;
        let dim = Self::model_dimension(&model_kind);
        let name = Self::model_to_name(&model_kind);

        let inner =
            TextEmbedding::try_new(InitOptions::new(model_kind).with_show_download_progress(false))
                .map_err(|e| EmbeddingError::Init(e.to_string()))?;

        Ok(Self {
            inner: tokio::sync::Mutex::new(inner),
            dim,
            mode: None,
            name,
        })
    }

    /// Check if model files are already cached in the given directory.
    pub fn is_cached(mode: LocalEmbeddingMode, cache_dir: &std::path::Path) -> bool {
        let snapshot = cache_dir
            .join(mode.hf_hub_model_dir())
            .join("snapshots/download")
            .join("onnx/model.onnx");
        snapshot.exists()
    }

    /// The default cache directory under the given app data dir.
    pub fn cache_dir(
        app_data_dir: &std::path::Path,
        mode: LocalEmbeddingMode,
    ) -> std::path::PathBuf {
        let root = app_data_dir.join("embedding_model");
        if mode.cache_key().is_empty() {
            root
        } else {
            root.join(mode.cache_key())
        }
    }

    pub(super) fn model_to_name(kind: &EmbeddingModel) -> &'static str {
        match kind {
            EmbeddingModel::NomicEmbedTextV15 => "nomic-embed-text-v1.5",
            EmbeddingModel::NomicEmbedTextV1 => "nomic-embed-text-v1",
            EmbeddingModel::AllMiniLML6V2 => "all-MiniLM-L6-v2",
            EmbeddingModel::AllMiniLML6V2Q => "all-MiniLM-L6-v2-quantized",
            _ => "unknown",
        }
    }

    pub(super) fn model_dimension(kind: &EmbeddingModel) -> usize {
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
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let prefixed: Vec<String> = texts
            .into_iter()
            .map(|text| match self.mode {
                Some(mode) => mode.document_text(&text),
                None => text,
            })
            .collect();
        let refs: Vec<&str> = prefixed.iter().map(|s| s.as_str()).collect();
        let mut model = self.inner.lock().await;
        model
            .embed(refs, None)
            .map_err(|e| EmbeddingError::Embed(e.to_string()))
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let prefixed = self
            .mode
            .map_or_else(|| text.to_string(), |mode| mode.query_text(text));
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
