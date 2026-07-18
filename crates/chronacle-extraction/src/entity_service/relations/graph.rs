use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use surrealdb::sql::Thing;

use super::super::{EntityError, EntityGraph, GraphEdge, GraphNodeRef, SELECT_SCOPE_ALIASES};
use super::edge::{is_safe_record_id, keep_most_specific};
use crate::naming;
use crate::wikilink::{query_all_entity_names, resolve_exact, WikilinkScope};

static WIKILINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").expect("wikilink regex is valid"));

#[derive(Deserialize)]
struct GraphTextRow {
    id: Thing,
    name: String,
    notes: Option<String>,
    codex_article: Option<String>,
    campaign: Option<Thing>,
    collection: Option<Thing>,
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
    let mut edges = keep_most_specific(
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
    let mut keys: BTreeSet<(String, String)> = BTreeSet::new();
    keys.insert((kind.to_string(), id.to_string()));
    for e in &edges {
        keys.insert((e.from_kind.clone(), e.from_id.clone()));
        keys.insert((e.to_kind.clone(), e.to_id.clone()));
    }

    // 3. Resolve names/text/scope. Group ids by table and query each table once.
    let mut by_table: HashMap<String, Vec<String>> = HashMap::new();
    for (k, i) in &keys {
        by_table.entry(k.clone()).or_default().push(i.clone());
    }

    let mut nodes: Vec<GraphNodeRef> = Vec::new();
    let mut text_rows: Vec<GraphTextRow> = Vec::new();
    for (table, ids) in by_table {
        // Build the id list as `Thing`s in Rust and bind as an array — robust
        // across SurrealDB versions, unlike type::thing() inside the query.
        let things: Vec<Thing> = ids
            .iter()
            .map(|i| Thing::from((table.as_str(), i.as_str())))
            .collect();
        // `vault_deleted != true`, never `= false`: a soft-deleted entity must
        // not appear as a node in the ego graph even when a live entity still
        // has an edge to it.
        let query = format!(
            "SELECT id, name, notes, codex_article, {SELECT_SCOPE_ALIASES} \
             FROM type::table($table) WHERE id IN $ids AND vault_deleted != true"
        );
        let mut r = db
            .query(query)
            .bind(("table", table.clone()))
            .bind(("ids", things))
            .await
            .map_err(|e| EntityError::Database {
                message: e.to_string(),
            })?;
        let found: Vec<GraphTextRow> = r.take(0).map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
        for nr in found {
            nodes.push(GraphNodeRef {
                id: nr.id.id.to_raw(),
                kind: nr.id.tb.clone(),
                name: nr.name.clone(),
                missing: None,
                source_id: None,
                source_kind: None,
            });
            text_rows.push(nr);
        }
    }

    append_missing_wikilinks(db, &text_rows, &mut nodes, &mut edges).await?;

    Ok(EntityGraph { nodes, edges })
}

async fn append_missing_wikilinks<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    rows: &[GraphTextRow],
    nodes: &mut Vec<GraphNodeRef>,
    edges: &mut Vec<GraphEdge>,
) -> Result<(), EntityError> {
    let mut seen_missing: BTreeSet<String> = BTreeSet::new();

    for row in rows {
        let entity_id = row.id.id.to_raw();
        let scope_collection = row.collection.as_ref().map(|t| t.id.to_raw());
        let scope_campaign = row.campaign.as_ref().map(|t| t.id.to_raw());
        let scope = if let Some(collection_id) = scope_collection.as_deref() {
            WikilinkScope::Collection { collection_id }
        } else if let Some(campaign_id) = scope_campaign.as_deref() {
            WikilinkScope::Campaign { campaign_id }
        } else {
            continue;
        };
        let names =
            query_all_entity_names(db, &scope)
                .await
                .map_err(|e| EntityError::Database {
                    message: e.to_string(),
                })?;

        for link_text in extracted_links(&row.notes, &row.codex_article) {
            if resolve_exact(&link_text, &names).is_some() {
                continue;
            }
            let missing_key = naming::normalize(&link_text);
            if missing_key.is_empty() {
                continue;
            }
            let missing_id = format!("missing_wikilink:{}:{}:{missing_key}", row.id.tb, entity_id);
            if !seen_missing.insert(missing_id.clone()) {
                continue;
            }
            nodes.push(GraphNodeRef {
                id: missing_id.clone(),
                kind: "missing_wikilink".to_string(),
                name: link_text,
                missing: Some(true),
                source_id: Some(entity_id.clone()),
                source_kind: Some(row.id.tb.clone()),
            });
            edges.push(GraphEdge {
                from_id: entity_id.clone(),
                from_kind: row.id.tb.clone(),
                to_id: missing_id,
                to_kind: "missing_wikilink".to_string(),
                rel_type: "unresolved".to_string(),
                notes: None,
            });
        }
    }

    Ok(())
}

fn extracted_links(notes: &Option<String>, codex_article: &Option<String>) -> Vec<String> {
    notes
        .iter()
        .chain(codex_article.iter())
        .flat_map(|text| {
            WIKILINK_RE
                .captures_iter(text)
                .map(|cap| cap[1].trim().to_string())
                .filter(|link| !link.is_empty())
        })
        .collect()
}
