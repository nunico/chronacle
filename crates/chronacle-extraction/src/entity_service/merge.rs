//! The merge operation: fold a `loser` entity into a `survivor`.
//!
//! The Maintenance inbox can DETECT a duplicate entity ("The Free League" vs
//! "Free League") but, until this module, could never FIX one. Merge is the
//! fix: it re-points every edge, keeps the loser's name as an alias, applies
//! per-field choices, marks the codex article stale, and soft-deletes the loser.

use serde::Deserialize;
use surrealdb::sql::Thing;

use super::{get_by_id, relate_collapsing, soft_delete, EntityError, EntityKind};

/// Which side of a per-field conflict to keep when merging.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldChoice {
    /// Keep the survivor's value; discard the loser's.
    KeepSurvivor,
    /// Keep the loser's value; discard the survivor's.
    KeepLoser,
    /// Concatenate both, the loser's under a `## Merged from <loser>` heading.
    KeepBoth,
}

/// The per-field decisions the GM makes when merging two entities. Crosses the
/// Tauri IPC boundary, so it deserializes from camelCase.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeChoices {
    /// What to do with the two `summary` fields.
    pub summary: FieldChoice,
    /// What to do with the two `notes` fields.
    pub notes: FieldChoice,
}

/// Resolve a per-field conflict. `KeepBoth` concatenates under a heading naming
/// the loser, so nothing is silently destroyed even when both sides had text.
fn choose(c: &FieldChoice, s: Option<&str>, l: Option<&str>, loser_name: &str) -> Option<String> {
    match c {
        FieldChoice::KeepSurvivor => s.map(str::to_string),
        FieldChoice::KeepLoser => l.map(str::to_string),
        FieldChoice::KeepBoth => match (s, l) {
            (Some(s), Some(l)) => Some(format!("{s}\n\n## Merged from {loser_name}\n\n{l}")),
            (Some(s), None) => Some(s.to_string()),
            (None, Some(l)) => Some(l.to_string()),
            (None, None) => None,
        },
    }
}

/// Map an `Option` to a SurrealDB value, using explicit `NULL` for `None`.
///
/// Binding `Option::None` directly serializes to SurrealDB `NONE`, which the
/// SCHEMAFULL `summary`/`notes` fields (`TYPE string | NULL`) reject on
/// `UPDATE` — `DEFAULT NULL` only backfills on `CREATE`. This is the same fact
/// that broke tranche 5; mirrors `crud::update::opt_value` (private to that
/// module, so replicated here rather than exported across an unrelated
/// boundary).
fn opt_value<T: Into<surrealdb::sql::Value>>(opt: Option<T>) -> surrealdb::sql::Value {
    opt.map_or(surrealdb::sql::Value::Null, Into::into)
}

/// One of the loser's `relates_to` edges, resolved down to the fields merge
/// needs to re-point it: which end was the loser, the other endpoint, the
/// relationship type, and the edge's own free-text `notes` (an authored GM
/// annotation, e.g. "betrayed the party in session 4") that must survive the
/// re-point rather than being silently dropped.
struct LoserEdge {
    other_table: String,
    other_id: String,
    rel_type: String,
    notes: Option<String>,
    /// `true` when the loser was the edge's `in` side (loser → other).
    loser_is_in: bool,
}

/// Fetch every `relates_to` edge touching `loser`, with `notes`, for
/// re-pointing during a merge.
///
/// This queries `relates_to` directly rather than reusing
/// [`super::get_entity_relations`], for two reasons: that helper does not
/// expose edge `notes` (see [`LoserEdge`]), and its specificity-tier
/// collapsing is unnecessary here — `relate_collapsing` already re-applies
/// that rule per edge as it re-points, so a raw, un-collapsed edge list is
/// fine to feed it.
///
/// Mirrors `relations::flat::get_entity_relations`'s
/// `vault_deleted != true` filter: an edge to an already soft-deleted
/// neighbour is dropped rather than re-pointed, so merge does not resurrect a
/// dangling reference to a record that no longer exists in any read path.
async fn loser_edges<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    l_id: &str,
    l_table: &str,
) -> Result<Vec<LoserEdge>, EntityError> {
    #[derive(Deserialize)]
    struct EdgeRow {
        #[serde(rename = "in")]
        in_: Thing,
        out: Thing,
        rel_type: String,
        notes: Option<String>,
    }

    let mut resp = db
        .query(format!(
            "SELECT in, out, rel_type, notes FROM relates_to \
             WHERE in = {l_table}:{l_id} OR out = {l_table}:{l_id}"
        ))
        .await
        .map_err(db_err)?;
    let rows: Vec<EdgeRow> = resp.take(0).map_err(db_err)?;

    let mut edges: Vec<LoserEdge> = Vec::new();
    for row in rows {
        let in_id = row.in_.id.to_raw();
        let out_id = row.out.id.to_raw();
        let loser_is_in = in_id == l_id && row.in_.tb == l_table;
        let (other_table, other_id) = if loser_is_in {
            (row.out.tb.clone(), out_id)
        } else {
            (row.in_.tb.clone(), in_id)
        };
        // Self-loop: loser both ends.
        if other_table == l_table && other_id == l_id {
            continue;
        }
        edges.push(LoserEdge {
            other_table,
            other_id,
            rel_type: row.rel_type,
            notes: row.notes,
            loser_is_in,
        });
    }

    if edges.is_empty() {
        return Ok(edges);
    }

    // Drop edges to neighbours that are themselves already soft-deleted —
    // matches `get_entity_relations`'s behaviour, grouping ids per table.
    let mut by_table: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for e in &edges {
        by_table
            .entry(e.other_table.clone())
            .or_default()
            .push(e.other_id.clone());
    }
    let mut live: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for (table, ids) in &by_table {
        let things: Vec<Thing> = ids
            .iter()
            .map(|i| Thing::from((table.as_str(), i.as_str())))
            .collect();
        let mut r = db
            .query("SELECT id FROM type::table($table) WHERE id IN $ids AND vault_deleted != true")
            .bind(("table", table.clone()))
            .bind(("ids", things))
            .await
            .map_err(db_err)?;
        #[derive(Deserialize)]
        struct IdRow {
            id: Thing,
        }
        let found: Vec<IdRow> = r.take(0).map_err(db_err)?;
        for row in found {
            live.insert((row.id.tb.clone(), row.id.id.to_raw()));
        }
    }
    edges.retain(|e| live.contains(&(e.other_table.clone(), e.other_id.clone())));

    Ok(edges)
}

/// Split a full record id (`faction:abc`) into its table and id fragments.
fn split_full_id(full: &str) -> Result<(&str, &str), EntityError> {
    full.split_once(':').ok_or_else(|| EntityError::Validation {
        field: "id".to_string(),
        message: format!("Malformed record id: {full}"),
    })
}

/// True when `id` is a safe SurrealDB record-id fragment (alphanumeric plus
/// `_`/`-`). Ids reach a `format!`'d `DELETE` below, so they are validated first
/// — mirrors `relations::edge::is_safe_record_id`, which is private to that
/// module.
fn is_safe_record_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Fold `loser` into `survivor`.
///
/// CRASH SAFETY. There is no transaction here: the codebase uses none, and
/// merge also does non-DB work (an embedding call in the command layer) that no
/// DB transaction could cover. So the ORDER is the safety property — edges
/// first, soft-delete LAST. Every step before the delete is idempotent and
/// re-runnable, so a crash mid-merge leaves both records alive with a SUPERSET
/// of edges: visibly unfinished, safe, and re-runnable. Deleting first would
/// orphan edges permanently.
///
/// Rejections: `survivor == loser`, a malformed id, a missing record, or a
/// cross-kind pair (an npc cannot merge into a location).
///
/// `survivor` and `loser` are full record ids (`faction:abc`). The survivor's
/// name/summary/notes may change, so the caller must re-embed it afterwards.
pub async fn merge<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    survivor: &str,
    loser: &str,
    choices: MergeChoices,
) -> Result<(), EntityError> {
    if survivor == loser {
        return Err(EntityError::Validation {
            field: "loser".to_string(),
            message: "Cannot merge a record into itself".to_string(),
        });
    }

    let (s_table, s_id) = split_full_id(survivor)?;
    let (l_table, l_id) = split_full_id(loser)?;

    // Same table or bust: a relationship graph keyed by kind cannot re-home an
    // npc's edges onto a location.
    if s_table != l_table {
        return Err(EntityError::Validation {
            field: "loser".to_string(),
            message: "Cannot merge entities of different kinds".to_string(),
        });
    }
    if !is_safe_record_id(s_id) || !is_safe_record_id(l_id) {
        return Err(EntityError::Validation {
            field: "id".to_string(),
            message: "Invalid entity id".to_string(),
        });
    }

    let kind = EntityKind::from_table(s_table)?;
    // Both must exist — `get_by_id` filters soft-deleted rows and returns
    // `NotFound` otherwise.
    let survivor_node = get_by_id(db, s_id, kind.clone()).await?;
    let loser_node = get_by_id(db, l_id, kind.clone()).await?;

    // 1. Re-point every edge of the loser onto the survivor, both directions.
    //
    //    REUSE `relate_collapsing` — do NOT hand-roll an UPDATE of `in`/`out`.
    //    It already knows the specificity TIERS: a generic `mentioned`/
    //    `related_to` edge must not pile up on top of a specific `allied_with`
    //    between the same pair, and a raw re-point would create exactly that
    //    pile. It also scope-checks both ends.
    //
    //    This means the survivor can end with FEWER edge ROWS than the two
    //    inputs had while losing no INFORMATION — a redundant generic edge
    //    collapses into the specific one that already says more.
    for e in loser_edges(db, l_id, l_table).await? {
        // An edge between loser and survivor themselves becomes a meaningless
        // self-loop after the merge — drop it (the DELETE below removes the row).
        if e.other_id == s_id && e.other_table == s_table {
            continue;
        }
        // Carry the edge's authored `notes` forward — a GM annotation like
        // "betrayed the party in session 4" is exactly the kind of hand-written
        // fact merge promises not to lose.
        if e.loser_is_in {
            // Loser was the `in` side (loser → other): survivor → other.
            relate_collapsing(
                db,
                s_id,
                s_table,
                &e.other_id,
                &e.other_table,
                &e.rel_type,
                e.notes,
            )
            .await?;
        } else {
            // Loser was the `out` side (other → loser): other → survivor.
            relate_collapsing(
                db,
                &e.other_id,
                &e.other_table,
                s_id,
                s_table,
                &e.rel_type,
                e.notes,
            )
            .await?;
        }
    }
    // The loser's own edges are now redundant; remove them so no row dangles off
    // a soon-to-be-hidden record. Safe only because the survivor already carries
    // a superset of the information.
    db.query(format!(
        "DELETE relates_to WHERE in = {l_table}:{l_id} OR out = {l_table}:{l_id}"
    ))
    .await
    .map_err(db_err)?
    .check()
    .map_err(db_err)?;

    // 2. Aliases: union of both sides PLUS the loser's NAME — this is what keeps
    //    every `[[Free League]]` the GM ever wrote resolving after the merge.
    //    Deduplicated case-insensitively.
    let mut aliases = survivor_node.aliases.clone();
    for a in loser_node
        .aliases
        .iter()
        .chain(std::iter::once(&loser_node.name))
    {
        if !aliases.iter().any(|x| x.eq_ignore_ascii_case(a)) {
            aliases.push(a.clone());
        }
    }

    // 3. Per-field choices. `KeepBoth` concatenates; nothing is silently lost.
    let summary = choose(
        &choices.summary,
        survivor_node.summary.as_deref(),
        loser_node.summary.as_deref(),
        &loser_node.name,
    );
    let notes = choose(
        &choices.notes,
        survivor_node.notes.as_deref(),
        loser_node.notes.as_deref(),
        &loser_node.name,
    );

    // 4. Write the survivor, marking the article stale: it was compiled from
    //    half the facts and must be recompiled. We do NOT textually merge the
    //    two articles — that would produce prose no compiler wrote.
    db.query(format!(
        "UPDATE type::thing('{s_table}', $id) SET aliases = $aliases, summary = $summary, \
         notes = $notes, codex_stale = true, updated_at = time::now()"
    ))
    .bind(("id", s_id.to_owned()))
    .bind(("aliases", aliases))
    .bind(("summary", opt_value(summary)))
    .bind(("notes", opt_value(notes)))
    .await
    .map_err(db_err)?
    .check()
    .map_err(db_err)?;

    // 5. Soft-delete the loser LAST. Its vault file goes through the normal
    //    reconcile sweep — never a raw DELETE, never a vault move.
    soft_delete(db, l_id, kind).await?;

    // 6. Resolve the `duplicate_entity` finding that flagged this pair — the
    //    Maintenance inbox should no longer surface it. The linter stores the
    //    pair sorted, as full record ids, so match either ordering.
    db.query(
        "UPDATE lint_finding SET resolved_at = time::now() \
         WHERE kind = 'duplicate_entity' AND resolved_at = NONE \
         AND ((payload.a = $s AND payload.b = $l) OR (payload.a = $l AND payload.b = $s))",
    )
    .bind(("s", survivor.to_owned()))
    .bind(("l", loser.to_owned()))
    .await
    .map_err(db_err)?
    .check()
    .map_err(db_err)?;

    Ok(())
}

fn db_err(e: surrealdb::Error) -> EntityError {
    EntityError::Database {
        message: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_both_concatenates_under_a_heading_naming_the_loser() {
        let out = choose(
            &FieldChoice::KeepBoth,
            Some("Survivor text."),
            Some("Loser text."),
            "Free League",
        );
        let out = out.unwrap();
        assert!(out.contains("Survivor text."));
        assert!(out.contains("## Merged from Free League"));
        assert!(out.contains("Loser text."));
    }

    #[test]
    fn keep_both_with_one_empty_side_does_not_add_a_heading() {
        assert_eq!(
            choose(&FieldChoice::KeepBoth, Some("only survivor"), None, "L"),
            Some("only survivor".to_string())
        );
        assert_eq!(
            choose(&FieldChoice::KeepBoth, None, Some("only loser"), "L"),
            Some("only loser".to_string())
        );
        assert_eq!(choose(&FieldChoice::KeepBoth, None, None, "L"), None);
    }

    #[test]
    fn keep_survivor_and_keep_loser_pick_one_side_wholesale() {
        assert_eq!(
            choose(&FieldChoice::KeepSurvivor, Some("s"), Some("l"), "L"),
            Some("s".to_string())
        );
        assert_eq!(
            choose(&FieldChoice::KeepLoser, Some("s"), Some("l"), "L"),
            Some("l".to_string())
        );
    }

    #[test]
    fn merge_choices_deserializes_from_camel_case_ipc_payload() {
        let choices: MergeChoices =
            serde_json::from_str(r#"{"summary":"keepSurvivor","notes":"keepBoth"}"#)
                .expect("camelCase IPC payload must deserialize");
        assert!(matches!(choices.summary, FieldChoice::KeepSurvivor));
        assert!(matches!(choices.notes, FieldChoice::KeepBoth));
    }
}
