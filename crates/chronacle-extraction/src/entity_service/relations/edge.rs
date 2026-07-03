use serde::Deserialize;

use super::super::EntityError;
use super::scope;

/// True when `id` is a safe SurrealDB record-id fragment (alphanumeric plus
/// `_`/`-`). Record ids are interpolated into query strings in a few places
/// because `type::thing()` is unreliable on edge endpoints in this SurrealDB
/// version, so any id that reaches those `format!` sites must be validated first.
pub(super) fn is_safe_record_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Create a directed graph edge between two nodes.
///
/// `from_kind` and `to_kind` are the table names of the source and target nodes.
pub async fn relate<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    from_id: &str,
    from_kind: &str,
    to_id: &str,
    to_kind: &str,
    rel_type: &str,
    notes: Option<String>,
) -> Result<(), EntityError> {
    if !is_safe_record_id(from_id) {
        return Err(EntityError::Validation {
            field: "from_id".to_string(),
            message: "Invalid entity id".to_string(),
        });
    }
    if !is_safe_record_id(to_id) {
        return Err(EntityError::Validation {
            field: "to_id".to_string(),
            message: "Invalid entity id".to_string(),
        });
    }
    scope::check_scope(db, from_kind, from_id, to_kind, to_id).await?;
    // Delete any pre-existing edge for this (from, to, rel_type) triple so that
    // RELATE does not create a duplicate on repeated calls. Mirrors the
    // delete-then-relate pattern in `wikilink::upsert_mentioned_edges`.
    let delete_query = format!(
        "DELETE relates_to WHERE in = {from_kind}:{from_id} AND out = {to_kind}:{to_id} AND rel_type = $rel_type"
    );
    db.query(delete_query)
        .bind(("rel_type", rel_type.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;

    // Build record IDs directly in the query string because some SurrealDB versions
    // do not allow type::thing() on the left/right side of RELATE arrows.
    let query = format!(
        "RELATE {}:{}->relates_to->{}:{} SET rel_type = $rel_type, notes = $notes, created_at = time::now()",
        from_kind, from_id, to_kind, to_id
    );
    db.query(query)
        .bind(("rel_type", rel_type.to_owned()))
        .bind(("notes", notes))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    Ok(())
}

/// Create a `relates_to` edge while enforcing the relationship-tier rule, so
/// only the most specific relationship(s) survive between any unordered pair of
/// entities (see [`rel_specificity`]).
///
/// - If a strictly higher-tier edge already exists between the pair, the new
///   edge is redundant and is skipped — returns `Ok(false)`.
/// - Otherwise any existing strictly-lower-tier edges between the pair (in
///   either direction) are deleted, the edge is created, and it returns
///   `Ok(true)`.
/// - Same-tier edges coexist (e.g. `allied_with` + `enemy_of`); an exact
///   duplicate is replaced in place by the underlying [`relate`].
///
/// This is how `mentioned` (tier 0, from wikilinks) and `related_to`/`knows`
/// (tier 1, the LLM's catch-alls) are prevented from piling up on top of a
/// specific relationship at write time, rather than being filtered at read time.
pub async fn relate_collapsing<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    from_id: &str,
    from_kind: &str,
    to_id: &str,
    to_kind: &str,
    rel_type: &str,
    notes: Option<String>,
) -> Result<bool, EntityError> {
    if !is_safe_record_id(from_id) {
        return Err(EntityError::Validation {
            field: "from_id".to_string(),
            message: "Invalid entity id".to_string(),
        });
    }
    if !is_safe_record_id(to_id) {
        return Err(EntityError::Validation {
            field: "to_id".to_string(),
            message: "Invalid entity id".to_string(),
        });
    }

    scope::check_scope(db, from_kind, from_id, to_kind, to_id).await?;

    let new_tier = rel_specificity(rel_type);

    // Existing edge rel_types between the unordered pair (both directions).
    #[derive(Deserialize)]
    struct RelRow {
        rel_type: String,
    }
    let pair_filter = format!(
        "(in = {from_kind}:{from_id} AND out = {to_kind}:{to_id}) OR \
         (in = {to_kind}:{to_id} AND out = {from_kind}:{from_id})"
    );
    let mut resp = db
        .query(format!(
            "SELECT rel_type FROM relates_to WHERE {pair_filter}"
        ))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let rows: Vec<RelRow> = resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    let max_existing = rows
        .iter()
        .map(|r| rel_specificity(&r.rel_type))
        .max()
        .unwrap_or(0);

    // A more specific relationship already describes this pair — skip the weaker edge.
    if new_tier < max_existing {
        return Ok(false);
    }

    // Drop now-redundant lower-tier edges for this pair (both directions).
    let lower_types: Vec<String> = ["mentioned", "related_to", "knows"]
        .into_iter()
        .filter(|t| rel_specificity(t) < new_tier)
        .map(String::from)
        .collect();
    if !lower_types.is_empty() {
        db.query(format!(
            "DELETE relates_to WHERE ({pair_filter}) AND rel_type IN $lower"
        ))
        .bind(("lower", lower_types))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    }

    relate(db, from_id, from_kind, to_id, to_kind, rel_type, notes).await?;
    Ok(true)
}

/// Specificity tier of a relationship type, used to collapse redundant parallel
/// edges between the same pair of entities. Higher = more informative.
///
/// Two extraction sources stack weak edges on top of real relationships:
/// - `mentioned` (tier 0) comes from wikilink resolution — entity B's name
///   merely appeared in entity A's notes. It carries no meaning beyond
///   co-occurrence.
/// - `related_to` / `knows` (tier 1) are the catch-all associations the LLM
///   emits when it cannot commit to a specific relationship.
/// - everything else (tier 2) is a specific, usually directional relationship
///   (`member_of`, `located_in`, `owns`, `leads`, …) or a custom verb.
///
/// When a higher tier exists between a pair, the lower tiers are dropped: a
/// `member_of` edge makes a parallel `mentioned` edge redundant noise.
pub(super) fn rel_specificity(rel_type: &str) -> u8 {
    match rel_type {
        "mentioned" => 0,
        "related_to" | "knows" => 1,
        _ => 2,
    }
}

/// Collapse parallel relationships: among all `items` sharing a `group_key`,
/// keep only those whose `rel_type` is at the highest specificity tier present
/// in that group, and drop exact duplicates (same `identity`). Order-preserving.
///
/// Shared by the graph (grouping by unordered entity pair) and the flat
/// relations list (grouping by the other endpoint).
pub(super) fn keep_most_specific<T, K, I>(
    items: Vec<T>,
    group_key: impl Fn(&T) -> K,
    identity: impl Fn(&T) -> I,
    rel_type: impl Fn(&T) -> &str,
) -> Vec<T>
where
    K: std::hash::Hash + Eq,
    I: std::hash::Hash + Eq,
{
    use std::collections::{HashMap, HashSet};

    let mut max_tier: HashMap<K, u8> = HashMap::new();
    for it in &items {
        let tier = rel_specificity(rel_type(it));
        let entry = max_tier.entry(group_key(it)).or_insert(0);
        if tier > *entry {
            *entry = tier;
        }
    }

    let mut seen: HashSet<I> = HashSet::new();
    items
        .into_iter()
        .filter(|it| rel_specificity(rel_type(it)) == max_tier[&group_key(it)])
        .filter(|it| seen.insert(identity(it)))
        .collect()
}
