//! Custom provider commands — user-registered OpenAI/Anthropic-compatible
//! providers and their model lists.

use std::sync::Arc;

use crate::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct CustomProviderResponse {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderModelResponse {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
}

#[tauri::command]
pub async fn get_custom_providers(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CustomProviderResponse>, String> {
    let providers = crate::services::custom_provider_service::get_all(&state.db).await?;
    Ok(providers
        .into_iter()
        .map(|p| CustomProviderResponse {
            id: p.id,
            name: p.name,
            provider_type: p.provider_type,
            base_url: p.base_url,
            api_key: p.api_key,
        })
        .collect())
}

#[tauri::command]
pub async fn create_custom_provider(
    state: State<'_, Arc<AppState>>,
    name: String,
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<CustomProviderResponse, String> {
    if name.trim().is_empty() {
        return Err("Provider name is required".to_string());
    }
    if provider_type != "openai" && provider_type != "anthropic" {
        return Err("provider_type must be 'openai' or 'anthropic'".to_string());
    }
    if base_url.trim().is_empty() {
        return Err("Base URL is required".to_string());
    }
    let provider = crate::services::custom_provider_service::create(
        &state.db,
        name.trim(),
        &provider_type,
        base_url.trim(),
        &api_key,
    )
    .await?;
    Ok(CustomProviderResponse {
        id: provider.id,
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
    })
}

#[tauri::command]
pub async fn update_custom_provider(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    provider_type: String,
    base_url: String,
    api_key: String,
) -> Result<CustomProviderResponse, String> {
    let provider = crate::services::custom_provider_service::update(
        &state.db,
        &id,
        &name,
        &provider_type,
        &base_url,
        &api_key,
    )
    .await?;
    Ok(CustomProviderResponse {
        id: provider.id,
        name: provider.name,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        api_key: provider.api_key,
    })
}

#[tauri::command]
pub async fn delete_custom_provider(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    crate::services::custom_provider_service::delete(&state.db, &id).await
}

#[tauri::command]
pub async fn get_provider_models(
    state: State<'_, Arc<AppState>>,
    provider_id: String,
) -> Result<Vec<ProviderModelResponse>, String> {
    let models =
        crate::services::custom_provider_service::get_models(&state.db, &provider_id).await?;
    Ok(models
        .into_iter()
        .map(|m| ProviderModelResponse {
            id: m.id,
            provider_id: m.provider_id,
            model_id: m.model_id,
            display_name: m.display_name,
        })
        .collect())
}

#[tauri::command]
pub async fn add_provider_model(
    state: State<'_, Arc<AppState>>,
    provider_id: String,
    model_id: String,
    display_name: String,
) -> Result<ProviderModelResponse, String> {
    if model_id.trim().is_empty() {
        return Err("Model ID is required".to_string());
    }
    if display_name.trim().is_empty() {
        return Err("Display name is required".to_string());
    }
    let model = crate::services::custom_provider_service::add_model(
        &state.db,
        &provider_id,
        model_id.trim(),
        display_name.trim(),
    )
    .await?;
    Ok(ProviderModelResponse {
        id: model.id,
        provider_id: model.provider_id,
        model_id: model.model_id,
        display_name: model.display_name,
    })
}

#[tauri::command]
pub async fn remove_provider_model(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    crate::services::custom_provider_service::remove_model(&state.db, &id).await
}
