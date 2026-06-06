# Entity Manager Phase 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fully functional entity manager to Chronacle — eight typed graph node tables in SurrealDB, CRUD service, Tauri IPC commands with structured errors, and a per-type-tabs UI inside CampaignView.

**Architecture:** Eight separate SurrealDB tables (`npc`, `location`, `faction`, `creature`, `item`, `event`, `player_character`, `misc`) act as first-class graph nodes; the existing `relates_to` relation is updated to `FROM ANY TO ANY`. A single `entity_service.rs` dispatches via `EntityKind` enum, returning structured `EntityError` typed errors. Six Tauri commands wire the service to a new `EntityManager` component rendered as a second tab in `CampaignView`.

**Tech Stack:** Rust/Tauri 2, SurrealDB 2 (SurrealQL), thiserror 2, serde, Svelte 5, TypeScript, Vitest + @testing-library/svelte

---

## File Map

| File | Action |
|------|--------|
| `src-tauri/src/schema/004_graph_entities.surql` | CREATE — 8 node tables + updated `relates_to` |
| `src-tauri/src/schema/mod.rs` | MODIFY — add migration verification test for new tables |
| `src-tauri/src/services/entity_service.rs` | CREATE — EntityKind, EntityError, GraphNode, CRUD, relate |
| `src-tauri/src/services/mod.rs` | MODIFY — add `pub mod entity_service` |
| `src-tauri/src/commands/entity_commands.rs` | CREATE — 6 Tauri command handlers |
| `src-tauri/src/commands/mod.rs` | MODIFY — add `pub mod entity_commands; pub use entity_commands::*;` |
| `src-tauri/src/lib.rs` | MODIFY — add 6 entity commands to `generate_handler!` |
| `tests/entity_service_test.rs` | CREATE — integration tests |
| `src/lib/commands.ts` | MODIFY — Entity types + invoke wrappers |
| `src/components/EntityForm.svelte` | CREATE — type-discriminated create/edit form |
| `src/components/EntityManager.svelte` | CREATE — per-type sub-tabs + list + form dispatch |
| `src/views/CampaignView.svelte` | MODIFY — add Entities tab |

---

## Task 1: Schema Migration

**Files:**
- Create: `src-tauri/src/schema/004_graph_entities.surql`
- Modify: `src-tauri/src/schema/mod.rs`

- [ ] **Step 1: Write the migration file**

Create `src-tauri/src/schema/004_graph_entities.surql`:

```surql
-- 004_graph_entities.surql
-- Replaces the Phase 1 stub entity table with proper graph node tables.
-- Each entity type is its own SurrealDB table (graph node), allowing
-- type-safe fields and natural graph traversal via relates_to edges.

-- Drop stub table and old relation from Phase 1
REMOVE TABLE IF EXISTS entity;
REMOVE TABLE IF EXISTS relates_to;

-- Generic graph edge (connects any two node types)
DEFINE TABLE relates_to SCHEMAFULL TYPE RELATION FROM ANY TO ANY;
DEFINE FIELD rel_type   ON relates_to TYPE string;
DEFINE FIELD notes      ON relates_to TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON relates_to TYPE datetime DEFAULT time::now();

-- Shared fields macro (applied to every node table below)
-- Each table gets: campaign, name, summary, notes, created_at, updated_at

DEFINE TABLE npc SCHEMAFULL;
DEFINE FIELD campaign   ON npc TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON npc TYPE string;
DEFINE FIELD summary    ON npc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON npc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON npc TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON npc TYPE datetime DEFAULT time::now();

DEFINE TABLE location SCHEMAFULL;
DEFINE FIELD campaign   ON location TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON location TYPE string;
DEFINE FIELD summary    ON location TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON location TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON location TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON location TYPE datetime DEFAULT time::now();

DEFINE TABLE faction SCHEMAFULL;
DEFINE FIELD campaign   ON faction TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON faction TYPE string;
DEFINE FIELD summary    ON faction TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON faction TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON faction TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON faction TYPE datetime DEFAULT time::now();

DEFINE TABLE creature SCHEMAFULL;
DEFINE FIELD campaign   ON creature TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON creature TYPE string;
DEFINE FIELD summary    ON creature TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON creature TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON creature TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON creature TYPE datetime DEFAULT time::now();

DEFINE TABLE item SCHEMAFULL;
DEFINE FIELD campaign   ON item TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON item TYPE string;
DEFINE FIELD summary    ON item TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON item TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON item TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON item TYPE datetime DEFAULT time::now();

DEFINE TABLE event SCHEMAFULL;
DEFINE FIELD campaign        ON event TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name            ON event TYPE string;
DEFINE FIELD summary         ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes           ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD date_start      ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD date_end        ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD is_ongoing      ON event TYPE bool DEFAULT false;
DEFINE FIELD sequence_index  ON event TYPE int | NULL DEFAULT NULL;
DEFINE FIELD era             ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD duration_label  ON event TYPE string | NULL DEFAULT NULL;
DEFINE FIELD session         ON event TYPE record<session> | NULL DEFAULT NULL;
DEFINE FIELD created_at      ON event TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at      ON event TYPE datetime DEFAULT time::now();

DEFINE TABLE player_character SCHEMAFULL;
DEFINE FIELD campaign         ON player_character TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name             ON player_character TYPE string;
DEFINE FIELD summary          ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes            ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD player_name      ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD character_class  ON player_character TYPE string | NULL DEFAULT NULL;
DEFINE FIELD character_level  ON player_character TYPE int | NULL DEFAULT NULL;
DEFINE FIELD status           ON player_character TYPE string | NULL DEFAULT NULL
  ASSERT $value = NONE OR $value IN ['active', 'retired', 'deceased', 'missing', 'on_hiatus'];
DEFINE FIELD created_at       ON player_character TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at       ON player_character TYPE datetime DEFAULT time::now();

DEFINE TABLE misc SCHEMAFULL;
DEFINE FIELD campaign   ON misc TYPE record<campaign> | NULL DEFAULT NULL;
DEFINE FIELD name       ON misc TYPE string;
DEFINE FIELD summary    ON misc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD notes      ON misc TYPE string | NULL DEFAULT NULL;
DEFINE FIELD created_at ON misc TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON misc TYPE datetime DEFAULT time::now();
```

- [ ] **Step 2: Add migration verification test to `src-tauri/src/schema/mod.rs`**

Add this test to the existing `#[cfg(test)]` block at the bottom of `mod.rs`:

```rust
#[tokio::test]
async fn test_migration_004_graph_node_tables_exist() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("in-memory db");
    db.use_ns("test").use_db("test").await.unwrap();
    run_migrations(&db).await.expect("migrations");

    for table in &["npc", "location", "faction", "creature", "item", "event", "player_character", "misc"] {
        db.query(&format!("SELECT count() FROM {table} GROUP ALL"))
            .await
            .unwrap_or_else(|e| panic!("table {table} should exist: {e}"));
    }

    // relates_to accepts cross-type edges
    db.query("SELECT count() FROM relates_to GROUP ALL")
        .await
        .expect("relates_to should exist");

    // entity table should be gone
    let result = db.query("SELECT count() FROM entity GROUP ALL").await;
    assert!(result.is_err(), "entity table should have been removed");
}
```

- [ ] **Step 3: Run the test to verify it fails (entity table still exists)**

```bash
cd src-tauri && cargo test test_migration_004 -- --nocapture
```

Expected: FAIL — migration 004 doesn't exist yet; the entity table is still present.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd src-tauri && cargo test test_migration_004 -- --nocapture
```

Expected: PASS — all 8 tables exist, entity is gone.

- [ ] **Step 5: Also run the full schema test**

```bash
cd src-tauri && cargo test test_schema_runs_cleanly -- --nocapture
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/schema/004_graph_entities.surql src-tauri/src/schema/mod.rs
git commit -m "feat: add graph entity node tables in migration 004"
```

---

## Task 2: EntityKind, EntityError, GraphNode Types

**Files:**
- Create: `src-tauri/src/services/entity_service.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Add `pub mod entity_service;` to `src-tauri/src/services/mod.rs`**

Append to the existing list:

```rust
pub mod entity_service;
```

- [ ] **Step 2: Create `src-tauri/src/services/entity_service.rs` with types only**

```rust
use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::sql::Thing;
use surrealdb::Surreal;
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
```

- [ ] **Step 3: Run unit tests to verify they pass**

```bash
cd src-tauri && cargo test entity_kind -- --nocapture
```

Expected: PASS (3 tests)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/entity_service.rs src-tauri/src/services/mod.rs
git commit -m "feat: add EntityKind, EntityError, and GraphNode types"
```

---

## Task 3: entity_service — create and get

**Files:**
- Modify: `src-tauri/src/services/entity_service.rs`
- Create: `tests/entity_service_test.rs`

- [ ] **Step 1: Write the integration test file first**

Create `tests/entity_service_test.rs`:

```rust
use chronacle_lib::services::entity_service::{
    create, get_by_campaign, get_by_id, EntityInput, EntityKind,
};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_lib::schema::run_migrations(&db).await.unwrap();
    db
}

fn npc_input(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        summary: Some("A shady merchant".to_string()),
        notes: None,
        date_start: None, date_end: None, is_ongoing: None,
        sequence_index: None, era: None, duration_label: None,
        player_name: None, character_class: None,
        character_level: None, status: None,
    }
}

#[tokio::test]
async fn create_npc_returns_node_with_correct_kind() {
    let db = setup_db().await;
    let node = create(&db, None, EntityKind::Npc, npc_input("Torvin")).await.unwrap();
    assert_eq!(node.kind, "npc");
    assert_eq!(node.name, "Torvin");
    assert_eq!(node.summary.as_deref(), Some("A shady merchant"));
    assert!(node.campaign_id.is_none());
    assert!(!node.id.is_empty());
}

#[tokio::test]
async fn create_event_stores_temporal_fields() {
    let db = setup_db().await;
    let input = EntityInput {
        name: "Battle of the Ashfields".to_string(),
        summary: None, notes: None,
        date_start: Some("Year 312".to_string()),
        date_end: Some("Year 312".to_string()),
        is_ongoing: Some(false),
        sequence_index: Some(42),
        era: Some("Third Age".to_string()),
        duration_label: Some("3 days".to_string()),
        player_name: None, character_class: None,
        character_level: None, status: None,
    };
    let node = create(&db, None, EntityKind::Event, input).await.unwrap();
    assert_eq!(node.kind, "event");
    assert_eq!(node.date_start.as_deref(), Some("Year 312"));
    assert_eq!(node.sequence_index, Some(42));
    assert_eq!(node.era.as_deref(), Some("Third Age"));
}

#[tokio::test]
async fn create_player_character_stores_pc_fields() {
    let db = setup_db().await;
    let input = EntityInput {
        name: "Aeris".to_string(),
        summary: None, notes: None,
        date_start: None, date_end: None, is_ongoing: None,
        sequence_index: None, era: None, duration_label: None,
        player_name: Some("Alice".to_string()),
        character_class: Some("Wizard".to_string()),
        character_level: Some(7),
        status: Some("active".to_string()),
    };
    let node = create(&db, None, EntityKind::PlayerCharacter, input).await.unwrap();
    assert_eq!(node.kind, "player_character");
    assert_eq!(node.player_name.as_deref(), Some("Alice"));
    assert_eq!(node.character_level, Some(7));
    assert_eq!(node.status.as_deref(), Some("active"));
}

#[tokio::test]
async fn get_by_id_returns_created_node() {
    let db = setup_db().await;
    let created = create(&db, None, EntityKind::Location, EntityInput {
        name: "Shadowmere".to_string(),
        summary: None, notes: None,
        date_start: None, date_end: None, is_ongoing: None,
        sequence_index: None, era: None, duration_label: None,
        player_name: None, character_class: None,
        character_level: None, status: None,
    }).await.unwrap();

    let fetched = get_by_id(&db, &created.id, EntityKind::Location).await.unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Shadowmere");
}

#[tokio::test]
async fn get_by_id_not_found_returns_error() {
    use chronacle_lib::services::entity_service::EntityError;
    let db = setup_db().await;
    let err = get_by_id(&db, "nonexistent", EntityKind::Npc).await.unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}

#[tokio::test]
async fn get_by_campaign_returns_only_matching_entities() {
    let db = setup_db().await;

    // Create a campaign
    let campaign = chronacle_lib::services::campaign_service::create(&db, "Test Campaign", "D&D 5e")
        .await.unwrap();

    let n1 = create(&db, Some(&campaign.id), EntityKind::Npc, npc_input("Nym")).await.unwrap();
    let _n2 = create(&db, None, EntityKind::Npc, npc_input("Orphan NPC")).await.unwrap();

    let results = get_by_campaign(&db, &campaign.id, EntityKind::Npc).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, n1.id);
}
```

- [ ] **Step 2: Run the tests to confirm they fail (functions not yet implemented)**

```bash
cd src-tauri && cargo test --test entity_service_test 2>&1 | head -20
```

Expected: compile error — `create`, `get_by_id`, `get_by_campaign` not found.

- [ ] **Step 3: Implement `create`, `get_by_id`, and `get_by_campaign` in `entity_service.rs`**

Add these functions after the types section (before `#[cfg(test)]`):

```rust
/// Create a new graph node of the given kind scoped to an optional campaign.
pub async fn create(
    db: &Surreal<Db>,
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
                campaign     = IF $campaign_id != NONE THEN type::thing('campaign', $campaign_id) ELSE NULL END,
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
                created_at   = time::now(),
                updated_at   = time::now()",
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
        .map_err(|e| EntityError::Database(e.to_string()))?;
    let records: Vec<GraphNodeRecord> = response
        .take(0)
        .map_err(|e| EntityError::Database(e.to_string()))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| EntityError::Database("No record returned after create".to_string()))
}

/// Fetch a single node by its raw ID and kind.
pub async fn get_by_id(
    db: &Surreal<Db>,
    id: &str,
    kind: EntityKind,
) -> Result<GraphNode, EntityError> {
    let table = kind.table_name();
    let mut response = db
        .query("SELECT * FROM type::thing($table, $id)")
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database(e.to_string()))?;
    let records: Vec<GraphNodeRecord> = response
        .take(0)
        .map_err(|e| EntityError::Database(e.to_string()))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| EntityError::NotFound { id: id.to_string() })
}

/// List all nodes of a kind for a campaign, ordered by name.
pub async fn get_by_campaign(
    db: &Surreal<Db>,
    campaign_id: &str,
    kind: EntityKind,
) -> Result<Vec<GraphNode>, EntityError> {
    let table = kind.table_name();
    let mut response = db
        .query("SELECT * FROM type::table($table) WHERE campaign = type::thing('campaign', $campaign_id) ORDER BY name ASC")
        .bind(("table", table))
        .bind(("campaign_id", campaign_id.to_owned()))
        .await
        .map_err(|e| EntityError::Database(e.to_string()))?;
    let records: Vec<GraphNodeRecord> = response
        .take(0)
        .map_err(|e| EntityError::Database(e.to_string()))?;
    Ok(records.into_iter().map(Into::into).collect())
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd src-tauri && cargo test --test entity_service_test -- create get_by 2>&1
```

Expected: 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/entity_service.rs tests/entity_service_test.rs
git commit -m "feat: add entity_service create and get functions"
```

---

## Task 4: entity_service — update and delete

**Files:**
- Modify: `src-tauri/src/services/entity_service.rs`
- Modify: `tests/entity_service_test.rs`

- [ ] **Step 1: Add update and delete tests to `tests/entity_service_test.rs`**

Append:

```rust
#[tokio::test]
async fn update_changes_name_and_notes() {
    use chronacle_lib::services::entity_service::{update, EntityInput};
    let db = setup_db().await;
    let created = create(&db, None, EntityKind::Npc, npc_input("Old Name")).await.unwrap();

    let updated_input = EntityInput {
        name: "New Name".to_string(),
        summary: Some("Updated summary".to_string()),
        notes: Some("Some notes".to_string()),
        date_start: None, date_end: None, is_ongoing: None,
        sequence_index: None, era: None, duration_label: None,
        player_name: None, character_class: None,
        character_level: None, status: None,
    };
    let updated = update(&db, &created.id, EntityKind::Npc, updated_input).await.unwrap();
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.notes.as_deref(), Some("Some notes"));
}

#[tokio::test]
async fn update_not_found_returns_error() {
    use chronacle_lib::services::entity_service::{update, EntityError};
    let db = setup_db().await;
    let err = update(&db, "missing", EntityKind::Location, EntityInput {
        name: "Ghost".to_string(),
        summary: None, notes: None,
        date_start: None, date_end: None, is_ongoing: None,
        sequence_index: None, era: None, duration_label: None,
        player_name: None, character_class: None,
        character_level: None, status: None,
    }).await.unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}

#[tokio::test]
async fn delete_removes_node() {
    use chronacle_lib::services::entity_service::{delete, EntityError};
    let db = setup_db().await;
    let created = create(&db, None, EntityKind::Faction, EntityInput {
        name: "The Crimson Hand".to_string(),
        summary: None, notes: None,
        date_start: None, date_end: None, is_ongoing: None,
        sequence_index: None, era: None, duration_label: None,
        player_name: None, character_class: None,
        character_level: None, status: None,
    }).await.unwrap();

    delete(&db, &created.id, EntityKind::Faction).await.unwrap();

    let err = get_by_id(&db, &created.id, EntityKind::Faction).await.unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}
```

- [ ] **Step 2: Run to confirm failures**

```bash
cd src-tauri && cargo test --test entity_service_test -- update delete 2>&1 | head -10
```

Expected: compile error — `update` and `delete` not found.

- [ ] **Step 3: Implement `update` and `delete` in `entity_service.rs`**

Add after `get_by_campaign`:

```rust
/// Update an existing graph node. Returns NotFound if the record doesn't exist.
pub async fn update(
    db: &Surreal<Db>,
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
        .map_err(|e| EntityError::Database(e.to_string()))?;
    let records: Vec<GraphNodeRecord> = response
        .take(0)
        .map_err(|e| EntityError::Database(e.to_string()))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| EntityError::NotFound { id: id.to_string() })
}

/// Hard-delete a graph node by id.
pub async fn delete(
    db: &Surreal<Db>,
    id: &str,
    kind: EntityKind,
) -> Result<(), EntityError> {
    let table = kind.table_name();
    db.query("DELETE type::thing($table, $id)")
        .bind(("table", table))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 4: Run all entity service integration tests**

```bash
cd src-tauri && cargo test --test entity_service_test -- --nocapture
```

Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/entity_service.rs tests/entity_service_test.rs
git commit -m "feat: add entity_service update and delete"
```

---

## Task 5: entity_service — relate_entities

**Files:**
- Modify: `src-tauri/src/services/entity_service.rs`
- Modify: `tests/entity_service_test.rs`

- [ ] **Step 1: Add graph traversal test to `tests/entity_service_test.rs`**

Append:

```rust
#[tokio::test]
async fn relate_creates_edge_traversable_in_both_directions() {
    use chronacle_lib::services::entity_service::relate;
    let db = setup_db().await;

    let npc = create(&db, None, EntityKind::Npc, npc_input("Varek")).await.unwrap();
    let loc = create(&db, None, EntityKind::Location, EntityInput {
        name: "The Rusty Flagon".to_string(),
        summary: None, notes: None,
        date_start: None, date_end: None, is_ongoing: None,
        sequence_index: None, era: None, duration_label: None,
        player_name: None, character_class: None,
        character_level: None, status: None,
    }).await.unwrap();

    relate(&db, &npc.id, "npc", &loc.id, "location", "frequents", None)
        .await
        .unwrap();

    // Forward traversal: which locations does Varek frequent?
    #[derive(serde::Deserialize)]
    struct Row { id: surrealdb::sql::Thing }
    let mut resp = db
        .query("SELECT ->relates_to->location AS locs FROM type::thing('npc', $id)")
        .bind(("id", npc.id.clone()))
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct LocRow { locs: Vec<Row> }
    let rows: Vec<LocRow> = resp.take(0).unwrap();
    assert_eq!(rows[0].locs.len(), 1);
    assert_eq!(rows[0].locs[0].id.id.to_raw(), loc.id);
}
```

- [ ] **Step 2: Run to see it fail**

```bash
cd src-tauri && cargo test --test entity_service_test relate -- --nocapture 2>&1 | head -10
```

Expected: compile error — `relate` not found.

- [ ] **Step 3: Implement `relate` in `entity_service.rs`**

Add after `delete`:

```rust
/// Create a directed graph edge between two nodes.
///
/// `from_kind` and `to_kind` are the table names of the source and target nodes.
pub async fn relate(
    db: &Surreal<Db>,
    from_id: &str,
    from_kind: &str,
    to_id: &str,
    to_kind: &str,
    rel_type: &str,
    notes: Option<String>,
) -> Result<(), EntityError> {
    db.query(
        "RELATE type::thing($from_table, $from_id)->relates_to->type::thing($to_table, $to_id) SET
            rel_type = $rel_type,
            notes    = $notes,
            created_at = time::now()",
    )
    .bind(("from_table", from_kind.to_owned()))
    .bind(("from_id", from_id.to_owned()))
    .bind(("to_table", to_kind.to_owned()))
    .bind(("to_id", to_id.to_owned()))
    .bind(("rel_type", rel_type.to_owned()))
    .bind(("notes", notes))
    .await
    .map_err(|e| EntityError::Database(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 4: Run all tests**

```bash
cd src-tauri && cargo test --test entity_service_test -- --nocapture
```

Expected: All tests PASS (including the graph traversal).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/entity_service.rs tests/entity_service_test.rs
git commit -m "feat: add relate function for graph edge creation"
```

---

## Task 6: Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/entity_commands.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/commands/entity_commands.rs`**

```rust
use std::sync::Arc;
use tauri::State;

use crate::services::entity_service::{
    self, EntityError, EntityInput, EntityKind, GraphNode,
};
use crate::AppState;

fn parse_kind(kind: &str) -> Result<EntityKind, EntityError> {
    serde_json::from_value(serde_json::Value::String(kind.to_owned()))
        .map_err(|_| EntityError::InvalidKind { kind: kind.to_owned() })
}

#[tauri::command]
pub async fn get_entities(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    kind: String,
) -> Result<Vec<GraphNode>, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::get_by_campaign(&state.db, &campaign_id, k).await
}

#[tauri::command]
pub async fn get_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::get_by_id(&state.db, &id, k).await
}

#[tauri::command]
pub async fn create_entity(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::create(&state.db, Some(&campaign_id), k, input).await
}

#[tauri::command]
pub async fn update_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::update(&state.db, &id, k, input).await
}

#[tauri::command]
pub async fn delete_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
) -> Result<(), EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::delete(&state.db, &id, k).await
}

#[tauri::command]
pub async fn relate_entities(
    state: State<'_, Arc<AppState>>,
    from_id: String,
    from_kind: String,
    to_id: String,
    to_kind: String,
    rel_type: String,
    notes: Option<String>,
) -> Result<(), EntityError> {
    entity_service::relate(&state.db, &from_id, &from_kind, &to_id, &to_kind, &rel_type, notes)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kind_valid() {
        assert!(matches!(parse_kind("npc"), Ok(EntityKind::Npc)));
        assert!(matches!(parse_kind("player_character"), Ok(EntityKind::PlayerCharacter)));
    }

    #[test]
    fn parse_kind_invalid_returns_error() {
        let err = parse_kind("dragon").unwrap_err();
        assert!(matches!(err, EntityError::InvalidKind { kind } if kind == "dragon"));
    }
}
```

- [ ] **Step 2: Add `pub mod entity_commands; pub use entity_commands::*;` to `src-tauri/src/commands/mod.rs`**

At the top of `mod.rs`, after the existing use statements, add:

```rust
pub mod entity_commands;
pub use entity_commands::*;
```

- [ ] **Step 3: Register the 6 new commands in `src-tauri/src/lib.rs`**

Find the `tauri::generate_handler![` block and add after `commands::get_chunk_for_citation,`:

```rust
commands::get_entities,
commands::get_entity,
commands::create_entity,
commands::update_entity,
commands::delete_entity,
commands::relate_entities,
```

- [ ] **Step 4: Build to verify it compiles**

```bash
cd src-tauri && cargo build 2>&1 | grep -E "^error"
```

Expected: No errors.

- [ ] **Step 5: Run command unit tests**

```bash
cd src-tauri && cargo test entity_commands -- --nocapture
```

Expected: 2 tests PASS (`parse_kind_valid`, `parse_kind_invalid_returns_error`).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/entity_commands.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add entity Tauri commands and register handlers"
```

---

## Task 7: Frontend Types and Invoke Wrappers

**Files:**
- Modify: `src/lib/commands.ts`

- [ ] **Step 1: Add entity types and invoke wrappers to `src/lib/commands.ts`**

Append at the end of the file:

```typescript
// ── Entity Manager ───────────────────────────────────────────────────────────

export type EntityKind =
  | 'npc'
  | 'location'
  | 'faction'
  | 'creature'
  | 'item'
  | 'event'
  | 'player_character'
  | 'misc';

export interface GraphNode {
  id: string;
  kind: string;
  campaign_id: string | null;
  name: string;
  summary: string | null;
  notes: string | null;
  created_at: string | null;
  updated_at: string | null;
  // event fields
  date_start: string | null;
  date_end: string | null;
  is_ongoing: boolean | null;
  sequence_index: number | null;
  era: string | null;
  duration_label: string | null;
  // player_character fields
  player_name: string | null;
  character_class: string | null;
  character_level: number | null;
  status: 'active' | 'retired' | 'deceased' | 'missing' | 'on_hiatus' | null;
}

export interface EntityInput {
  name: string;
  summary?: string | null;
  notes?: string | null;
  // event
  dateStart?: string | null;
  dateEnd?: string | null;
  isOngoing?: boolean | null;
  sequenceIndex?: number | null;
  era?: string | null;
  durationLabel?: string | null;
  // player_character
  playerName?: string | null;
  characterClass?: string | null;
  characterLevel?: number | null;
  status?: string | null;
}

export interface EntityError {
  code: 'NOT_FOUND' | 'CAMPAIGN_MISMATCH' | 'INVALID_KIND' | 'VALIDATION' | 'DATABASE';
  message: string;
  field?: string; // present on VALIDATION errors
}

export async function getEntities(campaignId: string, kind: EntityKind): Promise<GraphNode[]> {
  return invoke<GraphNode[]>('get_entities', { campaignId, kind });
}

export async function getEntity(id: string, kind: EntityKind): Promise<GraphNode> {
  return invoke<GraphNode>('get_entity', { id, kind });
}

export async function createEntity(
  campaignId: string,
  kind: EntityKind,
  input: EntityInput,
): Promise<GraphNode> {
  return invoke<GraphNode>('create_entity', { campaignId, kind, input });
}

export async function updateEntity(
  id: string,
  kind: EntityKind,
  input: EntityInput,
): Promise<GraphNode> {
  return invoke<GraphNode>('update_entity', { id, kind, input });
}

export async function deleteEntity(id: string, kind: EntityKind): Promise<void> {
  return invoke<void>('delete_entity', { id, kind });
}

export async function relateEntities(
  fromId: string,
  fromKind: EntityKind,
  toId: string,
  toKind: EntityKind,
  relType: string,
  notes?: string | null,
): Promise<void> {
  return invoke<void>('relate_entities', { fromId, fromKind, toId, toKind, relType, notes });
}
```

- [ ] **Step 2: Run TypeScript type check**

```bash
pnpm typecheck 2>&1 | grep -i error | head -10
```

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/lib/commands.ts
git commit -m "feat: add entity TypeScript types and invoke wrappers"
```

---

## Task 8: EntityForm Component

**Files:**
- Create: `src/components/EntityForm.svelte`

- [ ] **Step 1: Write the Vitest test first**

Create `src/components/EntityForm.test.ts`:

```typescript
import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import EntityForm from './EntityForm.svelte';
import type { EntityKind, GraphNode } from '../lib/commands';

const mockNode = (overrides: Partial<GraphNode> = {}): GraphNode => ({
  id: 'abc',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
  summary: null,
  notes: null,
  created_at: null,
  updated_at: null,
  date_start: null, date_end: null, is_ongoing: null,
  sequence_index: null, era: null, duration_label: null,
  player_name: null, character_class: null,
  character_level: null, status: null,
  ...overrides,
});

describe('EntityForm', () => {
  it('renders name field for any entity kind', () => {
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null } });
    expect(screen.getByLabelText(/name/i)).toBeInTheDocument();
  });

  it('shows temporal fields for event kind', () => {
    render(EntityForm, { props: { kind: 'event' as EntityKind, node: null } });
    expect(screen.getByLabelText(/date start/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/sequence index/i)).toBeInTheDocument();
  });

  it('does NOT show temporal fields for npc kind', () => {
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node: null } });
    expect(screen.queryByLabelText(/date start/i)).not.toBeInTheDocument();
  });

  it('shows player fields for player_character kind', () => {
    render(EntityForm, { props: { kind: 'player_character' as EntityKind, node: null } });
    expect(screen.getByLabelText(/player name/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/character class/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/character level/i)).toBeInTheDocument();
  });

  it('pre-fills fields when editing an existing node', () => {
    const node = mockNode({ name: 'Vex', kind: 'npc' });
    render(EntityForm, { props: { kind: 'npc' as EntityKind, node } });
    expect((screen.getByLabelText(/name/i) as HTMLInputElement).value).toBe('Vex');
  });

  it('emits save event with input on submit', async () => {
    const onSave = vi.fn();
    const { component } = render(EntityForm, {
      props: { kind: 'npc' as EntityKind, node: null },
    });
    component.$on('save', onSave);
    await fireEvent.input(screen.getByLabelText(/name/i), { target: { value: 'New NPC' } });
    await fireEvent.submit(screen.getByRole('form'));
    expect(onSave).toHaveBeenCalledOnce();
    expect(onSave.mock.calls[0][0].detail.name).toBe('New NPC');
  });

  it('shows inline validation error when name is empty on submit', async () => {
    const { component } = render(EntityForm, {
      props: { kind: 'npc' as EntityKind, node: null },
    });
    component.$on('save', vi.fn());
    await fireEvent.submit(screen.getByRole('form'));
    expect(screen.getByText(/name is required/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests to confirm failure**

```bash
pnpm test --run src/components/EntityForm.test.ts 2>&1 | tail -5
```

Expected: FAIL — `EntityForm.svelte` not found.

- [ ] **Step 3: Create `src/components/EntityForm.svelte`**

```svelte
<script lang="ts">
  import type { EntityKind, GraphNode, EntityInput } from '../lib/commands';
  import { createEventDispatcher } from 'svelte';

  export let kind: EntityKind;
  export let node: GraphNode | null = null;
  export let error: { code: string; message: string; field?: string } | null = null;

  const dispatch = createEventDispatcher<{ save: EntityInput; cancel: void }>();

  let name = node?.name ?? '';
  let summary = node?.summary ?? '';
  let notes = node?.notes ?? '';
  // event fields
  let dateStart = node?.date_start ?? '';
  let dateEnd = node?.date_end ?? '';
  let isOngoing = node?.is_ongoing ?? false;
  let sequenceIndex = node?.sequence_index?.toString() ?? '';
  let era = node?.era ?? '';
  let durationLabel = node?.duration_label ?? '';
  // pc fields
  let playerName = node?.player_name ?? '';
  let characterClass = node?.character_class ?? '';
  let characterLevel = node?.character_level?.toString() ?? '';
  let status = node?.status ?? '';

  let nameError = '';

  function handleSubmit() {
    nameError = '';
    if (!name.trim()) {
      nameError = 'Name is required';
      return;
    }
    const input: EntityInput = {
      name: name.trim(),
      summary: summary || null,
      notes: notes || null,
      dateStart: dateStart || null,
      dateEnd: dateEnd || null,
      isOngoing: isOngoing || null,
      sequenceIndex: sequenceIndex ? parseInt(sequenceIndex, 10) : null,
      era: era || null,
      durationLabel: durationLabel || null,
      playerName: playerName || null,
      characterClass: characterClass || null,
      characterLevel: characterLevel ? parseInt(characterLevel, 10) : null,
      status: status || null,
    };
    dispatch('save', input);
  }
</script>

<form aria-label="entity form" on:submit|preventDefault={handleSubmit}>
  <div class="field">
    <label for="ef-name">Name</label>
    <input id="ef-name" type="text" bind:value={name} />
    {#if nameError}<p class="field-error">{nameError}</p>{/if}
    {#if error?.field === 'name'}<p class="field-error">{error.message}</p>{/if}
  </div>

  <div class="field">
    <label for="ef-summary">Summary</label>
    <input id="ef-summary" type="text" bind:value={summary} />
  </div>

  <div class="field">
    <label for="ef-notes">Notes</label>
    <textarea id="ef-notes" bind:value={notes} rows="4"></textarea>
  </div>

  {#if kind === 'event'}
    <div class="field">
      <label for="ef-date-start">Date Start</label>
      <input id="ef-date-start" type="text" bind:value={dateStart} />
    </div>
    <div class="field">
      <label for="ef-date-end">Date End</label>
      <input id="ef-date-end" type="text" bind:value={dateEnd} />
    </div>
    <div class="field">
      <label for="ef-seq">Sequence Index</label>
      <input id="ef-seq" type="number" bind:value={sequenceIndex} />
    </div>
    <div class="field">
      <label for="ef-era">Era</label>
      <input id="ef-era" type="text" bind:value={era} />
    </div>
    <div class="field">
      <label for="ef-dur">Duration Label</label>
      <input id="ef-dur" type="text" bind:value={durationLabel} />
    </div>
    <div class="field checkbox">
      <label>
        <input type="checkbox" bind:checked={isOngoing} />
        Ongoing
      </label>
    </div>
  {/if}

  {#if kind === 'player_character'}
    <div class="field">
      <label for="ef-player">Player Name</label>
      <input id="ef-player" type="text" bind:value={playerName} />
    </div>
    <div class="field">
      <label for="ef-class">Character Class</label>
      <input id="ef-class" type="text" bind:value={characterClass} />
    </div>
    <div class="field">
      <label for="ef-level">Character Level</label>
      <input id="ef-level" type="number" min="1" max="20" bind:value={characterLevel} />
    </div>
    <div class="field">
      <label for="ef-status">Status</label>
      <select id="ef-status" bind:value={status}>
        <option value="">— select —</option>
        <option value="active">Active</option>
        <option value="retired">Retired</option>
        <option value="deceased">Deceased</option>
        <option value="missing">Missing</option>
        <option value="on_hiatus">On Hiatus</option>
      </select>
    </div>
  {/if}

  {#if error && !error.field}
    <p class="form-error">{error.message}</p>
  {/if}

  <div class="actions">
    <button type="submit" class="btn-primary">{node ? 'Save' : 'Create'}</button>
    <button type="button" class="btn-ghost" on:click={() => dispatch('cancel')}>Cancel</button>
  </div>
</form>

<style>
  form { display: flex; flex-direction: column; gap: 12px; }
  .field { display: flex; flex-direction: column; gap: 4px; }
  label { font-size: 0.85rem; color: var(--text-secondary, #aaa); }
  input, textarea, select {
    background: var(--surface-2, #1e1e2e);
    border: 1px solid var(--border, #333);
    border-radius: 6px;
    color: var(--text-primary, #fff);
    padding: 6px 10px;
    font-size: 0.9rem;
  }
  .field-error, .form-error { color: var(--error, #f38ba8); font-size: 0.8rem; margin: 0; }
  .actions { display: flex; gap: 8px; margin-top: 8px; }
  .btn-primary {
    background: var(--accent, #cba6f7);
    color: #1e1e2e;
    border: none;
    border-radius: 6px;
    padding: 6px 16px;
    cursor: pointer;
    font-weight: 600;
  }
  .btn-ghost {
    background: transparent;
    color: var(--text-secondary, #aaa);
    border: 1px solid var(--border, #333);
    border-radius: 6px;
    padding: 6px 16px;
    cursor: pointer;
  }
</style>
```

- [ ] **Step 4: Run the form tests**

```bash
pnpm test --run src/components/EntityForm.test.ts
```

Expected: All 7 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/EntityForm.svelte src/components/EntityForm.test.ts
git commit -m "feat: add EntityForm component with type-discriminated fields"
```

---

## Task 9: EntityManager Component

**Files:**
- Create: `src/components/EntityManager.svelte`

- [ ] **Step 1: Write the Vitest test first**

Create `src/components/EntityManager.test.ts`:

```typescript
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import EntityManager from './EntityManager.svelte';
import type { GraphNode } from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getEntities: vi.fn().mockResolvedValue([]),
  createEntity: vi.fn(),
  updateEntity: vi.fn(),
  deleteEntity: vi.fn(),
}));

import * as commands from '../lib/commands';

const mockNpc = (): GraphNode => ({
  id: 'npc1',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
  summary: 'Shady merchant',
  notes: null,
  created_at: null, updated_at: null,
  date_start: null, date_end: null, is_ongoing: null,
  sequence_index: null, era: null, duration_label: null,
  player_name: null, character_class: null,
  character_level: null, status: null,
});

describe('EntityManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(commands.getEntities).mockResolvedValue([]);
  });

  it('renders 8 entity type tabs', () => {
    render(EntityManager, { props: { campaignId: 'camp1' } });
    for (const label of ['NPC', 'Location', 'Faction', 'Creature', 'Item', 'Event', 'PC', 'Misc']) {
      expect(screen.getByRole('tab', { name: new RegExp(label, 'i') })).toBeInTheDocument();
    }
  });

  it('loads NPC list on mount', async () => {
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    render(EntityManager, { props: { campaignId: 'camp1' } });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeInTheDocument());
    expect(commands.getEntities).toHaveBeenCalledWith('camp1', 'npc');
  });

  it('shows form when New button is clicked', async () => {
    render(EntityManager, { props: { campaignId: 'camp1' } });
    await waitFor(() => screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    expect(screen.getByLabelText(/name/i)).toBeInTheDocument();
  });

  it('shows toast on DATABASE error from createEntity', async () => {
    vi.mocked(commands.createEntity).mockRejectedValue({
      code: 'DATABASE', message: 'disk full',
    });
    render(EntityManager, { props: { campaignId: 'camp1' } });
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    // submit the form with a name
    await fireEvent.input(screen.getByLabelText(/name/i), { target: { value: 'Test' } });
    await fireEvent.submit(screen.getByRole('form'));
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('disk full'));
  });
});
```

- [ ] **Step 2: Run to confirm failure**

```bash
pnpm test --run src/components/EntityManager.test.ts 2>&1 | tail -5
```

Expected: FAIL — `EntityManager.svelte` not found.

- [ ] **Step 3: Create `src/components/EntityManager.svelte`**

```svelte
<script lang="ts">
  import {
    getEntities, createEntity, updateEntity, deleteEntity,
    type EntityKind, type GraphNode, type EntityInput, type EntityError,
  } from '../lib/commands';
  import EntityForm from './EntityForm.svelte';

  export let campaignId: string;

  type Tab = { kind: EntityKind; label: string };
  const TABS: Tab[] = [
    { kind: 'npc',              label: 'NPC' },
    { kind: 'location',         label: 'Location' },
    { kind: 'faction',          label: 'Faction' },
    { kind: 'creature',         label: 'Creature' },
    { kind: 'item',             label: 'Item' },
    { kind: 'event',            label: 'Event' },
    { kind: 'player_character', label: 'PC' },
    { kind: 'misc',             label: 'Misc' },
  ];

  let activeKind: EntityKind = 'npc';
  let entities: GraphNode[] = [];
  let loading = false;
  let formNode: GraphNode | null = null; // null = create, non-null = edit
  let showForm = false;
  let formError: EntityError | null = null;
  let toast: string | null = null;
  let deleteConfirm: GraphNode | null = null;

  async function loadEntities(kind: EntityKind) {
    loading = true;
    try {
      entities = await getEntities(campaignId, kind);
    } catch (e) {
      showToast((e as EntityError).message ?? 'Failed to load entities');
    } finally {
      loading = false;
    }
  }

  function selectTab(kind: EntityKind) {
    activeKind = kind;
    showForm = false;
    formNode = null;
    formError = null;
    loadEntities(kind);
  }

  function openCreate() {
    formNode = null;
    formError = null;
    showForm = true;
  }

  function openEdit(node: GraphNode) {
    formNode = node;
    formError = null;
    showForm = true;
  }

  async function handleSave(event: CustomEvent<EntityInput>) {
    formError = null;
    try {
      if (formNode) {
        const updated = await updateEntity(formNode.id, activeKind, event.detail);
        entities = entities.map(e => e.id === updated.id ? updated : e);
      } else {
        const created = await createEntity(campaignId, activeKind, event.detail);
        entities = [created, ...entities];
      }
      showForm = false;
    } catch (e) {
      const err = e as EntityError;
      if (err.code === 'VALIDATION') {
        formError = err;
      } else if (err.code === 'NOT_FOUND') {
        showToast('Entity no longer exists — refresh the list');
        showForm = false;
        await loadEntities(activeKind);
      } else {
        showToast(err.message ?? 'An error occurred');
      }
    }
  }

  async function confirmDelete(node: GraphNode) {
    try {
      await deleteEntity(node.id, activeKind);
      entities = entities.filter(e => e.id !== node.id);
    } catch (e) {
      showToast((e as EntityError).message ?? 'Failed to delete');
    } finally {
      deleteConfirm = null;
    }
  }

  function showToast(msg: string) {
    toast = msg;
    setTimeout(() => { toast = null; }, 4000);
  }

  // Load initial tab
  $: if (campaignId) loadEntities(activeKind);
</script>

<div class="entity-manager">
  <!-- Type tabs -->
  <div class="type-tabs" role="tablist">
    {#each TABS as tab}
      <button
        role="tab"
        aria-selected={activeKind === tab.kind}
        class="type-tab"
        class:active={activeKind === tab.kind}
        on:click={() => selectTab(tab.kind)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="content">
    <!-- List panel -->
    <div class="list-panel">
      <div class="list-header">
        <button class="btn-primary" on:click={openCreate}>
          + New {TABS.find(t => t.kind === activeKind)?.label}
        </button>
      </div>

      {#if loading}
        <p class="muted">Loading…</p>
      {:else if entities.length === 0}
        <p class="muted">No {TABS.find(t => t.kind === activeKind)?.label.toLowerCase()}s yet.</p>
      {:else}
        <ul class="entity-list">
          {#each entities as node (node.id)}
            <li class="entity-row" class:selected={formNode?.id === node.id}>
              <button class="entity-name" on:click={() => openEdit(node)}>{node.name}</button>
              <button
                class="btn-icon delete"
                aria-label="Delete {node.name}"
                on:click={() => { deleteConfirm = node; }}
              >×</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Form panel (slide in) -->
    {#if showForm}
      <div class="form-panel">
        <EntityForm
          kind={activeKind}
          node={formNode}
          error={formError}
          on:save={handleSave}
          on:cancel={() => { showForm = false; formNode = null; }}
        />
      </div>
    {/if}
  </div>

  <!-- Delete confirmation -->
  {#if deleteConfirm}
    <div class="overlay" role="dialog" aria-modal="true">
      <div class="confirm-box">
        <p>Delete <strong>{deleteConfirm.name}</strong>? This cannot be undone.</p>
        <div class="actions">
          <button class="btn-danger" on:click={() => confirmDelete(deleteConfirm!)}>Delete</button>
          <button class="btn-ghost" on:click={() => { deleteConfirm = null; }}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Toast -->
  {#if toast}
    <div class="toast" role="alert">{toast}</div>
  {/if}
</div>

<style>
  .entity-manager { display: flex; flex-direction: column; gap: 0; height: 100%; }
  .type-tabs { display: flex; gap: 2px; border-bottom: 1px solid var(--border, #333); padding: 0 8px; }
  .type-tab {
    background: none; border: none; color: var(--text-secondary, #aaa);
    padding: 8px 12px; cursor: pointer; font-size: 0.85rem; border-bottom: 2px solid transparent;
    margin-bottom: -1px;
  }
  .type-tab.active { color: var(--text-primary, #fff); border-bottom-color: var(--accent, #cba6f7); }
  .content { display: flex; flex: 1; overflow: hidden; }
  .list-panel { flex: 0 0 260px; border-right: 1px solid var(--border, #333); overflow-y: auto; display: flex; flex-direction: column; }
  .list-header { padding: 10px; border-bottom: 1px solid var(--border, #333); }
  .entity-list { list-style: none; margin: 0; padding: 0; }
  .entity-row {
    display: flex; align-items: center; gap: 4px; padding: 0 8px;
    border-bottom: 1px solid var(--border, #222);
  }
  .entity-row.selected { background: var(--surface-2, #1e1e2e); }
  .entity-name {
    flex: 1; background: none; border: none; color: var(--text-primary, #fff);
    text-align: left; padding: 10px 4px; cursor: pointer; font-size: 0.9rem;
  }
  .btn-icon { background: none; border: none; color: var(--text-tertiary, #666); cursor: pointer; font-size: 1rem; }
  .btn-icon.delete:hover { color: var(--error, #f38ba8); }
  .form-panel { flex: 1; padding: 16px; overflow-y: auto; }
  .muted { color: var(--text-secondary, #aaa); font-size: 0.85rem; padding: 16px; }
  .btn-primary {
    background: var(--accent, #cba6f7); color: #1e1e2e; border: none;
    border-radius: 6px; padding: 6px 12px; cursor: pointer; font-size: 0.85rem; font-weight: 600;
  }
  .overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.6);
    display: flex; align-items: center; justify-content: center; z-index: 100;
  }
  .confirm-box {
    background: var(--surface-1, #181825); border: 1px solid var(--border, #333);
    border-radius: 10px; padding: 20px; max-width: 360px; width: 90%;
  }
  .confirm-box p { margin: 0 0 16px; color: var(--text-primary, #fff); }
  .actions { display: flex; gap: 8px; }
  .btn-danger {
    background: var(--error, #f38ba8); color: #1e1e2e; border: none;
    border-radius: 6px; padding: 6px 14px; cursor: pointer; font-weight: 600;
  }
  .btn-ghost {
    background: transparent; color: var(--text-secondary, #aaa);
    border: 1px solid var(--border, #333); border-radius: 6px; padding: 6px 14px; cursor: pointer;
  }
  .toast {
    position: fixed; bottom: 20px; left: 50%; transform: translateX(-50%);
    background: var(--surface-2, #1e1e2e); color: var(--text-primary, #fff);
    border: 1px solid var(--border, #333); border-radius: 8px;
    padding: 10px 20px; z-index: 200; font-size: 0.9rem;
  }
</style>
```

- [ ] **Step 4: Run EntityManager tests**

```bash
pnpm test --run src/components/EntityManager.test.ts
```

Expected: All 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/EntityManager.svelte src/components/EntityManager.test.ts
git commit -m "feat: add EntityManager component with per-type tabs and CRUD"
```

---

## Task 10: Wire EntityManager into CampaignView

**Files:**
- Modify: `src/views/CampaignView.svelte`

- [ ] **Step 1: Find the current top-level tab or hero section in `CampaignView.svelte`**

Open `src/views/CampaignView.svelte` and locate the top-level view toggle (the "Manage campaigns" and "Collections" panels). The view currently has no tab UI at the top level — instead it has two collapsible panels.

Add a top-level tab switcher with two tabs: "Library" (existing content) and "Entities" (new EntityManager).

- [ ] **Step 2: Add the tab switcher and EntityManager import**

At the top of `<script>` in `CampaignView.svelte`, add:

```typescript
import EntityManager from '../components/EntityManager.svelte';

let activeTab: 'library' | 'entities' = 'library';

- [ ] **Step 3: Add tab buttons to the template**

After the campaign hero section (the section with gem icon, name, system), add:

```svelte
<div class="view-tabs" role="tablist">
  <button
    role="tab"
    aria-selected={activeTab === 'library'}
    class="view-tab"
    class:active={activeTab === 'library'}
    on:click={() => { activeTab = 'library'; }}
  >
    Library
  </button>
  <button
    role="tab"
    aria-selected={activeTab === 'entities'}
    class="view-tab"
    class:active={activeTab === 'entities'}
    on:click={() => { activeTab = 'entities'; }}
  >
    Entities
  </button>
</div>
```

- [ ] **Step 4: Conditionally render existing content and EntityManager**

Wrap the existing collections/campaign panels in `{#if activeTab === 'library'}` and add:

```svelte
{#if activeTab === 'library'}
  <!-- existing collections + campaign management panels -->
{:else if activeTab === 'entities' && active}
  <EntityManager campaignId={active.id} />
{:else if activeTab === 'entities'}
  <p class="muted">Select a campaign to manage entities.</p>
{/if}
```

- [ ] **Step 5: Add tab styles to CampaignView.svelte `<style>`**

```css
.view-tabs {
  display: flex;
  gap: 4px;
  padding: 0 16px;
  border-bottom: 1px solid var(--border, #333);
  margin-bottom: 8px;
}
.view-tab {
  background: none;
  border: none;
  color: var(--text-secondary, #aaa);
  padding: 10px 16px;
  cursor: pointer;
  font-size: 0.9rem;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
}
.view-tab.active {
  color: var(--text-primary, #fff);
  border-bottom-color: var(--accent, #cba6f7);
}
```

- [ ] **Step 6: Type-check and lint**

```bash
pnpm typecheck && pnpm lint
```

Expected: No errors.

- [ ] **Step 7: Run full test suite**

```bash
pnpm test --run && cd src-tauri && cargo test
```

Expected: All tests PASS.

- [ ] **Step 8: Commit**

```bash
git add src/views/CampaignView.svelte src/components/EntityManager.svelte
git commit -m "feat: wire EntityManager into CampaignView as Entities tab"
```

---

## Verification

1. `cargo test` — all Rust unit + integration tests pass
2. `pnpm test --run` — all Vitest tests pass
3. `pnpm typecheck` — no TypeScript errors
4. `cargo tauri dev` — open the app, navigate to CampaignView, click "Entities" tab
5. In the Entities tab: select "NPC" → click "New NPC" → fill name → Create → NPC appears in list
6. Click the NPC → edit the name → Save → name updates in list
7. Click × → confirm deletion → NPC removed
8. Repeat with "Event" kind → verify `Date Start` and `Sequence Index` fields appear
9. Repeat with "PC" kind → verify `Player Name`, `Character Class`, `Character Level` fields appear
10. Open SurrealDB (or run a query via a test) to confirm `SELECT * FROM npc` returns the created record
