use std::sync::Arc;

use super::super::{
    EntityError, EntityInput, EntityKind, GraphNode, GraphNodeRecord, SELECT_SCOPE_ALIASES,
};
use chronacle_core::embedding::EmbeddingProvider;

/// Map an `Option` to a SurrealDB value, using explicit `NULL` for `None`.
///
/// Binding `Option::None` directly serializes to SurrealDB `NONE`, which
/// SCHEMAFULL `… | NULL` fields reject. Use this for every nullable field bind.
fn opt_value<T: Into<surrealdb::sql::Value>>(opt: Option<T>) -> surrealdb::sql::Value {
    opt.map_or(surrealdb::sql::Value::Null, Into::into)
}

/// Update an existing graph node. Returns NotFound if the record doesn't exist.
pub async fn update<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
    input: EntityInput,
    outbound: &dyn chronacle_core::VaultOutbound,
) -> Result<GraphNode, EntityError> {
    let sanitized_name = chronacle_core::sanitize_scalar(&input.name);
    if sanitized_name.is_empty() {
        return Err(EntityError::Validation {
            field: "name".to_string(),
            message: "Name is required".to_string(),
        });
    }
    let table = kind.table_name();
    let notes_for_wikilinks = input.notes.clone();
    let update_sql = format!(
        "UPDATE type::thing($table, $id) SET
            name           = $name,
            summary        = $summary,
            notes          = $notes,
            date_start     = $date_start,
            date_end       = $date_end,
            is_ongoing     = $is_ongoing,
            sequence_index = $sequence_index,
            era            = $era,
            duration_label = $duration_label,
            session        = IF $session_id IS NOT NONE THEN type::thing('session', $session_id) ELSE NULL END,
            player_name    = $player_name,
            character_class = $character_class,
            character_level = $character_level,
            status         = $status,
            codex_stale    = true,
            updated_at     = time::now();
         SELECT *, {SELECT_SCOPE_ALIASES} FROM type::thing($table, $id)"
    );
    let mut response = db
        .query(update_sql)
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .bind(("name", sanitized_name))
        // Nullable fields: bind explicit NULL (not NONE) on `None`. SCHEMAFULL
        // `string | NULL` / `int | NULL` fields reject NONE — binding
        // `Option::None` directly would silently abort the UPDATE.
        .bind(("summary", opt_value(input.summary)))
        .bind(("notes", opt_value(input.notes)))
        .bind(("date_start", opt_value(input.date_start)))
        .bind(("date_end", opt_value(input.date_end)))
        .bind(("is_ongoing", input.is_ongoing))
        .bind(("sequence_index", opt_value(input.sequence_index)))
        .bind(("era", opt_value(input.era)))
        .bind(("duration_label", opt_value(input.duration_label)))
        .bind(("session_id", input.session_id))
        .bind(("player_name", opt_value(input.player_name)))
        .bind(("character_class", opt_value(input.character_class)))
        .bind(("character_level", opt_value(input.character_level)))
        .bind(("status", opt_value(input.status)))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    // UPDATE is at index 0; SELECT is at index 1.
    let records: Vec<GraphNodeRecord> = response.take(1).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    let node: GraphNode = records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| EntityError::NotFound { id: id.to_string() })?;

    outbound.enqueue(chronacle_core::VaultRef {
        table: node.kind.clone(),
        id: node.id.clone(),
    });

    // Sync wikilinks in notes to relates_to edges (fire-and-forget).
    if let Some(ref notes) = notes_for_wikilinks {
        use crate::wikilink::WikilinkScope;
        let scope = match (node.campaign_id.as_deref(), node.collection_id.as_deref()) {
            (Some(cid), _) => Some(WikilinkScope::Campaign { campaign_id: cid }),
            (_, Some(col)) => Some(WikilinkScope::Collection { collection_id: col }),
            _ => None,
        };
        if let Some(scope) = scope {
            let _ = crate::wikilink::parse_and_sync_wikilinks(db, table, id, notes, scope).await;
        }
    }

    Ok(node)
}

/// Compose the document text used to embed an entity for semantic retrieval.
///
/// Includes name, summary, **and notes** so hand-written notes participate in
/// retrieval. Empty parts are skipped.
pub(crate) fn embed_text(name: &str, summary: Option<&str>, notes: Option<&str>) -> String {
    let mut text = name.trim().to_owned();
    if let Some(s) = summary {
        let s = s.trim();
        if !s.is_empty() {
            text.push_str(": ");
            text.push_str(s);
        }
    }
    if let Some(n) = notes {
        let n = n.trim();
        if !n.is_empty() {
            text.push('\n');
            text.push_str(n);
        }
    }
    text
}

/// Embed an entity's text (name + summary + notes) and persist the vector and
/// model ID onto the record.
///
/// A zero-length vector — e.g. from a mock provider — is a no-op so callers
/// never block a save on embedding.
pub async fn embed_node<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    embed: &Arc<dyn EmbeddingProvider>,
    node: &GraphNode,
) -> Result<(), EntityError> {
    let text = embed_text(&node.name, node.summary.as_deref(), node.notes.as_deref());
    let vecs = embed
        .embed_documents(vec![text])
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let vec = vecs.into_iter().next().unwrap_or_default();
    if vec.is_empty() {
        return Ok(());
    }
    let model = embed.model_name().to_owned();
    db.query("UPDATE type::thing($table, $id) SET embedding = $vec, embed_model = $model")
        .bind(("table", node.kind.clone()))
        .bind(("id", node.id.clone()))
        .bind(("vec", vec))
        .bind(("model", model))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    Ok(())
}
