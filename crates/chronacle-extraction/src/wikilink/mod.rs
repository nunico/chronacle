use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::json;
use surrealdb::sql::Thing;
use thiserror::Error;

use crate::naming;

mod edges;
mod query;
mod resolve;

pub(crate) use query::query_all_entity_names;
pub(crate) use resolve::{resolve_exact, EntityIdentity};

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
///
/// `Copy` because it is only ever `&'a str` fields — cheap to pass by value
/// into the tier-4 fuzzy path, which needs its own owned copy for the
/// `add_alias` side effect while the caller keeps using the original.
#[derive(Debug, Clone, Copy)]
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
    #[serde(default)]
    aliases: Vec<String>,
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
    let names_only: Vec<(String, String)> = all_entities
        .iter()
        .map(|e| (e.id.clone(), e.name.clone()))
        .collect();
    let source_full_id = format!("{source_table}:{source_id}");

    let mut matched_ids: Vec<String> = Vec::new();
    // A single wikilink text is only ever run through the fuzzy tier ONCE per
    // call, even if it appears multiple times in `notes` — the second
    // occurrence either already resolved via the alias the first occurrence
    // just persisted, or it is the same unresolved variant and re-attempting
    // fuzzy resolution would risk a duplicate alias write / lint finding.
    let mut fuzzy_attempted: HashSet<String> = HashSet::new();
    for wikilink_name in &extracted {
        if let Some(id) = resolve::resolve_exact(wikilink_name, &all_entities) {
            matched_ids.push(id);
            continue;
        }

        let key = wikilink_name.to_lowercase();
        if !fuzzy_attempted.insert(key) {
            continue;
        }
        if let Some(id) =
            try_fuzzy_resolve(db, wikilink_name, &names_only, scope, &source_full_id).await
        {
            matched_ids.push(id);
        }
    }

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

// ── Tier 4: fuzzy auto-resolve ──────────────────────────────────────────────

/// Tier 4 — the only tier that can be WRONG, so it is the only one that must
/// be unambiguous, persisted, and reviewable.
///
/// Fires only when [`naming::best_match`] returns `Unique`: exactly one
/// candidate clears [`naming::DEFAULT_THRESHOLD`]. On a unique hit it
/// PERSISTS the decision as an alias (so the next pass hits the deterministic
/// tier 2 — the fuzzy path runs once per variant, ever) and files an
/// `auto_alias` lint finding so the GM can review or undo it.
///
/// Either side effect failing (an alias collision, or a lint-write error)
/// leaves the link unresolved for THIS pass rather than guessing or aborting
/// — a whole extraction pass must never die because one alias could not be
/// written. An `Ambiguous` or `None` outcome also leaves the link unresolved;
/// it falls through to `broken_wikilink`, which carries ranked candidates for
/// a "did you mean …?" suggestion.
async fn try_fuzzy_resolve<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    link_text: &str,
    names_only: &[(String, String)],
    scope: WikilinkScope<'_>,
    source_full_id: &str,
) -> Option<String> {
    let naming::MatchOutcome::Unique { id, score, .. } =
        naming::best_match(link_text, names_only, naming::DEFAULT_THRESHOLD)
    else {
        return None;
    };
    let id = id.to_string();

    if let Err(e) = crate::entity_service::add_alias(db, &id, link_text, scope).await {
        eprintln!(
            "wikilink: fuzzy auto-resolve alias write failed for {id} <- {link_text:?}: {e}; \
             link stays unresolved this pass"
        );
        return None;
    }
    if let Err(e) = crate::codex_service::record_lint(
        db,
        "auto_alias",
        json!({
            "entity": id,
            "alias": link_text,
            "similarity": score,
            "source": source_full_id,
        }),
    )
    .await
    {
        eprintln!(
            "wikilink: fuzzy auto-resolve lint write failed for {id} <- {link_text:?}: {e}; \
             link stays unresolved this pass"
        );
        return None;
    }

    Some(id)
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
