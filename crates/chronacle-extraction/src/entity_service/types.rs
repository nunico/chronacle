//! Data model for the entity graph: error type, entity kinds, relationship
//! vocabulary, the DB record, and the frontend-facing DTOs.

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
    #[error("Collection entity cannot link to a campaign entity")]
    CrossLinkViolation,
    #[error("Unknown entity kind: '{kind}'")]
    InvalidKind { kind: String },
    #[error("Validation error on field '{field}': {message}")]
    Validation { field: String, message: String },
    #[error("Database error: {message}")]
    Database { message: String },
    #[error("Scope violation: {from} may not reference {to} (see reference rules, ADR-009)")]
    ScopeViolation { from: String, to: String },
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

// ── Relationship type ──────────────────────────────────────────────────────────

/// Canonical, finite vocabulary for `relates_to.rel_type`.
///
/// Both directions of each directional relationship are first-class variants so
/// the LLM always has a fitting type for the direction the source text describes
/// (no dropout). Inverse members normalize to their canonical counterpart via
/// [`RelType::canonical`], which also reports whether the edge must be flipped.
/// `Other` carries any unrecognised value verbatim — "unknown" is derived, not
/// stored, so no migration is needed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelType {
    // Directional — canonical direction.
    Leads,
    MemberOf,
    LocatedIn,
    Owns,
    Serves,
    Created,
    ParentOf,
    // Directional — inverse direction (normalize via `canonical()`).
    LedBy,
    HasMember,
    Contains,
    OwnedBy,
    ServedBy,
    CreatedBy,
    ChildOf,
    // Symmetric — direction irrelevant, self-inverse.
    AlliedWith,
    EnemyOf,
    RelatedTo,
    Knows,
    // Catch-all for unrecognised LLM output (stored verbatim).
    Other(String),
}

impl RelType {
    /// Parse a raw `rel_type` string from the LLM. Infallible: unknown values
    /// become `Other(raw)`. (Named `from_llm`, not `from_str`, to avoid clippy's
    /// `should_implement_trait` lint on an infallible parser.)
    pub fn from_llm(raw: &str) -> Self {
        match raw {
            "leads" => Self::Leads,
            "member_of" => Self::MemberOf,
            "located_in" => Self::LocatedIn,
            "owns" => Self::Owns,
            "serves" => Self::Serves,
            "created" => Self::Created,
            "parent_of" => Self::ParentOf,
            "led_by" => Self::LedBy,
            "has_member" => Self::HasMember,
            "contains" => Self::Contains,
            "owned_by" => Self::OwnedBy,
            "served_by" => Self::ServedBy,
            "created_by" => Self::CreatedBy,
            "child_of" => Self::ChildOf,
            "allied_with" => Self::AlliedWith,
            "enemy_of" => Self::EnemyOf,
            "related_to" => Self::RelatedTo,
            "knows" => Self::Knows,
            other => Self::Other(other.to_string()),
        }
    }

    /// Stable snake_case key for known variants; the raw string for `Other`.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Leads => "leads",
            Self::MemberOf => "member_of",
            Self::LocatedIn => "located_in",
            Self::Owns => "owns",
            Self::Serves => "serves",
            Self::Created => "created",
            Self::ParentOf => "parent_of",
            Self::LedBy => "led_by",
            Self::HasMember => "has_member",
            Self::Contains => "contains",
            Self::OwnedBy => "owned_by",
            Self::ServedBy => "served_by",
            Self::CreatedBy => "created_by",
            Self::ChildOf => "child_of",
            Self::AlliedWith => "allied_with",
            Self::EnemyOf => "enemy_of",
            Self::RelatedTo => "related_to",
            Self::Knows => "knows",
            Self::Other(s) => s.as_str(),
        }
    }

    /// True for any known variant; false only for `Other`.
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }

    /// Normalize to canonical direction. Returns `(canonical_variant, flip)`:
    /// when `flip` is true the caller must swap the edge's `in`/`out` endpoints.
    /// Canonical and symmetric variants (and `Other`) return `(self, false)`.
    pub fn canonical(&self) -> (RelType, bool) {
        match self {
            Self::LedBy => (Self::Leads, true),
            Self::HasMember => (Self::MemberOf, true),
            Self::Contains => (Self::LocatedIn, true),
            Self::OwnedBy => (Self::Owns, true),
            Self::ServedBy => (Self::Serves, true),
            Self::CreatedBy => (Self::Created, true),
            Self::ChildOf => (Self::ParentOf, true),
            other => (other.clone(), false),
        }
    }
}

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
    #[serde(default)]
    pub aliases: Vec<String>,
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
    // codex fields
    pub codex_article: Option<String>,
    pub codex_stale: Option<bool>,
    pub codex_compiled_at: Option<surrealdb::sql::Datetime>,
}

impl From<GraphNodeRecord> for GraphNode {
    fn from(r: GraphNodeRecord) -> Self {
        Self {
            id: r.id.id.to_raw(),
            kind: r.id.tb.clone(),
            campaign_id: r.campaign.map(|t| t.id.to_raw()),
            collection_id: r.collection.map(|t| t.id.to_raw()),
            name: r.name,
            aliases: r.aliases,
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
            codex_article: r.codex_article,
            codex_stale: r.codex_stale,
            codex_compiled_at: r.codex_compiled_at.map(|d| d.to_string()),
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
    pub aliases: Vec<String>,
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
    // codex fields
    pub codex_article: Option<String>,
    pub codex_stale: Option<bool>,
    pub codex_compiled_at: Option<String>,
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
    /// `None` vs `Some(vec![])` are NOT interchangeable, and the direction is
    /// the opposite of `GmParts`/vault-inbound `Option` fields elsewhere in
    /// this codebase. The desktop entity editor always sends the COMPLETE
    /// current array (`Some(v)`), so an alternate-name edit is never a no-op;
    /// `None` exists for non-form callers that legitimately have no opinion.
    ///
    /// - `None` means the caller has no opinion: UPDATE preserves whatever
    ///   aliases are already stored, CREATE defaults to `[]`.
    /// - `Some(v)` means the caller wants aliases set to exactly `v`,
    ///   including `Some(vec![])` to explicitly clear them.
    ///
    /// Contrast with vault-inbound parsing, where a payload is built from a
    /// WHOLE FILE and an absent section genuinely means the GM deleted it, so
    /// `None` means CLEAR there. Same `Option`, opposite meaning — provenance
    /// decides, not the type.
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
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

/// A related entity as it appears in the flat relationships list.
///
/// `direction` is `"outbound"` when the center entity is the `in` side of the
/// edge (i.e. center→other), and `"inbound"` when the center is the `out` side
/// (i.e. other→center).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelatedEntity {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub rel_type: String,
    pub direction: String,
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
    fn rel_type_known_variants_roundtrip() {
        for key in [
            "leads",
            "member_of",
            "located_in",
            "owns",
            "serves",
            "created",
            "parent_of",
            "led_by",
            "has_member",
            "contains",
            "owned_by",
            "served_by",
            "created_by",
            "child_of",
            "allied_with",
            "enemy_of",
            "related_to",
            "knows",
        ] {
            let rt = RelType::from_llm(key);
            assert_eq!(rt.as_str(), key, "{key} must round-trip");
            assert!(rt.is_known(), "{key} must be known");
        }
    }

    #[test]
    fn rel_type_unknown_becomes_other_and_is_preserved() {
        let rt = RelType::from_llm("secretly_betrays");
        assert_eq!(rt, RelType::Other("secretly_betrays".to_string()));
        assert_eq!(rt.as_str(), "secretly_betrays");
        assert!(!rt.is_known());
        let (canon, flip) = rt.canonical();
        assert_eq!(canon, RelType::Other("secretly_betrays".to_string()));
        assert!(!flip);
        // Empty rel_type is a plausible degenerate LLM output: it must fall to
        // Other(""), never be treated as a known sentinel.
        assert_eq!(RelType::from_llm(""), RelType::Other(String::new()));
    }

    #[test]
    fn rel_type_inverse_normalizes_to_canonical_with_flip() {
        let cases = [
            ("led_by", "leads"),
            ("has_member", "member_of"),
            ("contains", "located_in"),
            ("owned_by", "owns"),
            ("served_by", "serves"),
            ("created_by", "created"),
            ("child_of", "parent_of"),
        ];
        for (inverse, canonical) in cases {
            let (canon, flip) = RelType::from_llm(inverse).canonical();
            assert_eq!(canon.as_str(), canonical, "{inverse} -> {canonical}");
            assert!(flip, "{inverse} must flip");
        }
    }

    #[test]
    fn rel_type_canonical_and_symmetric_do_not_flip() {
        for key in [
            "leads",
            "member_of",
            "allied_with",
            "enemy_of",
            "related_to",
            "knows",
        ] {
            let (canon, flip) = RelType::from_llm(key).canonical();
            assert_eq!(canon.as_str(), key);
            assert!(!flip, "{key} must not flip");
        }
    }
}
