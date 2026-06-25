//! Embedding provider commands — status, runtime reconfiguration, model
//! identity mismatch reporting, and model download.

use std::sync::Arc;

use super::settings_commands::settings_map;
use crate::AppState;
use serde::Serialize;
use tauri::Emitter;
use tauri::State;

/// Status of the active embedding backend, for the settings UI.
#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingProviderStatus {
    /// `"local"` (fastembed) or `"openai"` (cloud).
    pub backend: String,
    /// Active model identity (the value stored in `embed_model`).
    pub model: String,
    /// Output vector dimension.
    pub dimension: usize,
    /// Whether a cloud `embedding_api_key` is configured.
    pub api_key_configured: bool,
    /// Whether ONNX Runtime is bundled for this platform (local embeddings can
    /// run). `false` on e.g. macOS x86_64.
    pub local_available: bool,
    /// Whether the local `nomic-embed-text-v1.5` model is already downloaded.
    pub local_cached: bool,
}

/// Returns the current embedding-provider configuration status.
#[tauri::command]
pub async fn get_embedding_provider_status(
    state: State<'_, Arc<AppState>>,
) -> Result<EmbeddingProviderStatus, String> {
    use crate::providers::embedding::{local_embeddings_available, FastEmbedProvider};

    let map = settings_map(&state.db).await?;

    let local_available = local_embeddings_available();
    let default_backend = if local_available { "local" } else { "openai" };
    let backend = map
        .get("embedding_backend")
        .cloned()
        .unwrap_or_else(|| default_backend.to_string());

    let data_dir = crate::app_data_dir();
    let local_cached = FastEmbedProvider::is_cached(&FastEmbedProvider::cache_dir(&data_dir));

    let (model, dimension) = {
        let provider = state
            .embedding_provider
            .read()
            .map_err(|e| format!("Failed to read embedding provider: {e}"))?;
        (provider.model_name().to_string(), provider.dimension())
    };

    let api_key_configured = map
        .get("embedding_api_key")
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    Ok(EmbeddingProviderStatus {
        backend,
        model,
        dimension,
        api_key_configured,
        local_available,
        local_cached,
    })
}

/// Re-read settings and reconstruct the embedding provider at runtime. Returns
/// the active model identity. The caller should follow up with a model-mismatch
/// check / re-index if the identity changed.
#[tauri::command]
pub async fn reconfigure_embedding_provider(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    let map = settings_map(&state.db).await?;

    let data_dir = crate::app_data_dir();
    let new_provider = crate::build_embedding_provider_from_map(&map, &data_dir).await;
    let model = new_provider.model_name().to_string();

    *state
        .embedding_provider
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {e}"))? = new_provider;

    Ok(model)
}

/// Report which sources were embedded with a different model than the active one.
///
/// Returns an empty `stale` list when every indexed source matches the active
/// embedding provider's model ID. The mock provider (used as a placeholder
/// before the real model is downloaded) is treated as "no active model" and
/// always returns clean — it's not a real mismatch, just the pre-download state.
#[tauri::command]
pub async fn get_embedding_model_mismatch(
    state: State<'_, Arc<AppState>>,
) -> Result<crate::providers::embedding::EmbeddingModelMismatch, String> {
    let active = state
        .embedding_provider
        .read()
        .map_err(|e| format!("embedding lock: {e}"))?
        .model_name()
        .to_string();
    if active == "mock" {
        return Ok(crate::providers::embedding::EmbeddingModelMismatch {
            active_model: active,
            stale: Vec::new(),
        });
    }
    crate::providers::embedding::check_embedding_model_consistency(&state.db, &active)
        .await
        .map_err(|e| format!("mismatch check failed: {e}"))
}

/// Check whether the nomic-embed-text-v1.5 model is already cached.
#[tauri::command]
pub async fn check_embedding_model(_state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    let data_dir = crate::app_data_dir();
    let cache_dir = crate::providers::embedding::FastEmbedProvider::cache_dir(&data_dir);
    Ok(crate::providers::embedding::FastEmbedProvider::is_cached(
        &cache_dir,
    ))
}

/// Download the embedding model with streaming progress.
///
/// Progress events are emitted as `model-download-progress` with payload:
/// `{ status: "downloading"|"done"|"error", file, bytes_downloaded,
///   total_bytes, progress: 0.0-1.0 }`.
#[tauri::command]
pub async fn download_embedding_model(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let data_dir = crate::app_data_dir();
    let cache_dir = data_dir.join("embedding_model");

    // Ensure the cache directory exists
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .map_err(|e| format!("Failed to create cache dir: {e}"))?;

    // Emit initial progress
    let _ = app_handle.emit(
        "model-download-progress",
        serde_json::json!({
            "status": "downloading",
            "file": "",
            "bytes_downloaded": 0,
            "total_bytes": 0,
            "progress": 0.0,
        }),
    );

    // Download model files using reqwest streaming
    let client = reqwest::Client::new();

    // Download each model file
    let model_files = vec![
        ("tokenizer.json", "tokenizer.json"),
        ("config.json", "config.json"),
        ("special_tokens_map.json", "special_tokens_map.json"),
        ("tokenizer_config.json", "tokenizer_config.json"),
        ("onnx/model.onnx", "onnx/model.onnx"),
    ];

    // Ensure onnx subdirectory exists
    tokio::fs::create_dir_all(cache_dir.join("onnx"))
        .await
        .map_err(|e| format!("Failed to create onnx dir: {e}"))?;

    let total_files = model_files.len() as f32;
    let hf_base = "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main";

    for (i, (url_suffix, local_path)) in model_files.iter().enumerate() {
        let url = format!("{hf_base}/{url_suffix}");
        let dest = cache_dir.join(local_path);

        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to download {local_path}: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Failed to download {local_path}: HTTP {}",
                response.status()
            ));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = tokio::fs::File::create(&dest)
            .await
            .map_err(|e| format!("Failed to create {local_path}: {e}"))?;

        use tokio::io::AsyncWriteExt;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
            let chunk = chunk.map_err(|e| format!("Download stream error: {e}"))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("Write error: {e}"))?;
            downloaded += chunk.len() as u64;

            let file_progress = if total_size > 0 {
                downloaded as f32 / total_size as f32
            } else {
                0.0
            };
            let overall = (i as f32 + file_progress) / total_files;

            let _ = app_handle.emit(
                "model-download-progress",
                serde_json::json!({
                    "status": "downloading",
                    "file": local_path,
                    "bytes_downloaded": downloaded,
                    "total_bytes": total_size,
                    "progress": overall,
                }),
            );
        }

        file.flush()
            .await
            .map_err(|e| format!("Flush error: {e}"))?;
    }

    // Now initialize the real embedding provider from the cached files
    let model_dir = cache_dir.join("models--nomic-ai--nomic-embed-text-v1.5/snapshots/download");
    tokio::fs::create_dir_all(&model_dir)
        .await
        .map_err(|e| format!("Failed to create model dir: {e}"))?;
    tokio::fs::create_dir_all(model_dir.join("onnx"))
        .await
        .map_err(|e| format!("Failed to create model onnx dir: {e}"))?;

    // Copy downloaded files into hf-hub-compatible cache structure
    for (_, local_path) in &model_files {
        let src = cache_dir.join(local_path);
        let dst = cache_dir
            .join("models--nomic-ai--nomic-embed-text-v1.5/snapshots/download")
            .join(local_path);
        tokio::fs::copy(&src, &dst)
            .await
            .map_err(|e| format!("Failed to copy {local_path}: {e}"))?;
    }

    // Write a sentinel ref so we know the model is cached
    let refs_dir = cache_dir.join("models--nomic-ai--nomic-embed-text-v1.5/refs");
    tokio::fs::create_dir_all(&refs_dir)
        .await
        .map_err(|e| format!("Failed to create refs dir: {e}"))?;
    tokio::fs::write(refs_dir.join("main"), b"download")
        .await
        .map_err(|e| format!("Failed to write ref: {e}"))?;

    // Initialize the real FastEmbedProvider using the custom cache dir
    let real_provider = crate::providers::embedding::FastEmbedProvider::try_new(Some(&cache_dir))
        .map_err(|e| format!("Failed to initialize embedding model: {e}"))?;

    // Swap the provider in AppState
    *state
        .embedding_provider
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {e}"))? = Arc::new(real_provider);

    let _ = app_handle.emit(
        "model-download-progress",
        serde_json::json!({
            "status": "done",
            "file": "",
            "bytes_downloaded": 0,
            "total_bytes": 0,
            "progress": 1.0,
        }),
    );

    Ok(())
}
