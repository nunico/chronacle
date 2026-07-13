use serde::Deserialize;
use surrealdb::sql::Thing;

use super::super::{EntityError, RelatedEntity};
use super::edge::{is_safe_record_id, keep_most_specific};

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
        // `vault_deleted != true`, never `= false`: a soft-deleted entity must
        // not appear in the flat relations list even when a live entity still
        // has an edge to it.
        let mut r = db
            .query("SELECT id, name FROM type::table($table) WHERE id IN $ids AND vault_deleted != true")
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
