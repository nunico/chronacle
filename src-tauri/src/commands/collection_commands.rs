//! Collection commands — CRUD plus campaign↔collection subscriptions.

use std::sync::Arc;

use crate::AppState;
use serde::Serialize;
use tauri::State;

/// IPC response shape for a `collection` record.
#[derive(Debug, Clone, Serialize)]
pub struct CollectionResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<crate::services::collection_service::Collection> for CollectionResponse {
    fn from(c: crate::services::collection_service::Collection) -> Self {
        Self {
            id: c.id,
            name: c.name,
            description: c.description,
        }
    }
}

/// Returns all collections ordered by name.
#[tauri::command]
pub async fn get_collections(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<CollectionResponse>, String> {
    let collections = crate::services::collection_service::get_all(&state.db).await?;
    Ok(collections.into_iter().map(Into::into).collect())
}

/// Creates a new collection.  `name` must be non-empty.
#[tauri::command]
pub async fn create_collection(
    state: State<'_, Arc<AppState>>,
    name: String,
    description: Option<String>,
) -> Result<CollectionResponse, String> {
    if name.trim().is_empty() {
        return Err("Collection name is required".to_string());
    }
    let c =
        crate::services::collection_service::create(&state.db, name.trim(), description.as_deref())
            .await?;
    Ok(c.into())
}

/// Updates the name and description of an existing collection.
#[tauri::command]
pub async fn update_collection(
    state: State<'_, Arc<AppState>>,
    id: String,
    name: String,
    description: Option<String>,
) -> Result<CollectionResponse, String> {
    if name.trim().is_empty() {
        return Err("Collection name is required".to_string());
    }
    let c = crate::services::collection_service::update(
        &state.db,
        &id,
        name.trim(),
        description.as_deref(),
    )
    .await?;
    Ok(c.into())
}

/// Deletes a collection.  Fails if any campaigns are subscribed or any sources
/// still reference it.
#[tauri::command]
pub async fn delete_collection(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    crate::services::collection_service::delete(&state.db, &id).await
}

/// Subscribes a campaign to a collection.  Idempotent.
#[tauri::command]
pub async fn add_campaign_collection(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    collection_id: String,
) -> Result<(), String> {
    crate::services::collection_service::add_campaign_collection(
        &state.db,
        &campaign_id,
        &collection_id,
    )
    .await
}

/// Removes a campaign's subscription to a collection.
#[tauri::command]
pub async fn remove_campaign_collection(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    collection_id: String,
) -> Result<(), String> {
    crate::services::collection_service::remove_campaign_collection(
        &state.db,
        &campaign_id,
        &collection_id,
    )
    .await
}

/// Returns all collections to which a campaign is subscribed.
#[tauri::command]
pub async fn get_campaign_collections(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<Vec<CollectionResponse>, String> {
    let cols =
        crate::services::collection_service::get_campaign_collections(&state.db, &campaign_id)
            .await?;
    Ok(cols.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_response_from_collection() {
        let c = crate::services::collection_service::Collection {
            id: "abc123".to_string(),
            name: "D&D 5e Core".to_string(),
            description: Some("Core rulebooks".to_string()),
        };
        let resp = CollectionResponse::from(c);
        assert_eq!(resp.id, "abc123");
        assert_eq!(resp.name, "D&D 5e Core");
        assert_eq!(resp.description.as_deref(), Some("Core rulebooks"));
    }

    #[test]
    fn collection_response_from_collection_no_desc() {
        let c = crate::services::collection_service::Collection {
            id: "xyz".to_string(),
            name: "Pathfinder".to_string(),
            description: None,
        };
        let resp = CollectionResponse::from(c);
        assert!(resp.description.is_none());
    }
}
