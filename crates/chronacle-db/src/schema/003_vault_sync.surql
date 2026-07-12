-- ── Vault sync (ADR-008, D-series) ───────────────────────────────────────────
-- DEFINE-only. `run_migrations` re-executes this file on every boot; a REMOVE
-- here would wipe live data on every restart.
--
-- NOTE: `DEFAULT false` does NOT backfill rows written before this migration.
-- Every read path must filter `vault_deleted != true`, never `= false`.

DEFINE FIELD OVERWRITE vault_deleted ON TABLE npc              TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE location         TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE faction          TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE creature         TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE item             TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE event            TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE player_character TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE misc             TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE session          TYPE bool DEFAULT false;

-- ── vault_sync_state: the three-way merge base ───────────────────────────────
-- One row per synced record, id = the record's thing string. `synced_hash` is a
-- u64 from std DefaultHasher, stored as a string because SurrealDB `int` is i64.
-- SCHEMAFULL with typed fields — no FLEXIBLE object, no serde_json::Value binds.

DEFINE TABLE OVERWRITE vault_sync_state SCHEMAFULL;
DEFINE FIELD OVERWRITE record      ON TABLE vault_sync_state TYPE string;
DEFINE FIELD OVERWRITE key         ON TABLE vault_sync_state TYPE string;
DEFINE FIELD OVERWRITE synced_hash ON TABLE vault_sync_state TYPE string;
DEFINE FIELD OVERWRITE synced_at   ON TABLE vault_sync_state TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE vault_sync_state_record_idx ON TABLE vault_sync_state COLUMNS record UNIQUE;

-- ── Tranche 5: inbound ────────────────────────────────────────────────────────
-- `synced_hash` gains DEFAULT '' so a conflict-only row (set_conflict before any
-- base exists) can be created; '' parses to no base (None) on read.
DEFINE FIELD OVERWRITE synced_hash ON TABLE vault_sync_state TYPE string DEFAULT '';
-- Frozen-conflict marker: while true AND the sidecar exists, reconcile neither
-- applies nor exports this record.
DEFINE FIELD OVERWRITE conflict ON TABLE vault_sync_state TYPE bool DEFAULT false;
-- Rule entries become soft-deletable like the other nine tables.
DEFINE FIELD OVERWRITE vault_deleted ON TABLE rule_entry TYPE bool DEFAULT false;
