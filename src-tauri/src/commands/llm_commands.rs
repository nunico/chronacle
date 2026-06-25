//! LLM provider commands — status and runtime reconfiguration.

use std::sync::Arc;

use super::settings_commands::settings_map;
use crate::AppState;
use serde::Serialize;
use tauri::State;

/// Response payload for the LLM provider status.
#[derive(Debug, Clone, Serialize)]
pub struct LlmProviderStatus {
    pub provider_type: String,
    pub model: String,
    pub api_key_configured: bool,
}

/// Returns the current LLM provider configuration status.
#[tauri::command]
pub async fn get_llm_provider_status(
    state: State<'_, Arc<AppState>>,
) -> Result<LlmProviderStatus, String> {
    let map = settings_map(&state.db).await?;

    Ok(LlmProviderStatus {
        provider_type: map
            .get("llm_provider")
            .cloned()
            .unwrap_or_else(|| "openai".into()),
        model: map.get("llm_model").cloned().unwrap_or_default(),
        api_key_configured: map
            .get("llm_api_key")
            .map(|k| !k.is_empty())
            .unwrap_or(false),
    })
}

/// Re-read settings from the database and reconstruct the LLM provider at
/// runtime. Returns the active provider type name on success.
#[tauri::command]
pub async fn reconfigure_llm_provider(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let map = settings_map(&state.db).await?;

    let new_provider = crate::build_llm_provider_from_map(&map, Some(&state.db)).await;
    let provider_type = crate::provider_type_name(&new_provider);

    // Swap the provider under the write lock
    *state
        .llm_provider
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {e}"))? = new_provider;

    Ok(provider_type.to_string())
}
