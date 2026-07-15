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

    let outcome = persist_alias_with_finding(
        || crate::entity_service::add_alias(db, &id, link_text, scope),
        || {
            crate::codex_service::record_lint(
                db,
                "auto_alias",
                json!({
                    "entity": id,
                    "alias": link_text,
                    "similarity": score,
                    "source": source_full_id,
                }),
            )
        },
        || crate::entity_service::remove_alias(db, &id, link_text),
        &id,
        link_text,
    )
    .await;

    match outcome {
        PersistOutcome::Persisted => Some(id),
        PersistOutcome::AliasWriteFailed
        | PersistOutcome::RolledBack
        | PersistOutcome::RollbackFailed => None,
    }
}

/// Outcome of attempting to persist an auto-resolved alias alongside its
/// review finding. Named states (rather than a bare `Result`) so the caller's
/// `match` reads as documentation of every path this invariant can take.
#[derive(Debug, PartialEq, Eq)]
enum PersistOutcome {
    /// Both writes succeeded — the alias is live and reviewable.
    Persisted,
    /// The alias write itself failed (e.g. a collision); nothing was
    /// persisted, so there is nothing to roll back.
    AliasWriteFailed,
    /// The alias was written, the finding write then failed, and the
    /// rollback (removing the alias) succeeded — back to a clean state so
    /// the next pass retries this variant from scratch.
    RolledBack,
    /// The alias was written, the finding write then failed, AND the
    /// rollback itself failed. Without a transaction this is the one
    /// unavoidable inconsistent state (an alias with no matching finding);
    /// it is logged loudly so it is at least diagnosable.
    RollbackFailed,
}

/// Persist an auto-resolved alias and its accompanying `auto_alias` review
/// finding as a pair, preserving the invariant this tier depends on for
/// safety:
///
/// > an auto-created alias EXISTS  <=>  an `auto_alias` finding exists for it
///
/// so the GM can see and undo every decision this tier made unattended.
///
/// The alias is written first (so a resolvable alias always has *something*
/// backing it while the finding write is in flight). If the finding write
/// then fails, the alias is rolled back via `remove_alias` — restoring the
/// clean pre-attempt state — rather than left orphaned with no way for the
/// GM to review or undo it. A rollback failure is the one state this
/// best-effort (non-transactional) approach cannot avoid; it is logged
/// clearly so it stays diagnosable.
///
/// Takes the three writes as closures (rather than concrete DB calls) so the
/// ordering/rollback logic itself is unit-testable without a real database —
/// see the tests below, which drive it with a deliberately-failing `record_lint`.
async fn persist_alias_with_finding<AddFut, LintFut, RemFut>(
    add_alias: impl FnOnce() -> AddFut,
    record_lint: impl FnOnce() -> LintFut,
    remove_alias: impl FnOnce() -> RemFut,
    id: &str,
    link_text: &str,
) -> PersistOutcome
where
    AddFut: std::future::Future<Output = Result<(), crate::entity_service::EntityError>>,
    LintFut: std::future::Future<Output = Result<(), String>>,
    RemFut: std::future::Future<Output = Result<(), crate::entity_service::EntityError>>,
{
    if let Err(e) = add_alias().await {
        eprintln!(
            "wikilink: fuzzy auto-resolve alias write failed for {id} <- {link_text:?}: {e}; \
             link stays unresolved this pass"
        );
        return PersistOutcome::AliasWriteFailed;
    }

    if let Err(e) = record_lint().await {
        eprintln!(
            "wikilink: fuzzy auto-resolve lint write failed for {id} <- {link_text:?}: {e}; \
             rolling back the alias to preserve the auto_alias invariant"
        );
        return match remove_alias().await {
            Ok(()) => {
                eprintln!(
                    "wikilink: rollback succeeded for {id} <- {link_text:?}; \
                     link stays unresolved this pass and will be retried"
                );
                PersistOutcome::RolledBack
            }
            Err(re) => {
                eprintln!(
                    "wikilink: ROLLBACK FAILED for {id} <- {link_text:?} after lint write \
                     error ({e}); rollback error: {re}. The alias may now be persisted with \
                     NO matching auto_alias finding — this state is invisible to the GM and \
                     requires manual investigation."
                );
                PersistOutcome::RollbackFailed
            }
        };
    }

    PersistOutcome::Persisted
}

#[cfg(test)]
mod persist_alias_with_finding_tests {
    use std::cell::Cell;

    use super::{persist_alias_with_finding, PersistOutcome};
    use crate::entity_service::EntityError;

    fn db_err(msg: &str) -> EntityError {
        EntityError::Database {
            message: msg.to_string(),
        }
    }

    /// The invariant this whole tier depends on: an alias write that
    /// succeeds followed by a finding write that fails must NOT leave the
    /// alias persisted with no finding — it must be rolled back.
    #[tokio::test]
    async fn a_lint_write_failure_after_a_successful_alias_write_rolls_back_the_alias() {
        let removed = Cell::new(false);

        let outcome = persist_alias_with_finding(
            || async { Ok(()) },
            || async { Err("simulated lint-write failure".to_string()) },
            || {
                removed.set(true);
                async { Ok(()) }
            },
            "faction:q",
            "The Quassars",
        )
        .await;

        assert_eq!(outcome, PersistOutcome::RolledBack);
        assert!(
            removed.get(),
            "remove_alias must be called to undo the orphaned alias"
        );
    }

    #[tokio::test]
    async fn a_rollback_failure_is_reported_distinctly() {
        let outcome = persist_alias_with_finding(
            || async { Ok(()) },
            || async { Err("simulated lint-write failure".to_string()) },
            || async { Err(db_err("simulated rollback failure")) },
            "faction:q",
            "The Quassars",
        )
        .await;

        assert_eq!(outcome, PersistOutcome::RollbackFailed);
    }

    #[tokio::test]
    async fn an_alias_write_failure_never_attempts_a_rollback() {
        let removed = Cell::new(false);

        let outcome = persist_alias_with_finding(
            || async { Err(db_err("simulated alias-write failure")) },
            || async { Ok(()) },
            || {
                removed.set(true);
                async { Ok(()) }
            },
            "faction:q",
            "The Quassars",
        )
        .await;

        assert_eq!(outcome, PersistOutcome::AliasWriteFailed);
        assert!(
            !removed.get(),
            "there is nothing to roll back when the alias was never written"
        );
    }

    #[tokio::test]
    async fn both_writes_succeeding_persists_cleanly() {
        let outcome = persist_alias_with_finding(
            || async { Ok(()) },
            || async { Ok(()) },
            || async { Ok(()) },
            "faction:q",
            "The Quassars",
        )
        .await;

        assert_eq!(outcome, PersistOutcome::Persisted);
    }
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
