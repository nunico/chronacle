use async_trait::async_trait;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use super::{EmbeddingError, EmbeddingProvider};

/// Subdirectory of the cache dir where hf-hub stores the model data.
const HF_HUB_MODEL_DIR: &str = "models--nomic-ai--nomic-embed-text-v1.5";

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

/// Locate the bundled ONNX Runtime dynamic library.
///
/// `fastembed` is built with the `ort-load-dynamic` feature, so ONNX Runtime is
/// loaded at runtime rather than linked. Mirrors `pdfium_library_path()` in
/// `lib.rs`: resolve via `CARGO_MANIFEST_DIR` in dev, and via the executable's
/// resource directory in a bundled app.
fn onnxruntime_library_path() -> Option<std::path::PathBuf> {
    // Dev: <manifest>/resources/onnxruntime/<lib>
    let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/onnxruntime")
        .join(ORT_DYLIB_NAME);
    if dev.exists() {
        return Some(dev);
    }
    // Bundled app: <exe>/../Resources/resources/onnxruntime/<lib> on macOS,
    // <exe>/resources/onnxruntime/<lib> elsewhere.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let mac = exe_dir
                .join("../Resources/resources/onnxruntime")
                .join(ORT_DYLIB_NAME);
            if mac.exists() {
                return Some(mac);
            }
            let other = exe_dir.join("resources/onnxruntime").join(ORT_DYLIB_NAME);
            if other.exists() {
                return Some(other);
            }
        }
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

/// Resolve an ONNX Runtime library: bundled resource first (version-controlled),
/// then a system/Homebrew install.
pub(super) fn resolve_onnxruntime_library_path() -> Option<std::path::PathBuf> {
    onnxruntime_library_path().or_else(system_onnxruntime_library_path)
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
    name: &'static str,
}

impl FastEmbedProvider {
    /// Try to create the provider using the standard fastembed mechanism.
    ///
    /// When `cache_dir` is `Some`, hf-hub uses that directory for model caching
    /// (expected structure: `{cache_dir}/models--nomic-ai--nomic-embed-text-v1.5/...`).
    /// When `None`, the default hf-hub cache is used (`~/.cache/huggingface/`).
    pub fn try_new(cache_dir: Option<&std::path::Path>) -> Result<Self, EmbeddingError> {
        ensure_ort_dylib_path();
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
