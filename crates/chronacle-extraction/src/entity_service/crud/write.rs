use super::super::{EntityError, EntityInput, EntityKind, GraphNode};

/// Create a new graph node of the given kind, scoped to a campaign or collection.
///
/// Scope membership is recorded via `in_campaign` or `in_collection` edge tables
/// (not scalar fields). After creation, any `[[wikilinks]]` in `notes` are
/// resolved against the same scope and synced to `relates_to` edges.
pub async fn create<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: Option<&str>,
    collection_id: Option<&str>,
    kind: EntityKind,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let sanitized_name = chronacle_core::sanitize_scalar(&input.name);
    if sanitized_name.is_empty() {
        return Err(EntityError::Validation {
            field: "name".to_string(),
            message: "Name is required".to_string(),
        });
    }
    let table = kind.table_name();
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let id_for_wikilinks = id.clone();
    let notes_for_wikilinks = input.notes.clone();

    db.query(
        "CREATE type::thing($table, $id) SET
            name            = $name,
            summary         = $summary,
            notes           = $notes,
            date_start      = $date_start,
            date_end        = $date_end,
            is_ongoing      = $is_ongoing,
            sequence_index  = $sequence_index,
            era             = $era,
            duration_label  = $duration_label,
            session         = IF $session_id IS NOT NONE THEN type::thing('session', $session_id) ELSE NULL END,
            player_name     = $player_name,
            character_class = $character_class,
            character_level = $character_level,
            status          = $status,
            created_at      = time::now(),
            updated_at      = time::now()",
    )
    .bind(("table", table))
    .bind(("id", id.clone()))
    .bind(("name", sanitized_name))
    .bind(("summary", input.summary))
    .bind(("notes", input.notes))
    .bind(("date_start", input.date_start))
    .bind(("date_end", input.date_end))
    .bind(("is_ongoing", input.is_ongoing))
    .bind(("sequence_index", input.sequence_index))
    .bind(("era", input.era))
    .bind(("duration_label", input.duration_label))
    .bind(("session_id", input.session_id))
    .bind(("player_name", input.player_name))
    .bind(("character_class", input.character_class))
    .bind(("character_level", input.character_level))
    .bind(("status", input.status))
    .await
    .map_err(|e| EntityError::Database { message: e.to_string() })?;

    if let Some(cid) = campaign_id {
        db.query(
            "LET $src = type::thing('campaign', $cid); \
             LET $dst = type::thing($table, $id); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .bind(("cid", cid.to_owned()))
        .bind(("table", table))
        .bind(("id", id.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    }
    if let Some(col) = collection_id {
        db.query(
            "LET $src = type::thing('collection', $col); \
             LET $dst = type::thing($table, $id); \
             RELATE $src->in_collection->$dst SET created_at = time::now()",
        )
        .bind(("col", col.to_owned()))
        .bind(("table", table))
        .bind(("id", id.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    }

    // Build GraphNode from parameters — avoids re-fetch timing issues with
    // backward-traversal aliases immediately after creation in RocksDB engine.
    #[derive(serde::Deserialize)]
    struct StoredRecord {
        id: surrealdb::sql::Thing,
        #[serde(default)]
        name: String,
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        notes: Option<String>,
        #[serde(default)]
        created_at: Option<String>,
        #[serde(default)]
        updated_at: Option<String>,
        #[serde(default)]
        date_start: Option<String>,
        #[serde(default)]
        date_end: Option<String>,
        #[serde(default)]
        is_ongoing: Option<bool>,
        #[serde(default)]
        sequence_index: Option<i64>,
        #[serde(default)]
        era: Option<String>,
        #[serde(default)]
        duration_label: Option<String>,
        #[serde(default)]
        session: Option<surrealdb::sql::Thing>,
        #[serde(default)]
        player_name: Option<String>,
        #[serde(default)]
        character_class: Option<String>,
        #[serde(default)]
        character_level: Option<i64>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        codex_article: Option<String>,
        #[serde(default)]
        codex_stale: Option<bool>,
        #[serde(default)]
        codex_compiled_at: Option<surrealdb::sql::Datetime>,
    }
    let mut fetch_resp = db
        .query("SELECT * FROM type::thing($table, $id)")
        .bind(("table", table))
        .bind(("id", id.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let recs: Vec<StoredRecord> = fetch_resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    let rec = recs
        .into_iter()
        .next()
        .ok_or_else(|| EntityError::Database {
            message: "No record returned after create".to_string(),
        })?;

    let node = GraphNode {
        id: rec.id.id.to_raw(),
        kind: rec.id.tb.clone(),
        campaign_id: campaign_id.map(|s| s.to_owned()),
        collection_id: collection_id.map(|s| s.to_owned()),
        name: rec.name,
        summary: rec.summary,
        notes: rec.notes,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
        date_start: rec.date_start,
        date_end: rec.date_end,
        is_ongoing: rec.is_ongoing,
        sequence_index: rec.sequence_index,
        era: rec.era,
        duration_label: rec.duration_label,
        session_id: rec.session.map(|t| t.id.to_raw()),
        player_name: rec.player_name,
        character_class: rec.character_class,
        character_level: rec.character_level,
        status: rec.status,
        codex_article: rec.codex_article,
        codex_stale: rec.codex_stale,
        codex_compiled_at: rec.codex_compiled_at.map(|d| d.to_string()),
    };

    // Sync wikilinks (fire-and-forget: ignore errors so failures never block saves).
    if let Some(notes) = &notes_for_wikilinks {
        if !notes.is_empty() {
            use crate::wikilink::WikilinkScope;
            let scope = match (campaign_id, collection_id) {
                (Some(cid), _) => Some(WikilinkScope::Campaign { campaign_id: cid }),
                (_, Some(col)) => Some(WikilinkScope::Collection { collection_id: col }),
                _ => None,
            };
            if let Some(scope) = scope {
                let _ = crate::wikilink::parse_and_sync_wikilinks(
                    db,
                    table,
                    &id_for_wikilinks,
                    notes,
                    scope,
                )
                .await;
            }
        }
    }

    // Reconcile forward-reference wikilinks (fire-and-forget).
    {
        use crate::wikilink::WikilinkScope;
        let inbound_scope = match (campaign_id, collection_id) {
            (Some(cid), _) => Some(WikilinkScope::Campaign { campaign_id: cid }),
            (_, Some(col)) => Some(WikilinkScope::Collection { collection_id: col }),
            _ => None,
        };
        if let Some(inbound_scope) = inbound_scope {
            let _ = crate::wikilink::sync_inbound_wikilinks_for_new_entity(
                db,
                table,
                &id_for_wikilinks,
                &node.name,
                inbound_scope,
            )
            .await;
        }
    }

    Ok(node)
}

/// Hard-delete a graph node by id.
pub async fn delete<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
) -> Result<(), EntityError> {
    let table = kind.table_name();
    db.query("DELETE type::thing($table, $id)")
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    Ok(())
}

/// Soft-delete: hide the entity from the app and the vault without destroying
/// it. `delete` (hard) remains for genuine destruction.
pub async fn soft_delete<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
) -> Result<(), EntityError> {
    // `RETURN AFTER` on a nonexistent record returns an empty result set (UPDATE
    // never creates); deserialize only `id` — the full record contains
    // `Datetime`/`Thing` fields that `serde_json::Value` cannot deserialize
    // through the SurrealDB response decoder.
    #[derive(serde::Deserialize)]
    struct IdRow {
        #[allow(dead_code)] // only the row's presence is used, not its content
        id: surrealdb::sql::Thing,
    }
    let mut response = db
        .query(
            "UPDATE type::thing($table, $id) SET \
                 vault_deleted = true, updated_at = time::now() RETURN AFTER",
        )
        .bind(("table", kind.table_name()))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let rows: Vec<IdRow> = response.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    if rows.is_empty() {
        return Err(EntityError::NotFound { id: id.to_string() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity_service::crud::{get_by_campaign, get_by_id};
    use crate::entity_service::EntityInput;

    async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();
        db
    }

    /// After `soft_delete`, the entity must vanish from `get_by_campaign` (the
    /// app-facing list) and `get_by_id` must report `NotFound` — read paths
    /// filter `vault_deleted != true`.
    #[tokio::test]
    async fn soft_delete_hides_the_entity_from_read_paths() {
        let db = setup_db().await;
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        let node = create(
            &db,
            Some("camp1"),
            None,
            EntityKind::Npc,
            EntityInput {
                name: "Torvin".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        soft_delete(&db, &node.id, EntityKind::Npc).await.unwrap();

        let list = get_by_campaign(&db, "camp1", EntityKind::Npc)
            .await
            .unwrap();
        assert!(
            list.is_empty(),
            "soft-deleted entity must not appear in get_by_campaign"
        );

        let err = get_by_id(&db, &node.id, EntityKind::Npc).await.unwrap_err();
        assert!(matches!(err, EntityError::NotFound { .. }));
    }

    /// Soft-deleting an unknown id reports `NotFound`, matching `delete`'s
    /// error shape for a missing record.
    #[tokio::test]
    async fn soft_delete_of_unknown_id_returns_not_found() {
        let db = setup_db().await;
        let err = soft_delete(&db, "nope", EntityKind::Npc).await.unwrap_err();
        assert!(matches!(err, EntityError::NotFound { id } if id == "nope"));
    }
}
