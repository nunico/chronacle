use std::sync::Arc;
use tauri::State;

use crate::services::entity_service::{
    self, EntityError, EntityGraph, EntityInput, EntityKind, GraphNode, RelatedEntity,
};
use crate::AppState;

fn parse_kind(kind: &str) -> Result<EntityKind, EntityError> {
    serde_json::from_value(serde_json::Value::String(kind.to_owned())).map_err(|_| {
        EntityError::InvalidKind {
            kind: kind.to_owned(),
        }
    })
}

/// Embed an entity's notes for semantic retrieval after a manual create/update.
///
/// Embedding failure is logged but never fails the save — a missing vector only
/// means the entity won't surface in semantic search until the next edit, which
/// is far less bad than losing the user's note.
async fn embed_after_save(state: &AppState, node: &GraphNode) {
    let provider = match state.embedding_provider.read() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("entity embed: provider lock poisoned: {e}");
            return;
        }
    };
    if let Err(e) = entity_service::embed_node(&state.db, &provider, node).await {
        eprintln!(
            "entity embed: failed to embed {} ({}); it will be missing from semantic search: {e}",
            node.name, node.kind
        );
    }
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

/// Campaign events in canonical timeline order (`sequence_index`, nulls last).
#[tauri::command]
pub async fn get_events_timeline(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<Vec<GraphNode>, EntityError> {
    entity_service::get_events_timeline(&state.db, &campaign_id).await
}

/// Ego graph (one hop) around an entity: center, neighbors, and edges.
#[tauri::command]
pub async fn get_entity_graph(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
    depth: u32,
) -> Result<EntityGraph, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::get_entity_graph(&state.db, &id, k.table_name(), depth).await
}

/// Per-kind entity counts for a campaign, keyed by table name (`npc`, …).
/// Used by the rail navigation to label entity categories.
#[tauri::command]
pub async fn get_entity_counts(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<std::collections::HashMap<String, u64>, EntityError> {
    entity_service::count_by_campaign(&state.db, &campaign_id).await
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
    let node = entity_service::create(&state.db, Some(&campaign_id), None, k, input).await?;
    embed_after_save(&state, &node).await;
    Ok(node)
}

#[tauri::command]
pub async fn update_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    let node = entity_service::update(&state.db, &id, k, input).await?;
    embed_after_save(&state, &node).await;
    Ok(node)
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

/// Flat relationships list for an entity: both inbound and outbound edges
/// resolved to named related entities.
#[tauri::command]
pub async fn get_entity_relations(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
) -> Result<Vec<RelatedEntity>, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::get_entity_relations(&state.db, &id, k.table_name()).await
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
