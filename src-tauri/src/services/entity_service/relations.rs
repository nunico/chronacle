//! Relationship edges and graph queries: creating `relates_to` edges (with the
//! specificity-tier collapsing rule), the ego graph, and the flat relations list.

use serde::Deserialize;
use surrealdb::sql::Thing;

use super::{EntityError, EntityGraph, GraphEdge, GraphNodeRef, RelatedEntity};

/// True when `id` is a safe SurrealDB record-id fragment (alphanumeric plus
/// `_`/`-`). Record ids are interpolated into query strings in a few places
/// because `type::thing()` is unreliable on edge endpoints in this SurrealDB
/// version, so any id that reaches those `format!` sites must be validated first.
fn is_safe_record_id(id: &str) -> bool {
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
fn rel_specificity(rel_type: &str) -> u8 {
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
fn keep_most_specific<T, K, I>(
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

/// Fetch the ego graph around an entity: the center, its `relates_to` neighbors
/// (one hop), and the edges among them. `_depth` is reserved for future use;
/// the graph is currently always one hop, with deeper walks produced client-side
/// by re-calling on a neighbor.
pub async fn get_entity_graph<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: &str,
    _depth: u32,
) -> Result<EntityGraph, EntityError> {
    if !is_safe_record_id(id) {
        return Err(EntityError::Validation {
            field: "id".to_string(),
            message: "Invalid entity id".to_string(),
        });
    }

    #[derive(Deserialize)]
    struct EdgeRow {
        #[serde(rename = "in")]
        in_: Thing,
        out: Thing,
        rel_type: String,
        notes: Option<String>,
    }

    // 1. Edges touching the center in both directions. Build the center Thing
    //    directly in the query string — type::thing() in WHERE on edge endpoints
    //    is unreliable on some SurrealDB versions.
    let edge_sql = format!(
        "SELECT in, out, rel_type, notes FROM relates_to \
         WHERE in = {kind}:{id} OR out = {kind}:{id}"
    );
    let mut resp = db
        .query(edge_sql)
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let rows: Vec<EdgeRow> = resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;

    let edges: Vec<GraphEdge> = rows
        .iter()
        .map(|r| GraphEdge {
            from_id: r.in_.id.to_raw(),
            from_kind: r.in_.tb.clone(),
            to_id: r.out.id.to_raw(),
            to_kind: r.out.tb.clone(),
            rel_type: r.rel_type.clone(),
            notes: r.notes.clone(),
        })
        .collect();

    // Collapse parallel edges between the same pair, keeping only the most
    // specific relationship(s). Drops redundant `mentioned`/`related_to` edges
    // when a specific relationship exists for that pair.
    let edges = keep_most_specific(
        edges,
        |e| {
            // Unordered pair key: sort the two endpoints so A→B and B→A group together.
            let a = format!("{}:{}", e.from_kind, e.from_id);
            let b = format!("{}:{}", e.to_kind, e.to_id);
            if a <= b {
                (a, b)
            } else {
                (b, a)
            }
        },
        |e| {
            (
                e.from_kind.clone(),
                e.from_id.clone(),
                e.to_kind.clone(),
                e.to_id.clone(),
                e.rel_type.clone(),
            )
        },
        |e| e.rel_type.as_str(),
    );

    // 2. Collect distinct (kind, id) node keys: the center plus every endpoint
    //    of the surviving (collapsed) edges.
    let mut keys: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
    keys.insert((kind.to_string(), id.to_string()));
    for e in &edges {
        keys.insert((e.from_kind.clone(), e.from_id.clone()));
        keys.insert((e.to_kind.clone(), e.to_id.clone()));
    }

    // 3. Resolve names. Group ids by table and query each table once.
    #[derive(Deserialize)]
    struct NameRow {
        id: Thing,
        name: String,
    }

    let mut by_table: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (k, i) in &keys {
        by_table.entry(k.clone()).or_default().push(i.clone());
    }

    let mut nodes: Vec<GraphNodeRef> = Vec::new();
    for (table, ids) in by_table {
        // Build the id list as `Thing`s in Rust and bind as an array — robust
        // across SurrealDB versions, unlike type::thing() inside the query.
        let things: Vec<Thing> = ids
            .iter()
            .map(|i| Thing::from((table.as_str(), i.as_str())))
            .collect();
        let mut r = db
            .query("SELECT id, name FROM type::table($table) WHERE id IN $ids")
            .bind(("table", table.clone()))
            .bind(("ids", things))
            .await
            .map_err(|e| EntityError::Database {
                message: e.to_string(),
            })?;
        let found: Vec<NameRow> = r.take(0).map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
        for nr in found {
            nodes.push(GraphNodeRef {
                id: nr.id.id.to_raw(),
                kind: nr.id.tb.clone(),
                name: nr.name,
            });
        }
    }

    Ok(EntityGraph { nodes, edges })
}

/// Fetch all entities related to `id:kind` as a flat list, both directions.
///
/// - Outbound: edges where center is `in` (center → other).
/// - Inbound: edges where center is `out` (other → center).
/// - Self-loops are excluded.
/// - Results are sorted by name for a stable, deterministic order.
pub async fn get_entity_relations<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: &str,
) -> Result<Vec<RelatedEntity>, EntityError> {
    if !is_safe_record_id(id) {
        return Err(EntityError::Validation {
            field: "id".to_string(),
            message: "Invalid entity id".to_string(),
        });
    }

    #[derive(Deserialize)]
    struct EdgeRow {
        #[serde(rename = "in")]
        in_: Thing,
        out: Thing,
        rel_type: String,
    }

    // Fetch all edges touching the center in either direction.
    let edge_sql = format!(
        "SELECT in, out, rel_type FROM relates_to \
         WHERE in = {kind}:{id} OR out = {kind}:{id}"
    );
    let mut resp = db
        .query(edge_sql)
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let rows: Vec<EdgeRow> = resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;

    if rows.is_empty() {
        return Ok(vec![]);
    }

    // Collect (other_table, other_id, rel_type, direction), skipping self-loops.
    struct OtherEndpoint {
        table: String,
        other_id: String,
        rel_type: String,
        direction: String,
    }

    let mut endpoints: Vec<OtherEndpoint> = Vec::new();
    for row in &rows {
        let in_id = row.in_.id.to_raw();
        let in_tb = row.in_.tb.clone();
        let out_id = row.out.id.to_raw();
        let out_tb = row.out.tb.clone();

        if in_id == id && in_tb == kind {
            // Center is `in` → outbound edge, other end is `out`.
            if out_id == id && out_tb == kind {
                continue; // self-loop
            }
            endpoints.push(OtherEndpoint {
                table: out_tb,
                other_id: out_id,
                rel_type: row.rel_type.clone(),
                direction: "outbound".to_string(),
            });
        } else {
            // Center is `out` → inbound edge, other end is `in`. A true
            // self-loop (in == out == center) is impossible here — it is caught
            // by the first branch, since `in == kind:id` would be true.
            debug_assert!(
                !(in_id == id && in_tb == kind),
                "self-loop should have been handled by the outbound branch"
            );
            endpoints.push(OtherEndpoint {
                table: in_tb,
                other_id: in_id,
                rel_type: row.rel_type.clone(),
                direction: "inbound".to_string(),
            });
        }
    }

    // Collapse parallel relationships to the same entity: keep only the most
    // specific rel_type(s), dropping redundant `mentioned`/`related_to` when a
    // specific relationship to that same entity exists (mirrors get_entity_graph).
    let endpoints = keep_most_specific(
        endpoints,
        |ep| (ep.table.clone(), ep.other_id.clone()),
        |ep| {
            (
                ep.table.clone(),
                ep.other_id.clone(),
                ep.rel_type.clone(),
                ep.direction.clone(),
            )
        },
        |ep| ep.rel_type.as_str(),
    );

    // Resolve names by grouping ids per table (mirrors get_entity_graph).
    #[derive(Deserialize)]
    struct NameRow {
        id: Thing,
        name: String,
    }

    let mut by_table: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for ep in &endpoints {
        by_table
            .entry(ep.table.clone())
            .or_default()
            .push(ep.other_id.clone());
    }

    let mut name_map: std::collections::HashMap<(String, String), String> =
        std::collections::HashMap::new();
    for (table, ids) in &by_table {
        let things: Vec<Thing> = ids
            .iter()
            .map(|i| Thing::from((table.as_str(), i.as_str())))
            .collect();
        let mut r = db
            .query("SELECT id, name FROM type::table($table) WHERE id IN $ids")
            .bind(("table", table.clone()))
            .bind(("ids", things))
            .await
            .map_err(|e| EntityError::Database {
                message: e.to_string(),
            })?;
        let found: Vec<NameRow> = r.take(0).map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
        for nr in found {
            name_map.insert((nr.id.tb.clone(), nr.id.id.to_raw()), nr.name);
        }
    }

    let mut related: Vec<RelatedEntity> = endpoints
        .into_iter()
        .filter_map(|ep| {
            let name = name_map
                .get(&(ep.table.clone(), ep.other_id.clone()))
                .cloned()?;
            Some(RelatedEntity {
                id: ep.other_id,
                kind: ep.table,
                name,
                rel_type: ep.rel_type,
                direction: ep.direction,
            })
        })
        .collect();

    // Sort by name for deterministic ordering.
    related.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(related)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::entity_service::{create, EntityInput, EntityKind};

    #[test]
    fn rel_specificity_tiers_match_vocab() {
        assert_eq!(rel_specificity("mentioned"), 0);
        assert_eq!(rel_specificity("related_to"), 1);
        assert_eq!(rel_specificity("knows"), 1);
        // Specific directional types and unknown custom verbs are all tier 2.
        assert_eq!(rel_specificity("member_of"), 2);
        assert_eq!(rel_specificity("located_in"), 2);
        assert_eq!(rel_specificity("enemy_of"), 2);
        assert_eq!(rel_specificity("betrays"), 2);
    }

    fn edge(from: &str, to: &str, rel: &str) -> GraphEdge {
        let (ft, fi) = from.split_once(':').unwrap();
        let (tt, ti) = to.split_once(':').unwrap();
        GraphEdge {
            from_id: fi.to_string(),
            from_kind: ft.to_string(),
            to_id: ti.to_string(),
            to_kind: tt.to_string(),
            rel_type: rel.to_string(),
            notes: None,
        }
    }

    fn pair_key(e: &GraphEdge) -> (String, String) {
        let a = format!("{}:{}", e.from_kind, e.from_id);
        let b = format!("{}:{}", e.to_kind, e.to_id);
        if a <= b {
            (a, b)
        } else {
            (b, a)
        }
    }

    fn identity(e: &GraphEdge) -> (String, String, String, String, String) {
        (
            e.from_kind.clone(),
            e.from_id.clone(),
            e.to_kind.clone(),
            e.to_id.clone(),
            e.rel_type.clone(),
        )
    }

    fn collapse(edges: Vec<GraphEdge>) -> Vec<GraphEdge> {
        keep_most_specific(edges, pair_key, identity, |e| e.rel_type.as_str())
    }

    #[test]
    fn collapse_drops_generic_when_specific_exists_for_pair() {
        // Hegemony located_in Spire AND related_to Spire → keep only located_in.
        let kept = collapse(vec![
            edge("faction:heg", "location:spire", "located_in"),
            edge("faction:heg", "location:spire", "related_to"),
        ]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].rel_type, "located_in");
    }

    #[test]
    fn collapse_drops_mentioned_against_specific_regardless_of_direction() {
        // member_of one way, mentioned the other way, same pair → keep member_of.
        let kept = collapse(vec![
            edge("faction:heg", "faction:other", "member_of"),
            edge("faction:other", "faction:heg", "mentioned"),
        ]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].rel_type, "member_of");
    }

    #[test]
    fn collapse_keeps_mentioned_when_it_is_the_only_edge() {
        let kept = collapse(vec![edge("faction:heg", "npc:bob", "mentioned")]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].rel_type, "mentioned");
    }

    #[test]
    fn collapse_keeps_distinct_specific_types_for_same_pair() {
        // Two contradictory-but-specific edges both survive (same tier).
        let kept = collapse(vec![
            edge("faction:a", "faction:b", "allied_with"),
            edge("faction:a", "faction:b", "enemy_of"),
        ]);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn collapse_dedupes_identical_edges() {
        let kept = collapse(vec![
            edge("faction:a", "faction:b", "member_of"),
            edge("faction:a", "faction:b", "member_of"),
        ]);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn collapse_does_not_mix_unrelated_pairs() {
        // related_to to one entity stays when there is no specific edge to it,
        // even though a specific edge exists to a different entity.
        let kept = collapse(vec![
            edge("faction:heg", "location:spire", "located_in"),
            edge("faction:heg", "npc:bob", "related_to"),
        ]);
        assert_eq!(kept.len(), 2);
    }

    /// Create a campaign + two factions and return their ids.
    async fn setup_pair<C: surrealdb::Connection>(db: &surrealdb::Surreal<C>) -> (String, String) {
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        let a = create(
            db,
            Some("camp1"),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "Hegemony".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let b = create(
            db,
            Some("camp1"),
            None,
            EntityKind::Faction,
            EntityInput {
                name: "Syndicate".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        (a.id, b.id)
    }

    /// Return the rel_types of every `relates_to` edge between the two factions
    /// (either direction), sorted.
    async fn rel_types_between<C: surrealdb::Connection>(
        db: &surrealdb::Surreal<C>,
        a: &str,
        b: &str,
    ) -> Vec<String> {
        #[derive(Deserialize)]
        struct Row {
            rel_type: String,
        }
        let sql = format!(
            "SELECT rel_type FROM relates_to WHERE \
             (in = faction:{a} AND out = faction:{b}) OR \
             (in = faction:{b} AND out = faction:{a})"
        );
        let mut resp = db.query(sql).await.unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        let mut types: Vec<String> = rows.into_iter().map(|r| r.rel_type).collect();
        types.sort();
        types
    }

    #[tokio::test]
    async fn relate_collapsing_specific_removes_existing_mentioned() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        let (a, b) = setup_pair(&db).await;
        // A pre-existing `mentioned` edge (as a wikilink would create).
        relate(&db, &a, "faction", &b, "faction", "mentioned", None)
            .await
            .unwrap();
        // A specific relationship supersedes it.
        let created = relate_collapsing(&db, &a, "faction", &b, "faction", "member_of", None)
            .await
            .unwrap();
        assert!(created, "specific edge should be created");
        assert_eq!(rel_types_between(&db, &a, &b).await, vec!["member_of"]);
    }

    #[tokio::test]
    async fn relate_collapsing_generic_skipped_when_specific_exists() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        let (a, b) = setup_pair(&db).await;
        relate_collapsing(&db, &a, "faction", &b, "faction", "member_of", None)
            .await
            .unwrap();
        // related_to is generic (tier 1) and must not be added over member_of (tier 2).
        let created = relate_collapsing(&db, &a, "faction", &b, "faction", "related_to", None)
            .await
            .unwrap();
        assert!(!created, "generic edge should be skipped");
        assert_eq!(rel_types_between(&db, &a, &b).await, vec!["member_of"]);
    }

    #[tokio::test]
    async fn relate_collapsing_drops_lower_tier_even_in_opposite_direction() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        let (a, b) = setup_pair(&db).await;
        // mentioned B→A, then specific A→B: the unordered pair collapses to the specific.
        relate(&db, &b, "faction", &a, "faction", "mentioned", None)
            .await
            .unwrap();
        relate_collapsing(&db, &a, "faction", &b, "faction", "enemy_of", None)
            .await
            .unwrap();
        assert_eq!(rel_types_between(&db, &a, &b).await, vec!["enemy_of"]);
    }

    #[tokio::test]
    async fn relate_collapsing_keeps_lone_mentioned_and_coexisting_specifics() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        let (a, b) = setup_pair(&db).await;
        // A lone mentioned survives (it is the only connection).
        let created = relate_collapsing(&db, &a, "faction", &b, "faction", "mentioned", None)
            .await
            .unwrap();
        assert!(created);
        // Two same-tier specifics coexist; the mentioned is dropped.
        relate_collapsing(&db, &a, "faction", &b, "faction", "allied_with", None)
            .await
            .unwrap();
        relate_collapsing(&db, &a, "faction", &b, "faction", "enemy_of", None)
            .await
            .unwrap();
        assert_eq!(
            rel_types_between(&db, &a, &b).await,
            vec!["allied_with", "enemy_of"]
        );
    }
}
