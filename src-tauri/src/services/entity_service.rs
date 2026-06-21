use std::sync::Arc;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use thiserror::Error;

use crate::providers::embedding::EmbeddingProvider;

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "code")]
pub enum EntityError {
    #[error("Entity '{id}' not found")]
    NotFound { id: String },
    #[error("Entity does not belong to the specified campaign")]
    CampaignMismatch,
    #[error("Collection entity cannot link to a campaign entity")]
    CrossLinkViolation,
    #[error("Unknown entity kind: '{kind}'")]
    InvalidKind { kind: String },
    #[error("Validation error on field '{field}': {message}")]
    Validation { field: String, message: String },
    #[error("Database error: {message}")]
    Database { message: String },
}

// ── Entity kind ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Npc,
    Location,
    Faction,
    Creature,
    Item,
    Event,
    PlayerCharacter,
    Misc,
}

impl EntityKind {
    pub fn table_name(&self) -> &'static str {
        match self {
            Self::Npc => "npc",
            Self::Location => "location",
            Self::Faction => "faction",
            Self::Creature => "creature",
            Self::Item => "item",
            Self::Event => "event",
            Self::PlayerCharacter => "player_character",
            Self::Misc => "misc",
        }
    }

    pub fn from_table(table: &str) -> Result<Self, EntityError> {
        match table {
            "npc" => Ok(Self::Npc),
            "location" => Ok(Self::Location),
            "faction" => Ok(Self::Faction),
            "creature" => Ok(Self::Creature),
            "item" => Ok(Self::Item),
            "event" => Ok(Self::Event),
            "player_character" => Ok(Self::PlayerCharacter),
            "misc" => Ok(Self::Misc),
            other => Err(EntityError::InvalidKind {
                kind: other.to_string(),
            }),
        }
    }
}

// ── SELECT alias clause ───────────────────────────────────────────────────────

/// Appended to every SELECT that needs to populate `campaign` and `collection`
/// in `GraphNodeRecord` via backward edge traversal.
///
/// Using `array::first(...)` to project a single record (or NULL when no edge
/// exists) from the `in_campaign` / `in_collection` edge tables.
const SELECT_SCOPE_ALIASES: &str = "array::first(<-in_campaign<-campaign) AS campaign, \
     array::first(<-in_collection<-collection) AS collection";

// ── Data structs ─────────────────────────────────────────────────────────────

/// Internal SurrealDB record — all type-specific fields are Option so a single
/// struct deserializes any node table.
///
/// `campaign` and `collection` are NOT stored scalar fields — they are projected
/// via the `SELECT_SCOPE_ALIASES` clause in every SELECT query.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GraphNodeRecord {
    pub id: Thing,
    // populated via backward traversal: array::first(<-in_campaign<-campaign)
    pub campaign: Option<Thing>,
    // populated via backward traversal: array::first(<-in_collection<-collection)
    pub collection: Option<Thing>,
    pub name: String,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // event fields
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    pub is_ongoing: Option<bool>,
    pub sequence_index: Option<i64>,
    pub era: Option<String>,
    pub duration_label: Option<String>,
    pub session: Option<Thing>, // event only — FK to session record
    // player_character fields
    pub player_name: Option<String>,
    pub character_class: Option<String>,
    pub character_level: Option<i64>,
    pub status: Option<String>,
}

impl From<GraphNodeRecord> for GraphNode {
    fn from(r: GraphNodeRecord) -> Self {
        Self {
            id: r.id.id.to_raw(),
            kind: r.id.tb.clone(),
            campaign_id: r.campaign.map(|t| t.id.to_raw()),
            collection_id: r.collection.map(|t| t.id.to_raw()),
            name: r.name,
            summary: r.summary,
            notes: r.notes,
            created_at: r.created_at,
            updated_at: r.updated_at,
            date_start: r.date_start,
            date_end: r.date_end,
            is_ongoing: r.is_ongoing,
            sequence_index: r.sequence_index,
            era: r.era,
            duration_label: r.duration_label,
            session_id: r.session.map(|t| t.id.to_raw()),
            player_name: r.player_name,
            character_class: r.character_class,
            character_level: r.character_level,
            status: r.status,
        }
    }
}

/// Frontend-facing DTO — sent over Tauri IPC.
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub campaign_id: Option<String>,
    pub collection_id: Option<String>,
    pub name: String,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    // event fields
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    pub is_ongoing: Option<bool>,
    pub sequence_index: Option<i64>,
    pub era: Option<String>,
    pub duration_label: Option<String>,
    pub session_id: Option<String>, // event only — raw session record ID
    // player_character fields
    pub player_name: Option<String>,
    pub character_class: Option<String>,
    pub character_level: Option<i64>,
    pub status: Option<String>,
}

/// A node as it appears in a relationship graph — identity + display only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphNodeRef {
    pub id: String,
    pub kind: String, // table name: npc, location, …
    pub name: String,
}

/// A directed `relates_to` edge between two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphEdge {
    pub from_id: String,
    pub from_kind: String,
    pub to_id: String,
    pub to_kind: String,
    pub rel_type: String,
    pub notes: Option<String>,
}

/// An ego graph: the center entity, its neighbors, and the edges among them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntityGraph {
    pub nodes: Vec<GraphNodeRef>,
    pub edges: Vec<GraphEdge>,
}

/// Input for both create and update operations.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityInput {
    pub name: String,
    pub summary: Option<String>,
    pub notes: Option<String>,
    // event
    pub date_start: Option<String>,
    pub date_end: Option<String>,
    pub is_ongoing: Option<bool>,
    pub sequence_index: Option<i64>,
    pub era: Option<String>,
    pub duration_label: Option<String>,
    pub session_id: Option<String>, // event only — links event to a session
    // player_character
    pub player_name: Option<String>,
    pub character_class: Option<String>,
    pub character_level: Option<i64>,
    pub status: Option<String>,
}

// ── Service functions ────────────────────────────────────────────────────────

/// Create a new graph node of the given kind, scoped to a campaign or collection.
///
/// Scope membership is recorded via `in_campaign` or `in_collection` edge tables
/// (not scalar fields). After creation, any `[[wikilinks]]` in `notes` are
/// resolved against the same scope and synced to `relates_to` edges.
pub async fn create<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: Option<&str>,
    collection_id: Option<&str>,
    kind: EntityKind,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    if input.name.trim().is_empty() {
        return Err(EntityError::Validation {
            field: "name".to_string(),
            message: "Name is required".to_string(),
        });
    }
    let table = kind.table_name();
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let id_for_wikilinks = id.clone();
    let notes_for_wikilinks = input.notes.clone();

    // 1. Create the entity record. Scope (campaign/collection) is NOT a field
    //    here — it is stored as an edge in the next step.
    db.query(
        "CREATE type::thing($table, $id) SET
            name            = $name,
            summary         = $summary,
            notes           = $notes,
            date_start      = $date_start,
            date_end        = $date_end,
            is_ongoing      = $is_ongoing,
            sequence_index  = $sequence_index,
            era             = $era,
            duration_label  = $duration_label,
            session         = IF $session_id IS NOT NONE THEN type::thing('session', $session_id) ELSE NULL END,
            player_name     = $player_name,
            character_class = $character_class,
            character_level = $character_level,
            status          = $status,
            created_at      = time::now(),
            updated_at      = time::now()",
    )
    .bind(("table", table))
    .bind(("id", id.clone()))
    .bind(("name", input.name.trim().to_owned()))
    .bind(("summary", input.summary))
    .bind(("notes", input.notes))
    .bind(("date_start", input.date_start))
    .bind(("date_end", input.date_end))
    .bind(("is_ongoing", input.is_ongoing))
    .bind(("sequence_index", input.sequence_index))
    .bind(("era", input.era))
    .bind(("duration_label", input.duration_label))
    .bind(("session_id", input.session_id))
    .bind(("player_name", input.player_name))
    .bind(("character_class", input.character_class))
    .bind(("character_level", input.character_level))
    .bind(("status", input.status))
    .await
    .map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;

    // 2. Create the scope edge.
    if let Some(cid) = campaign_id {
        db.query(
            "LET $src = type::thing('campaign', $cid); \
             LET $dst = type::thing($table, $id); \
             RELATE $src->in_campaign->$dst SET created_at = time::now()",
        )
        .bind(("cid", cid.to_owned()))
        .bind(("table", table))
        .bind(("id", id.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    }
    if let Some(col) = collection_id {
        db.query(
            "LET $src = type::thing('collection', $col); \
             LET $dst = type::thing($table, $id); \
             RELATE $src->in_collection->$dst SET created_at = time::now()",
        )
        .bind(("col", col.to_owned()))
        .bind(("table", table))
        .bind(("id", id.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    }

    // 3. Build the GraphNode from what we already know — no re-fetch needed.
    //    The backward-traversal aliases used in other read paths
    //    (SELECT_SCOPE_ALIASES) rely on edge table lookups that are unreliable
    //    immediately after creation in the RocksDB engine. Since campaign_id and
    //    collection_id are function parameters we can populate them directly.
    let mut fetch_resp = db
        .query("SELECT * FROM type::thing($table, $id)")
        .bind(("table", table))
        .bind(("id", id.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;

    // GraphNodeRecord.campaign / .collection will be None here (no aliases),
    // but we override them from the function parameters below.
    #[derive(serde::Deserialize)]
    struct StoredRecord {
        id: surrealdb::sql::Thing,
        #[serde(default)]
        name: String,
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        notes: Option<String>,
        #[serde(default)]
        created_at: Option<String>,
        #[serde(default)]
        updated_at: Option<String>,
        #[serde(default)]
        date_start: Option<String>,
        #[serde(default)]
        date_end: Option<String>,
        #[serde(default)]
        is_ongoing: Option<bool>,
        #[serde(default)]
        sequence_index: Option<i64>,
        #[serde(default)]
        era: Option<String>,
        #[serde(default)]
        duration_label: Option<String>,
        #[serde(default)]
        session: Option<surrealdb::sql::Thing>,
        #[serde(default)]
        player_name: Option<String>,
        #[serde(default)]
        character_class: Option<String>,
        #[serde(default)]
        character_level: Option<i64>,
        #[serde(default)]
        status: Option<String>,
    }

    let recs: Vec<StoredRecord> = fetch_resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    let rec = recs
        .into_iter()
        .next()
        .ok_or_else(|| EntityError::Database {
            message: "No record returned after create".to_string(),
        })?;

    let node = GraphNode {
        id: rec.id.id.to_raw(),
        kind: rec.id.tb.clone(),
        campaign_id: campaign_id.map(|s| s.to_owned()),
        collection_id: collection_id.map(|s| s.to_owned()),
        name: rec.name,
        summary: rec.summary,
        notes: rec.notes,
        created_at: rec.created_at,
        updated_at: rec.updated_at,
        date_start: rec.date_start,
        date_end: rec.date_end,
        is_ongoing: rec.is_ongoing,
        sequence_index: rec.sequence_index,
        era: rec.era,
        duration_label: rec.duration_label,
        session_id: rec.session.map(|t| t.id.to_raw()),
        player_name: rec.player_name,
        character_class: rec.character_class,
        character_level: rec.character_level,
        status: rec.status,
    };

    // 4. Sync wikilinks in notes to relates_to edges (fire-and-forget: ignore errors
    //    so a wikilink resolution failure never blocks the entity save).
    if let Some(notes) = &notes_for_wikilinks {
        if !notes.is_empty() {
            use crate::services::wikilink::WikilinkScope;
            let scope = match (campaign_id, collection_id) {
                (Some(cid), _) => Some(WikilinkScope::Campaign { campaign_id: cid }),
                (_, Some(col)) => Some(WikilinkScope::Collection { collection_id: col }),
                _ => None,
            };
            if let Some(scope) = scope {
                let _ = crate::services::wikilink::parse_and_sync_wikilinks(
                    db,
                    table,
                    &id_for_wikilinks,
                    notes,
                    scope,
                )
                .await;
            }
        }
    }

    Ok(node)
}

/// Fetch a single node by its raw ID and kind.
pub async fn get_by_id<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
) -> Result<GraphNode, EntityError> {
    let table = kind.table_name();
    let sql = format!("SELECT *, {SELECT_SCOPE_ALIASES} FROM type::thing($table, $id)");
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let records: Vec<GraphNodeRecord> = response.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| EntityError::NotFound { id: id.to_string() })
}

/// List all nodes of a kind visible to a campaign, ordered by name.
///
/// Returns both campaign-scoped entities (`in_campaign` edge) and entities
/// belonging to any collection the campaign `subscribes_to` (`in_collection`
/// edge). Extraction writes collection-scoped entities, so the campaign browser
/// must include subscribed collections or extracted entities never surface.
pub async fn get_by_campaign<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    kind: EntityKind,
) -> Result<Vec<GraphNode>, EntityError> {
    let table = kind.table_name();
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} \
         FROM type::table($table) \
         WHERE id IN (SELECT VALUE out FROM in_campaign WHERE in = type::thing('campaign', $campaign_id)) \
            OR id IN (SELECT VALUE out FROM in_collection \
                      WHERE in IN (SELECT VALUE out FROM subscribes_to \
                                   WHERE in = type::thing('campaign', $campaign_id))) \
         ORDER BY name ASC"
    );
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("campaign_id", campaign_id.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let records: Vec<GraphNodeRecord> = response.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Order events for a timeline by their canonical key.
///
/// `sequence_index` is the ordering key (lower = earlier); unsequenced events
/// (`NULL`) sort last so a half-filled timeline still reads sensibly, and ties
/// are broken by name for a stable order. `date_start` is never parsed — it is
/// an opaque in-world string (see CLAUDE.md), so `sequence_index` is canonical.
pub fn order_events_for_timeline(mut events: Vec<GraphNode>) -> Vec<GraphNode> {
    use std::cmp::Ordering;
    events.sort_by(|a, b| match (a.sequence_index, b.sequence_index) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.name.cmp(&b.name)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.name.cmp(&b.name),
    });
    events
}

/// Fetch a campaign's events in timeline order (see [`order_events_for_timeline`]).
pub async fn get_events_timeline<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<GraphNode>, EntityError> {
    let events = get_by_campaign(db, campaign_id, EntityKind::Event).await?;
    Ok(order_events_for_timeline(events))
}

/// List all nodes of a kind for a collection, ordered by name.
/// Count entities of every kind that belong to a campaign.
///
/// Returns a map keyed by table name (`npc`, `location`, …) with an entry for
/// all eight kinds, zero included — the rail uses it to label its nav items.
pub async fn count_by_campaign<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<std::collections::HashMap<String, u64>, EntityError> {
    #[derive(Deserialize)]
    struct CountRow {
        c: u64,
    }

    const ALL_KINDS: [EntityKind; 8] = [
        EntityKind::Npc,
        EntityKind::Location,
        EntityKind::Faction,
        EntityKind::Creature,
        EntityKind::Item,
        EntityKind::Event,
        EntityKind::PlayerCharacter,
        EntityKind::Misc,
    ];

    let mut counts = std::collections::HashMap::new();
    for kind in ALL_KINDS {
        let table = kind.table_name();
        let row: Option<CountRow> = db
            .query(
                "SELECT count() AS c FROM type::table($table) \
                 WHERE id IN (SELECT VALUE out FROM in_campaign \
                              WHERE in = type::thing('campaign', $campaign_id)) \
                    OR id IN (SELECT VALUE out FROM in_collection \
                              WHERE in IN (SELECT VALUE out FROM subscribes_to \
                                           WHERE in = type::thing('campaign', $campaign_id))) \
                 GROUP ALL",
            )
            .bind(("table", table))
            .bind(("campaign_id", campaign_id.to_owned()))
            .await
            .map_err(|e| EntityError::Database {
                message: e.to_string(),
            })?
            .take(0)
            .map_err(|e| EntityError::Database {
                message: e.to_string(),
            })?;
        counts.insert(table.to_string(), row.map(|r| r.c).unwrap_or(0));
    }
    Ok(counts)
}

pub async fn get_by_collection<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
    kind: EntityKind,
) -> Result<Vec<GraphNode>, EntityError> {
    let table = kind.table_name();
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} \
         FROM type::table($table) \
         WHERE id IN (SELECT VALUE out FROM in_collection WHERE in = type::thing('collection', $collection_id)) \
         ORDER BY name ASC"
    );
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("collection_id", collection_id.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let records: Vec<GraphNodeRecord> = response.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Find a collection-scoped entity by name (case-insensitive) and kind.
///
/// Used by `ExtractionService` for deduplication before creating new entities.
pub async fn find_by_name_and_collection<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
    name: &str,
    kind: EntityKind,
) -> Result<Option<GraphNode>, EntityError> {
    let table = kind.table_name();
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} \
         FROM type::table($table) \
         WHERE id IN (SELECT VALUE out FROM in_collection WHERE in = type::thing('collection', $collection_id)) \
             AND string::lowercase(name) = string::lowercase($name) \
         LIMIT 1"
    );
    let mut response = db
        .query(sql)
        .bind(("table", table))
        .bind(("collection_id", collection_id.to_owned()))
        .bind(("name", name.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let records: Vec<GraphNodeRecord> = response.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(records.into_iter().next().map(Into::into))
}

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
) -> Result<GraphNode, EntityError> {
    if input.name.trim().is_empty() {
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
            updated_at     = time::now();
         SELECT *, {SELECT_SCOPE_ALIASES} FROM type::thing($table, $id)"
    );
    let mut response = db
        .query(update_sql)
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .bind(("name", input.name.trim().to_owned()))
        // Nullable fields: bind explicit NULL (not NONE) on `None`. The graph
        // entity tables are SCHEMAFULL with `string | NULL` / `int | NULL`
        // fields, which reject NONE — binding `Option::None` directly would
        // silently abort the UPDATE and leave the old value in place.
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
    // The UPDATE is at index 0; the SELECT is at index 1.
    let records: Vec<GraphNodeRecord> = response.take(1).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    let node: GraphNode = records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| EntityError::NotFound { id: id.to_string() })?;

    // Sync wikilinks in notes to relates_to edges (fire-and-forget).
    if let Some(ref notes) = notes_for_wikilinks {
        use crate::services::wikilink::WikilinkScope;
        let scope = match (node.campaign_id.as_deref(), node.collection_id.as_deref()) {
            (Some(cid), _) => Some(WikilinkScope::Campaign { campaign_id: cid }),
            (_, Some(col)) => Some(WikilinkScope::Collection { collection_id: col }),
            _ => None,
        };
        if let Some(scope) = scope {
            let _ =
                crate::services::wikilink::parse_and_sync_wikilinks(db, table, id, notes, scope)
                    .await;
        }
    }

    Ok(node)
}

/// Return all events that reference the given session.
pub async fn get_events_for_session<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    session_id: &str,
) -> Result<Vec<GraphNode>, EntityError> {
    let sql = format!(
        "SELECT *, {SELECT_SCOPE_ALIASES} FROM event \
         WHERE session = type::thing('session', $session_id)"
    );
    let mut response = db
        .query(sql)
        .bind(("session_id", session_id.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let records: Vec<GraphNodeRecord> = response.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Hard-delete a graph node by id.
pub async fn delete<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
) -> Result<(), EntityError> {
    let table = kind.table_name();
    db.query("DELETE type::thing($table, $id)")
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    Ok(())
}

/// Compose the document text used to embed an entity for semantic retrieval.
///
/// Includes name, summary, **and notes** so hand-written notes participate in
/// retrieval — the whole point of notes indexing. Empty parts are skipped.
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
/// This is the single source of truth for entity embedding. It is called both
/// by manual create/update (so hand-edited notes become searchable) and by
/// `extraction_service` (LLM-extracted entities). A zero-length vector — e.g.
/// from a mock provider whose model isn't ready — is treated as a no-op rather
/// than an error so callers never block a save on embedding.
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

/// Fetch the ego graph around an entity: the center, its `relates_to` neighbors
/// (one hop), and the edges among them. `depth` is currently always treated as
/// one hop; deeper walks are produced client-side by re-calling on a neighbor.
pub async fn get_entity_graph<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: &str,
    _depth: u32,
) -> Result<EntityGraph, EntityError> {
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

    // 2. Collect distinct (kind, id) node keys: the center plus every endpoint.
    let mut keys: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
    keys.insert((kind.to_string(), id.to_string()));
    for r in &rows {
        keys.insert((r.in_.tb.clone(), r.in_.id.to_raw()));
        keys.insert((r.out.tb.clone(), r.out.id.to_raw()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_kind_table_names_are_correct() {
        assert_eq!(EntityKind::Npc.table_name(), "npc");
        assert_eq!(EntityKind::Location.table_name(), "location");
        assert_eq!(EntityKind::Faction.table_name(), "faction");
        assert_eq!(EntityKind::Creature.table_name(), "creature");
        assert_eq!(EntityKind::Item.table_name(), "item");
        assert_eq!(EntityKind::Event.table_name(), "event");
        assert_eq!(EntityKind::PlayerCharacter.table_name(), "player_character");
        assert_eq!(EntityKind::Misc.table_name(), "misc");
    }

    #[test]
    fn entity_kind_from_table_roundtrips_all_variants() {
        let variants = [
            EntityKind::Npc,
            EntityKind::Location,
            EntityKind::Faction,
            EntityKind::Creature,
            EntityKind::Item,
            EntityKind::Event,
            EntityKind::PlayerCharacter,
            EntityKind::Misc,
        ];
        for kind in &variants {
            assert_eq!(
                EntityKind::from_table(kind.table_name()).unwrap(),
                *kind,
                "roundtrip failed for {:?}",
                kind
            );
        }
    }

    #[test]
    fn entity_kind_from_table_unknown_returns_invalid_kind() {
        let err = EntityKind::from_table("goblin").unwrap_err();
        assert!(matches!(err, EntityError::InvalidKind { kind } if kind == "goblin"));
    }

    #[test]
    fn order_events_for_timeline_sorts_by_sequence_then_name_nulls_last() {
        fn event(name: &str, seq: Option<i64>) -> GraphNode {
            GraphNode {
                id: name.to_string(),
                kind: "event".to_string(),
                campaign_id: None,
                collection_id: None,
                name: name.to_string(),
                summary: None,
                notes: None,
                created_at: None,
                updated_at: None,
                date_start: None,
                date_end: None,
                is_ongoing: None,
                sequence_index: seq,
                era: None,
                duration_label: None,
                session_id: None,
                player_name: None,
                character_class: None,
                character_level: None,
                status: None,
            }
        }
        // Deliberately unsorted, with a tie at seq=2 and two unsequenced events.
        let input = vec![
            event("Unplaced B", None),
            event("Second", Some(2)),
            event("Third", Some(3)),
            event("Also Second", Some(2)),
            event("First", Some(1)),
            event("Unplaced A", None),
        ];
        let ordered: Vec<String> = order_events_for_timeline(input)
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(
            ordered,
            vec![
                "First",       // seq 1
                "Also Second", // seq 2, name tiebreak before "Second"
                "Second",      // seq 2
                "Third",       // seq 3
                "Unplaced A",  // NULL seq, name order
                "Unplaced B",
            ]
        );
    }

    #[test]
    fn embed_text_includes_name_summary_and_notes() {
        let text = embed_text(
            "Seraphina",
            Some("the archivist"),
            Some("Guards the Sunstone."),
        );
        assert!(text.contains("Seraphina"), "name missing: {text}");
        assert!(text.contains("the archivist"), "summary missing: {text}");
        assert!(
            text.contains("Guards the Sunstone."),
            "notes missing: {text}"
        );
    }

    #[test]
    fn embed_text_skips_empty_parts() {
        assert_eq!(embed_text("Bob", None, None), "Bob");
        assert_eq!(embed_text("Bob", Some("  "), Some("")), "Bob");
    }

    #[tokio::test]
    async fn embed_node_populates_embedding_and_model() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let node = create(
            &db,
            Some("camp1"),
            None,
            EntityKind::Npc,
            EntityInput {
                name: "Seraphina".to_string(),
                notes: Some("Guards the Sunstone beneath the Iron Tower.".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let embed: Arc<dyn EmbeddingProvider> =
            Arc::new(crate::providers::embedding::MockEmbeddingProvider::new(768));
        embed_node(&db, &embed, &node).await.unwrap();

        #[derive(Deserialize)]
        struct Row {
            embedding: Option<Vec<f32>>,
            embed_model: Option<String>,
        }
        let mut resp = db
            .query("SELECT embedding, embed_model FROM type::thing('npc', $id)")
            .bind(("id", node.id.clone()))
            .await
            .unwrap();
        let rows: Vec<Row> = resp.take(0).unwrap();
        let row = rows.into_iter().next().expect("npc row");
        assert_eq!(
            row.embedding.as_ref().map(|v| v.len()),
            Some(768),
            "embedding vector should be stored with the provider's dimension"
        );
        assert_eq!(row.embed_model.as_deref(), Some("mock"));
    }

    #[tokio::test]
    async fn count_by_campaign_returns_per_kind_counts() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let input = |name: &str| EntityInput {
            name: name.to_string(),
            ..Default::default()
        };
        create(&db, Some("camp1"), None, EntityKind::Npc, input("Torvin"))
            .await
            .unwrap();
        create(&db, Some("camp1"), None, EntityKind::Npc, input("Mira"))
            .await
            .unwrap();
        create(
            &db,
            Some("camp1"),
            None,
            EntityKind::Location,
            input("Docks"),
        )
        .await
        .unwrap();

        let counts = count_by_campaign(&db, "camp1").await.unwrap();
        assert_eq!(counts.get("npc"), Some(&2));
        assert_eq!(counts.get("location"), Some(&1));
        assert_eq!(counts.get("faction"), Some(&0));
        assert_eq!(counts.len(), 8, "every kind should be present");
    }

    #[tokio::test]
    async fn count_by_campaign_does_not_count_other_campaigns() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        for c in ["camp1", "camp2"] {
            db.query(format!(
                "CREATE campaign SET id='{c}', name='Test', system='5e', \
                 created_at=time::now(), updated_at=time::now()"
            ))
            .await
            .unwrap();
        }
        create(
            &db,
            Some("camp2"),
            None,
            EntityKind::Npc,
            EntityInput {
                name: "Elsewhere".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let counts = count_by_campaign(&db, "camp1").await.unwrap();
        assert_eq!(counts.get("npc"), Some(&0));
    }

    #[tokio::test]
    async fn create_with_campaign_id_populates_campaign_via_edge() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let node = create(
            &db,
            Some("camp1"),
            None,
            EntityKind::Npc,
            EntityInput {
                name: "Torvin".to_string(),
                summary: None,
                notes: None,
                date_start: None,
                date_end: None,
                is_ongoing: None,
                sequence_index: None,
                era: None,
                duration_label: None,
                session_id: None,
                player_name: None,
                character_class: None,
                character_level: None,
                status: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(node.campaign_id.as_deref(), Some("camp1"));
        assert!(node.collection_id.is_none());

        // Verify in_campaign edge was written
        let mut resp = db
            .query("SELECT count() FROM in_campaign WHERE in = type::thing('campaign','camp1') GROUP ALL")
            .await
            .unwrap();
        #[derive(serde::Deserialize)]
        struct C {
            count: i64,
        }
        let counts: Vec<C> = resp.take(0).unwrap();
        assert_eq!(counts.first().map(|c| c.count).unwrap_or(0), 1);
    }

    #[tokio::test]
    async fn create_with_collection_id_populates_collection_via_edge() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE collection SET id='col1', name='PHB', description=NULL, \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let node = create(
            &db,
            None,
            Some("col1"),
            EntityKind::Npc,
            EntityInput {
                name: "Goblin".to_string(),
                summary: None,
                notes: None,
                date_start: None,
                date_end: None,
                is_ongoing: None,
                sequence_index: None,
                era: None,
                duration_label: None,
                session_id: None,
                player_name: None,
                character_class: None,
                character_level: None,
                status: None,
            },
        )
        .await
        .unwrap();

        assert!(node.campaign_id.is_none());
        assert_eq!(node.collection_id.as_deref(), Some("col1"));
    }

    #[tokio::test]
    async fn get_by_campaign_returns_only_campaign_entities() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col1', name='PHB', description=NULL, \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let input = |name: &str| EntityInput {
            name: name.to_string(),
            summary: None,
            notes: None,
            date_start: None,
            date_end: None,
            is_ongoing: None,
            sequence_index: None,
            era: None,
            duration_label: None,
            session_id: None,
            player_name: None,
            character_class: None,
            character_level: None,
            status: None,
        };

        create(&db, Some("camp1"), None, EntityKind::Npc, input("Torvin"))
            .await
            .unwrap();
        create(&db, None, Some("col1"), EntityKind::Npc, input("Goblin"))
            .await
            .unwrap();

        let results = get_by_campaign(&db, "camp1", EntityKind::Npc)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Torvin");
    }

    /// Regression: entities extracted from a rulebook are collection-scoped
    /// (`in_collection`). The campaign entity browser must surface them when the
    /// campaign `subscribes_to` that collection — otherwise extraction output is
    /// invisible in the UI even though the RAG agent can see it.
    #[tokio::test]
    async fn get_by_campaign_includes_subscribed_collection_entities() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col1', name='PHB', description=NULL, \
             created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col2', name='DMG', description=NULL, \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        // camp1 subscribes to col1 only — col2 is an unrelated rulebook.
        db.query(
            "LET $in = type::thing('campaign','camp1'); \
             LET $out1 = type::thing('collection','col1'); \
             RELATE $in->subscribes_to->$out1 SET created_at=time::now()",
        )
        .await
        .unwrap();

        let input = |name: &str| EntityInput {
            name: name.to_string(),
            summary: None,
            notes: None,
            date_start: None,
            date_end: None,
            is_ongoing: None,
            sequence_index: None,
            era: None,
            duration_label: None,
            session_id: None,
            player_name: None,
            character_class: None,
            character_level: None,
            status: None,
        };

        create(&db, Some("camp1"), None, EntityKind::Npc, input("Torvin"))
            .await
            .unwrap();
        create(&db, None, Some("col1"), EntityKind::Npc, input("Goblin"))
            .await
            .unwrap();
        // Subscribed-to a different collection — must NOT appear.
        create(&db, None, Some("col2"), EntityKind::Npc, input("Lich"))
            .await
            .unwrap();

        let results = get_by_campaign(&db, "camp1", EntityKind::Npc)
            .await
            .unwrap();
        let names: Vec<_> = results.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["Goblin", "Torvin"], "ordered by name ASC");
    }

    #[tokio::test]
    async fn get_by_collection_returns_only_collection_entities() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now(); \
             CREATE collection SET id='col1', name='PHB', description=NULL, \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let input = |name: &str| EntityInput {
            name: name.to_string(),
            summary: None,
            notes: None,
            date_start: None,
            date_end: None,
            is_ongoing: None,
            sequence_index: None,
            era: None,
            duration_label: None,
            session_id: None,
            player_name: None,
            character_class: None,
            character_level: None,
            status: None,
        };

        create(&db, Some("camp1"), None, EntityKind::Npc, input("Torvin"))
            .await
            .unwrap();
        create(&db, None, Some("col1"), EntityKind::Npc, input("Goblin"))
            .await
            .unwrap();

        let results = get_by_collection(&db, "col1", EntityKind::Npc)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Goblin");
    }

    #[tokio::test]
    async fn find_by_name_and_collection_is_case_insensitive() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        db.query(
            "CREATE collection SET id='col1', name='PHB', description=NULL, \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let input = |name: &str| EntityInput {
            name: name.to_string(),
            summary: None,
            notes: None,
            date_start: None,
            date_end: None,
            is_ongoing: None,
            sequence_index: None,
            era: None,
            duration_label: None,
            session_id: None,
            player_name: None,
            character_class: None,
            character_level: None,
            status: None,
        };

        create(
            &db,
            None,
            Some("col1"),
            EntityKind::Npc,
            input("The Iron Fist"),
        )
        .await
        .unwrap();

        let found = find_by_name_and_collection(&db, "col1", "the iron fist", EntityKind::Npc)
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "The Iron Fist");

        let not_found = find_by_name_and_collection(&db, "col1", "other", EntityKind::Npc)
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn update_clears_nullable_fields_to_null_not_none() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        // Create an npc with a summary and notes…
        let node = create(
            &db,
            Some("camp1"),
            None,
            EntityKind::Npc,
            EntityInput {
                name: "Torvin".to_string(),
                summary: Some("Old summary.".to_string()),
                notes: Some("Old notes.".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // …then clear them. Binding `None` must persist SurrealDB NULL, not NONE,
        // which the SCHEMAFULL `string | NULL` fields would reject.
        let updated = update(
            &db,
            &node.id,
            EntityKind::Npc,
            EntityInput {
                name: "Torvin".to_string(),
                summary: None,
                notes: None,
                ..Default::default()
            },
        )
        .await
        .expect("update should not error when clearing nullable fields");
        assert_eq!(updated.summary, None);
        assert_eq!(updated.notes, None);

        // Confirm it actually persisted (not just the returned value).
        let refetched = get_by_id(&db, &node.id, EntityKind::Npc).await.unwrap();
        assert_eq!(
            refetched.summary, None,
            "summary should be cleared in the DB"
        );
        assert_eq!(refetched.notes, None, "notes should be cleared in the DB");
    }

    #[tokio::test]
    async fn update_clears_nullable_event_fields() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        let node = create(
            &db,
            Some("camp1"),
            None,
            EntityKind::Event,
            EntityInput {
                name: "The Siege".to_string(),
                date_start: Some("1402".to_string()),
                era: Some("Third Age".to_string()),
                sequence_index: Some(3),
                is_ongoing: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let updated = update(
            &db,
            &node.id,
            EntityKind::Event,
            EntityInput {
                name: "The Siege".to_string(),
                date_start: None,
                era: None,
                sequence_index: None,
                is_ongoing: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("clearing nullable event fields should not error");
        assert_eq!(updated.date_start, None);
        assert_eq!(updated.era, None);
        assert_eq!(updated.sequence_index, None);
    }
}
