use serde::Deserialize;
use surrealdb::sql::Thing;

use super::super::{EntityError, EntityGraph, GraphEdge, GraphNodeRef};
use super::edge::{is_safe_record_id, keep_most_specific};

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
