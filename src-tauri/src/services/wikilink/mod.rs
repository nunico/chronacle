use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use surrealdb::sql::Thing;
use thiserror::Error;

mod edges;
mod query;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
pub enum WikilinkError {
    #[error("Database error: {message}")]
    Database { message: String },
    #[error("Invalid identifier '{value}': only [a-zA-Z0-9_] characters are allowed")]
    InvalidIdentifier { value: String },
    #[error("Malformed record ID '{value}': expected 'table:id'")]
    MalformedRecordId { value: String },
}

// ── Scope ─────────────────────────────────────────────────────────────────────

/// Determines which entities are candidates for wikilink resolution.
pub enum WikilinkScope<'a> {
    /// Source is a campaign entity. Resolves against:
    ///   - all entities in the campaign (`in_campaign` edges), AND
    ///   - all entities in collections the campaign subscribes to
    ///     (`subscribes_to->in_collection` chained traversal).
    Campaign { campaign_id: &'a str },

    /// Source is a collection entity. Resolves against:
    ///   - all entities in the same collection only (`in_collection` edges).
    Collection { collection_id: &'a str },
}

// ── Constants ─────────────────────────────────────────────────────────────────

const ENTITY_TABLES: &[&str] = &[
    "npc",
    "location",
    "faction",
    "creature",
    "item",
    "event",
    "player_character",
    "misc",
];

// Compiled once at first use — avoids recompiling the pattern on every call.
static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").expect("wikilink regex is valid"));

// ── Internal structs ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct EntityNameRow {
    id: Thing,
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct EntityNotesRow {
    id: Thing,
    name: String,
    notes: Option<String>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Parse `[[name]]` wikilinks from `notes`, resolve them against all entity
/// tables within `scope`, and (when `source_table` is one of the 8 entity
/// tables) maintain `relates_to` edges with `rel_type = "mentioned"`.
///
/// Returns the full record IDs (e.g. `"npc:abc123"`) of every matched entity.
/// Callers whose source table is *not* an entity table (e.g. `"session"`) must
/// persist the returned IDs themselves.
pub async fn parse_and_sync_wikilinks<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    source_table: &str,
    source_id: &str,
    notes: &str,
    scope: WikilinkScope<'_>,
) -> Result<Vec<String>, WikilinkError> {
    validate_identifier(source_table)?;
    validate_identifier(source_id)?;

    let extracted: Vec<String> = WIKILINK_RE
        .captures_iter(notes)
        .map(|cap| cap[1].trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if extracted.is_empty() {
        if ENTITY_TABLES.contains(&source_table) {
            edges::delete_stale_mentioned_edges(db, source_table, source_id, &[]).await?;
        }
        return Ok(vec![]);
    }

    let all_entities = query::query_all_entity_names(db, &scope).await?;

    let mut matched_ids: Vec<String> = extracted
        .iter()
        .filter_map(|wikilink_name| {
            let lower = wikilink_name.to_lowercase();
            all_entities
                .iter()
                .find(|(_, name)| name.to_lowercase() == lower)
                .map(|(id, _)| id.clone())
        })
        .collect();

    matched_ids.sort_unstable();
    matched_ids.dedup();

    if ENTITY_TABLES.contains(&source_table) {
        edges::upsert_mentioned_edges(db, source_table, source_id, &matched_ids).await?;
        edges::delete_stale_mentioned_edges(db, source_table, source_id, &matched_ids).await?;
    }

    Ok(matched_ids)
}

/// When a NEW entity is created, scan every existing in-scope entity whose
/// `notes` already contains a `[[wikilink]]` matching `new_name` and create
/// an inbound `relates_to` edge from that entity to the new one.
///
/// This reconciles "forward references" — notes written before the target
/// entity existed — without requiring the author to re-save the source entity.
///
/// Scope rules mirror [`parse_and_sync_wikilinks`].
/// Self-links are skipped. Duplicate edges are prevented via the same
/// delete-then-relate pattern used in `upsert_mentioned_edges`.
pub async fn sync_inbound_wikilinks_for_new_entity<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    new_table: &str,
    new_id: &str,
    new_name: &str,
    scope: WikilinkScope<'_>,
) -> Result<(), WikilinkError> {
    validate_identifier(new_table)?;
    validate_identifier(new_id)?;

    let all_with_notes = query::query_all_entity_notes(db, &scope).await?;
    let new_name_lower = new_name.trim().to_lowercase();
    let new_full_id = format!("{new_table}:{new_id}");

    for (source_full_id, _source_name, notes) in all_with_notes {
        let Some(notes_text) = notes else { continue };
        if notes_text.is_empty() || source_full_id == new_full_id {
            continue;
        }

        let mentions_new = WIKILINK_RE
            .captures_iter(&notes_text)
            .any(|cap| cap[1].trim().to_lowercase() == new_name_lower);

        if !mentions_new {
            continue;
        }

        let (src_table, src_id) = split_record_id(&source_full_id)?;
        validate_identifier(src_table)?;
        validate_identifier(src_id)?;

        let delete_query = format!(
            "DELETE relates_to \
             WHERE in = {src_table}:{src_id} \
             AND out = {new_table}:{new_id} \
             AND rel_type = 'mentioned'"
        );
        db.query(delete_query)
            .await
            .map_err(|e| WikilinkError::Database {
                message: e.to_string(),
            })?;

        if edges::has_higher_tier_edge(db, src_table, src_id, new_table, new_id).await? {
            continue;
        }

        let relate_query = format!(
            "RELATE {src_table}:{src_id}->relates_to->{new_table}:{new_id} \
             SET rel_type = 'mentioned', notes = NULL, created_at = time::now()"
        );
        db.query(relate_query)
            .await
            .map_err(|e| WikilinkError::Database {
                message: e.to_string(),
            })?;
    }

    Ok(())
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Reject any string that is empty or contains characters outside `[a-zA-Z0-9_]`.
fn validate_identifier(s: &str) -> Result<(), WikilinkError> {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(WikilinkError::InvalidIdentifier {
            value: s.to_string(),
        })
    }
}

/// Validate a full record ID such as `"npc:abc123"`.
fn validate_record_id(s: &str) -> Result<(), WikilinkError> {
    let (table, id) = split_record_id(s)?;
    validate_identifier(table)?;
    validate_identifier(id)?;
    Ok(())
}

/// Split a full record ID like `"npc:abc123"` into `("npc", "abc123")`.
fn split_record_id(full_id: &str) -> Result<(&str, &str), WikilinkError> {
    let pos = full_id
        .find(':')
        .ok_or_else(|| WikilinkError::MalformedRecordId {
            value: full_id.to_string(),
        })?;
    Ok((&full_id[..pos], &full_id[pos + 1..]))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "wikilink_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wikilink_tests_extra.rs"]
mod tests_extra;

#[cfg(test)]
#[path = "wikilink_tests_scope.rs"]
mod tests_scope;
