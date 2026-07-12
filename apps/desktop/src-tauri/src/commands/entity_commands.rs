use std::sync::Arc;
use tauri::State;

use crate::AppState;
use chronacle_extraction::entity_service::{
    self, EntityError, EntityGraph, EntityInput, EntityKind, GraphNode, RelatedEntity,
};

fn parse_kind(kind: &str) -> Result<EntityKind, EntityError> {
    serde_json::from_value(serde_json::Value::String(kind.to_owned())).map_err(|_| {
        EntityError::InvalidKind {
            kind: kind.to_owned(),
        }
    })
}

/// Inverse of [`parse_kind`] for a bare table name — table names equal the
/// serde kind strings, so this is the same lookup.
pub(crate) fn kind_of_table(table: &str) -> Result<EntityKind, EntityError> {
    parse_kind(table)
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

/// A create must be scoped to exactly one of a campaign or a collection —
/// never both, never neither. Split out so the XOR rule is unit-testable
/// without a Tauri `State`.
fn validate_create_scope(
    campaign_id: Option<&str>,
    collection_id: Option<&str>,
) -> Result<(), EntityError> {
    if campaign_id.is_some() == collection_id.is_some() {
        return Err(EntityError::Validation {
            field: "scope".to_string(),
            message: "Exactly one of campaignId or collectionId is required".to_string(),
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn create_entity(
    state: State<'_, Arc<AppState>>,
    campaign_id: Option<String>,
    collection_id: Option<String>,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    validate_create_scope(campaign_id.as_deref(), collection_id.as_deref())?;
    let outbound = state.outbound.read().await.clone();
    let node = entity_service::create(
        &state.db,
        campaign_id.as_deref(),
        collection_id.as_deref(),
        k,
        input,
    )
    .await?;
    outbound.enqueue(chronacle_core::VaultRef {
        table: node.kind.clone(),
        id: node.id.clone(),
    });
    embed_after_save(&state, &node).await;
    Ok(node)
}

/// Soft-delete: the record disappears from the app and (via the next
/// reconcile's orphan sweep) from the vault. Hard delete remains `delete_entity`.
#[tauri::command]
pub async fn soft_delete_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
) -> Result<(), EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::soft_delete(&state.db, &id, k).await?;
    // Latency: sweep the vault file now instead of waiting for the next sync.
    if let Some(svc) = state.vault.read().await.as_ref().map(Arc::clone) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = svc.reconcile().await {
                eprintln!("vault: post-soft-delete reconcile failed: {e}");
            }
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn update_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    let outbound = state.outbound.read().await.clone();
    let node = entity_service::update(&state.db, &id, k, input).await?;
    outbound.enqueue(chronacle_core::VaultRef {
        table: node.kind.clone(),
        id: node.id.clone(),
    });
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

/// Re-run wikilink resolution over every existing entity in the database,
/// turning stale forward-references into live `mentioned` edges now that all
/// entities exist.  Returns the number of entities whose notes were processed.
#[tauri::command]
pub async fn resync_wikilinks(state: State<'_, Arc<AppState>>) -> Result<usize, EntityError> {
    entity_service::resync_all_wikilinks(&state.db).await
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

/// Core logic for [`delete_relation`], split out so it can be exercised
/// directly against a bare connection in tests without constructing a Tauri
/// `State`.
///
/// `edge_id` arrives as the full record string "relates_to:<id>" (the form the
/// scope_violation lint payload stores); strip the table prefix so type::thing
/// rebuilds the correct Thing.
async fn delete_relation_impl<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    edge_id: &str,
) -> Result<(), String> {
    let raw = edge_id.strip_prefix("relates_to:").unwrap_or(edge_id);
    db.query("DELETE type::thing('relates_to', $id)")
        .bind(("id", raw.to_string()))
        .await
        .map_err(|e| format!("Failed to delete relation: {e}"))?
        .check()
        .map_err(|e| format!("Failed to delete relation: {e}"))?;
    Ok(())
}

/// Delete one `relates_to` edge by its record id (Maintenance resolve action).
#[tauri::command]
pub async fn delete_relation(
    state: State<'_, Arc<AppState>>,
    edge_id: String,
) -> Result<(), String> {
    delete_relation_impl(&state.db, &edge_id).await
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
    fn kind_of_table_is_the_inverse_of_parse_kind() {
        for table in [
            "npc",
            "location",
            "faction",
            "creature",
            "item",
            "event",
            "player_character",
            "misc",
        ] {
            assert_eq!(kind_of_table(table).unwrap(), parse_kind(table).unwrap());
        }
        assert!(
            kind_of_table("session").is_err(),
            "sessions are not entities"
        );
    }

    #[test]
    fn validate_create_scope_accepts_exactly_one_of_campaign_or_collection() {
        assert!(validate_create_scope(Some("camp1"), None).is_ok());
        assert!(validate_create_scope(None, Some("col1")).is_ok());
    }

    #[test]
    fn validate_create_scope_rejects_neither_or_both() {
        let err = validate_create_scope(None, None).unwrap_err();
        assert!(matches!(err, EntityError::Validation { field, .. } if field == "scope"));

        let err = validate_create_scope(Some("camp1"), Some("col1")).unwrap_err();
        assert!(matches!(err, EntityError::Validation { field, .. } if field == "scope"));
    }

    #[test]
    fn relate_entities_rejects_invalid_from_kind() {
        let err = parse_kind("goblin").unwrap_err();
        assert!(matches!(err, EntityError::InvalidKind { .. }));
    }

    /// Proves the scope-violation resolve round-trip: the lint detector stores
    /// the edge as the *full* record string `relates_to:<id>` (see
    /// `codex_service::lint::lint_scope_violations`), and `delete_relation`
    /// must strip that table prefix before rebuilding the `Thing`, or the
    /// delete silently matches nothing.
    #[tokio::test]
    async fn delete_relation_removes_edge_given_full_record_string() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE npc:`a` SET name = 'A', created_at = time::now(), updated_at = time::now();
             CREATE npc:`b` SET name = 'B', created_at = time::now(), updated_at = time::now();",
        )
        .await
        .unwrap()
        .check()
        .unwrap();

        entity_service::relate(&db, "a", "npc", "b", "npc", "member_of", None)
            .await
            .unwrap();

        #[derive(serde::Deserialize)]
        struct EdgeRow {
            id: surrealdb::sql::Thing,
        }
        let mut resp = db
            .query("SELECT id FROM relates_to")
            .await
            .unwrap()
            .check()
            .unwrap();
        let rows: Vec<EdgeRow> = resp.take(0).unwrap();
        assert_eq!(rows.len(), 1, "expected exactly one edge before delete");

        // Mirror the exact payload format lint_scope_violations records:
        // "relates_to:<raw id>", the FULL record string, not a bare id.
        let edge_id = format!("relates_to:{}", rows[0].id.id.to_raw());

        delete_relation_impl(&db, &edge_id).await.unwrap();

        let mut resp = db
            .query("SELECT id FROM relates_to")
            .await
            .unwrap()
            .check()
            .unwrap();
        let rows_after: Vec<EdgeRow> = resp.take(0).unwrap();
        assert_eq!(
            rows_after.len(),
            0,
            "edge should be deleted when given the full 'relates_to:<id>' string"
        );
    }

    /// Smoke test: `delete_relation` command function is referenced so the
    /// compiler verifies its signature, imports, and return type.
    #[test]
    fn delete_relation_command_compiles() {
        let _ = delete_relation as fn(_, _) -> _;
    }

    /// Smoke test: `create_entity` and `soft_delete_entity` command functions
    /// are referenced so the compiler verifies their signatures, imports, and
    /// return types (including the new `collection_id` parameter).
    #[test]
    fn create_and_soft_delete_entity_commands_compile() {
        let _ = create_entity as fn(_, _, _, _, _) -> _;
        let _ = soft_delete_entity as fn(_, _, _) -> _;
    }
}
