use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use thiserror::Error;

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", tag = "code")]
pub enum EntityError {
    #[error("Entity '{id}' not found")]
    NotFound { id: String },
    #[error("Entity does not belong to the specified campaign")]
    CampaignMismatch,
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

// ── Data structs ─────────────────────────────────────────────────────────────

/// Internal SurrealDB record — all type-specific fields are Option so a single
/// struct deserializes any node table.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GraphNodeRecord {
    pub id: Thing,
    pub campaign: Option<Thing>,
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
    // player_character fields
    pub player_name: Option<String>,
    pub character_class: Option<String>,
    pub character_level: Option<i64>,
    pub status: Option<String>,
}

/// Input for both create and update operations.
#[derive(Debug, Clone, Deserialize)]
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
    // player_character
    pub player_name: Option<String>,
    pub character_class: Option<String>,
    pub character_level: Option<i64>,
    pub status: Option<String>,
}

// ── Service functions ────────────────────────────────────────────────────────

/// Create a new graph node of the given kind scoped to an optional campaign.
pub async fn create<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: Option<&str>,
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
    let mut response = db
        .query(
            "CREATE type::thing($table, $id) SET
                campaign        = IF $campaign_id IS NOT NONE THEN type::thing('campaign', $campaign_id) ELSE NULL END,
                name            = $name,
                summary         = $summary,
                notes           = $notes,
                date_start      = $date_start,
                date_end        = $date_end,
                is_ongoing      = $is_ongoing,
                sequence_index  = $sequence_index,
                era             = $era,
                duration_label  = $duration_label,
                player_name     = $player_name,
                character_class = $character_class,
                character_level = $character_level,
                status          = $status,
                created_at      = time::now(),
                updated_at      = time::now()",
        )
        .bind(("table", table))
        .bind(("id", id))
        .bind(("campaign_id", campaign_id.map(|s| s.to_owned())))
        .bind(("name", input.name.trim().to_owned()))
        .bind(("summary", input.summary))
        .bind(("notes", input.notes))
        .bind(("date_start", input.date_start))
        .bind(("date_end", input.date_end))
        .bind(("is_ongoing", input.is_ongoing))
        .bind(("sequence_index", input.sequence_index))
        .bind(("era", input.era))
        .bind(("duration_label", input.duration_label))
        .bind(("player_name", input.player_name))
        .bind(("character_class", input.character_class))
        .bind(("character_level", input.character_level))
        .bind(("status", input.status))
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
        .ok_or_else(|| EntityError::Database {
            message: "No record returned after create".to_string(),
        })
}

/// Fetch a single node by its raw ID and kind.
pub async fn get_by_id<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
) -> Result<GraphNode, EntityError> {
    let table = kind.table_name();
    let mut response = db
        .query("SELECT * FROM type::thing($table, $id)")
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

/// List all nodes of a kind for a campaign, ordered by name.
pub async fn get_by_campaign<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    kind: EntityKind,
) -> Result<Vec<GraphNode>, EntityError> {
    let table = kind.table_name();
    let mut response = db
        .query("SELECT * FROM type::table($table) WHERE campaign = type::thing('campaign', $campaign_id) ORDER BY name ASC")
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
    let mut response = db
        .query(
            "UPDATE type::thing($table, $id) SET
                name         = $name,
                summary      = $summary,
                notes        = $notes,
                date_start   = $date_start,
                date_end     = $date_end,
                is_ongoing   = $is_ongoing,
                sequence_index = $sequence_index,
                era          = $era,
                duration_label = $duration_label,
                player_name  = $player_name,
                character_class = $character_class,
                character_level = $character_level,
                status       = $status,
                updated_at   = time::now()",
        )
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .bind(("name", input.name.trim().to_owned()))
        .bind(("summary", input.summary))
        .bind(("notes", input.notes))
        .bind(("date_start", input.date_start))
        .bind(("date_end", input.date_end))
        .bind(("is_ongoing", input.is_ongoing))
        .bind(("sequence_index", input.sequence_index))
        .bind(("era", input.era))
        .bind(("duration_label", input.duration_label))
        .bind(("player_name", input.player_name))
        .bind(("character_class", input.character_class))
        .bind(("character_level", input.character_level))
        .bind(("status", input.status))
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
}
