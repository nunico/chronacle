use std::sync::Arc;
use tauri::State;

use crate::services::entity_service::{self, EntityError, EntityInput, EntityKind, GraphNode};
use crate::AppState;

fn parse_kind(kind: &str) -> Result<EntityKind, EntityError> {
    serde_json::from_value(serde_json::Value::String(kind.to_owned())).map_err(|_| {
        EntityError::InvalidKind {
            kind: kind.to_owned(),
        }
    })
}

#[tauri::command]
pub async fn get_entities(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    kind: String,
) -> Result<Vec<GraphNode>, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::get_by_campaign(&state.db, &campaign_id, k).await
}

#[tauri::command]
pub async fn get_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::get_by_id(&state.db, &id, k).await
}

#[tauri::command]
pub async fn create_entity(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::create(&state.db, Some(&campaign_id), k, input).await
}

#[tauri::command]
pub async fn update_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::update(&state.db, &id, k, input).await
}

#[tauri::command]
pub async fn delete_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
) -> Result<(), EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::delete(&state.db, &id, k).await
}

#[tauri::command]
pub async fn relate_entities(
    state: State<'_, Arc<AppState>>,
    from_id: String,
    from_kind: String,
    to_id: String,
    to_kind: String,
    rel_type: String,
    notes: Option<String>,
) -> Result<(), EntityError> {
    let from_k = parse_kind(&from_kind)?;
    let to_k = parse_kind(&to_kind)?;
    entity_service::relate(
        &state.db,
        &from_id,
        from_k.table_name(),
        &to_id,
        to_k.table_name(),
        &rel_type,
        notes,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kind_all_valid_variants() {
        let cases = [
            ("npc", EntityKind::Npc),
            ("location", EntityKind::Location),
            ("faction", EntityKind::Faction),
            ("creature", EntityKind::Creature),
            ("item", EntityKind::Item),
            ("event", EntityKind::Event),
            ("player_character", EntityKind::PlayerCharacter),
            ("misc", EntityKind::Misc),
        ];
        for (s, expected) in &cases {
            assert_eq!(parse_kind(s).unwrap(), *expected, "failed for {s}");
        }
    }

    #[test]
    fn parse_kind_invalid_returns_invalid_kind_error() {
        let err = parse_kind("dragon").unwrap_err();
        assert!(matches!(err, EntityError::InvalidKind { kind } if kind == "dragon"));
    }

    #[test]
    fn relate_entities_rejects_invalid_from_kind() {
        let err = parse_kind("goblin").unwrap_err();
        assert!(matches!(err, EntityError::InvalidKind { .. }));
    }
}
