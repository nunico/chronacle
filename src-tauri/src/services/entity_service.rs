use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use thiserror::Error;

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(tag = "code", content = "message", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityError {
    #[error("Entity '{id}' not found")]
    NotFound { id: String },
    #[error("Entity does not belong to the specified campaign")]
    CampaignMismatch,
    #[error("Unknown entity kind: '{kind}'")]
    InvalidKind { kind: String },
    #[error("Validation error on field '{field}': {message}")]
    Validation { field: String, message: String },
    #[error("Database error: {0}")]
    Database(String),
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
            other => Err(EntityError::InvalidKind { kind: other.to_string() }),
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
    fn entity_kind_from_table_roundtrips() {
        for (table, kind) in &[
            ("npc", EntityKind::Npc),
            ("location", EntityKind::Location),
            ("player_character", EntityKind::PlayerCharacter),
        ] {
            assert_eq!(EntityKind::from_table(table).unwrap(), *kind);
        }
    }

    #[test]
    fn entity_kind_from_table_unknown_returns_invalid_kind() {
        let err = EntityKind::from_table("goblin").unwrap_err();
        assert!(matches!(err, EntityError::InvalidKind { kind } if kind == "goblin"));
    }
}
