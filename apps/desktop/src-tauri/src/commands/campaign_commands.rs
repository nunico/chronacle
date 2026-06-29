//! Campaign commands — campaign CRUD over the `campaign_service`.

use std::sync::Arc;

use crate::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct CampaignResponse {
    pub id: String,
    pub name: String,
    pub system: String,
}

#[tauri::command]
pub async fn get_campaigns(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CampaignResponse>, String> {
    let campaigns = chronacle_domain::campaign_service::get_all(&state.db).await?;
    Ok(campaigns
        .into_iter()
        .map(|c| CampaignResponse {
            id: c.id,
            name: c.name,
            system: c.system,
        })
        .collect())
}

#[tauri::command]
pub async fn get_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<CampaignResponse, String> {
    let campaign = chronacle_domain::campaign_service::get_by_id(&state.db, &id).await?;
    Ok(CampaignResponse {
        id: campaign.id,
        name: campaign.name,
        system: campaign.system,
    })
}

#[tauri::command]
pub async fn create_campaign(
    state: State<'_, Arc<AppState>>,
    name: String,
    system: String,
) -> Result<CampaignResponse, String> {
    if name.trim().is_empty() {
        return Err("Campaign name is required".to_string());
    }
    let campaign =
        chronacle_domain::campaign_service::create(&state.db, name.trim(), system.trim()).await?;
    Ok(CampaignResponse {
        id: campaign.id,
        name: campaign.name,
        system: campaign.system,
    })
}

#[tauri::command]
pub async fn update_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    system: String,
) -> Result<CampaignResponse, String> {
    let campaign =
        chronacle_domain::campaign_service::update(&state.db, &id, &name, &system).await?;
    Ok(CampaignResponse {
        id: campaign.id,
        name: campaign.name,
        system: campaign.system,
    })
}

#[tauri::command]
pub async fn delete_campaign(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    chronacle_domain::campaign_service::delete(&state.db, &id).await
}
