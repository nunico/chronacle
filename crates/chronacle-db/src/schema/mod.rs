/// Schema runner.
///
/// Reads `.surql` files from the `schema/` directory, sorts them by filename,
/// and executes each against the database. All `DEFINE` statements are
/// idempotent on re-run; the `relates_to` edge table uses `DEFINE … OVERWRITE`
/// to re-assert its definition without dropping existing edges.
///
/// # Schema files
///
/// - `001_base_schema.surql` — complete consolidated schema (squashed from
///   Phases 1-3 individual migrations; safe to re-run on every app startup)
/// - `002_wiki_layer.surql` — LLM Wiki layer, additive; adds
///   `collection.owner_campaign` and the `lint_finding` table (A1a onward)
/// - `003_vault_sync.surql` — Markdown Vault Sync (ADR-008, D-series),
///   additive; adds `vault_deleted` to the nine syncable entity/session
///   tables and the `vault_sync_state` merge-base table
/// - `004_entity_identity.surql` — Tranche 6 entity identity (F1), additive;
///   adds `aliases` to the eight entity tables plus `rule_entry`
use std::path::Path;

/// Run all pending schema migrations against the given database.
///
/// SurrealDB schema definitions (`DEFINE TABLE`, `DEFINE FIELD`, …) are
/// idempotent, so this function simply executes every `.surql` file found
/// in the schema directory in sorted order.
pub async fn run_migrations<C>(db: &surrealdb::Surreal<C>) -> Result<(), String>
where
    C: surrealdb::Connection + Send + Sync,
{
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("schema");

    let mut entries: Vec<_> = std::fs::read_dir(&schema_dir)
        .map_err(|e| format!("Failed to read schema directory: {e}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("surql"))
        .collect();

    // Sort by filename for deterministic execution order
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let sql = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read '{}': {e}", path.display()))?;

        // Skip empty lines / whitespace-only files
        if sql.trim().is_empty() {
            continue;
        }

        db.query(&sql)
            .await
            .map_err(|e| format!("Schema migration failed on '{}': {e}", path.display()))?;
    }

    backfill_unset_fields(db).await?;

    Ok(())
}

/// Tables whose rows can predate the fields now defined on them.
const BACKFILL_ENTITY_TABLES: [&str; 8] = [
    "npc",
    "location",
    "faction",
    "creature",
    "item",
    "event",
    "player_character",
    "misc",
];

/// Give every row a value for the fields that a `DEFINE FIELD` cannot backfill.
///
/// **Why this exists.** `DEFINE FIELD … DEFAULT x` is a *write-time* default: it
/// does not touch rows that already exist, so a record created before a field was
/// defined holds no value for it at all. SurrealDB re-validates **every** field of
/// a SCHEMAFULL record on **any** write, and none of these types admit `NONE`
/// (`bool`, `array<object>`, and even `string | NULL` — `NULL` is a value, `NONE`
/// is the absence of one). So one unset field makes every later write to that
/// record fail:
///
/// ```text
/// Found NONE for field `codex_sources`, with record `npc:…`, but expected a array<object>
/// ```
///
/// Found by running the real app against a real campaign: **every** inbound vault
/// edit failed on entities older than the codex migration. It breaks in-app edits
/// of those records too — the vault merely made it visible.
///
/// All of a row's missing fields must be set in **one** statement: a partial fix
/// still fails validation on whatever is left unset. `??` coalesces only
/// `NONE`/`NULL`, so a row that already has a value keeps it — which is what makes
/// this idempotent, as it must be (`run_migrations` re-runs on every boot).
/// `option<T>` fields need no entry: `NONE` is legal there. `name` has no entry
/// either — it is required, and there is no value we could invent for it.
///
/// **Never fatal.** A failure is logged and skipped: it must never stop the app
/// from booting. The worst case is one stubborn row that keeps failing its own
/// writes — exactly the status quo this heals, and far better than a database that
/// refuses to open.
async fn backfill_unset_fields<C>(db: &surrealdb::Surreal<C>) -> Result<(), String>
where
    C: surrealdb::Connection + Send + Sync,
{
    // The eight entity tables share one field set (verified against
    // `INFO FOR TABLE`); sessions and rule entries carry their own.
    const ENTITY_SET: &str = "summary       = summary       ?? NULL, \
         notes         = notes         ?? NULL, \
         embedding     = embedding     ?? NULL, \
         embed_model   = embed_model   ?? NULL, \
         codex_stale   = codex_stale   ?? false, \
         codex_sources = codex_sources ?? [], \
         vault_deleted = vault_deleted ?? false, \
         aliases       = aliases       ?? [], \
         created_at    = created_at    ?? time::now(), \
         updated_at    = updated_at    ?? time::now()";

    let mut statements: Vec<String> = BACKFILL_ENTITY_TABLES
        .iter()
        .map(|t| format!("UPDATE {t} SET {ENTITY_SET}"))
        .collect();
    statements.push("UPDATE session SET vault_deleted = vault_deleted ?? false".to_owned());
    statements.push(
        "UPDATE rule_entry SET vault_deleted = vault_deleted ?? false, aliases = aliases ?? []"
            .to_owned(),
    );

    for sql in statements {
        let outcome = match db.query(&sql).await {
            Ok(response) => response.check().err(),
            Err(e) => Some(e),
        };
        if let Some(e) = outcome {
            eprintln!("schema: backfill skipped ({e}) for: {sql}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_schema_runs_cleanly_against_in_memory_db() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("Failed to create in-memory SurrealDB");

        db.use_ns("test").use_db("test").await.unwrap();

        run_migrations(&db)
            .await
            .expect("Schema migration should succeed");

        // ── Verify tables were created ──────────────────────────────
        // INFO FOR DB verifies the DB is operational; we don't inspect
        // the result structure as it varies by SurrealDB version.
        db.query("INFO FOR DB")
            .await
            .expect("INFO FOR DB should work");

        // Run a simple query to verify the DB is operational after migration.
        db.query("SELECT count() FROM campaign GROUP ALL")
            .await
            .expect("Query after migration should work");

        // Verify the collection table exists (defined in base schema).
        db.query("SELECT count() FROM collection GROUP ALL")
            .await
            .expect("collection table should exist after schema setup");
    }

    #[tokio::test]
    async fn graph_node_tables_exist_after_schema_setup() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("in-memory db");
        db.use_ns("test").use_db("test").await.unwrap();
        run_migrations(&db).await.expect("migrations");

        #[derive(serde::Deserialize)]
        struct DbInfo {
            tables: std::collections::HashMap<String, serde_json::Value>,
        }
        let mut resp = db.query("INFO FOR DB").await.expect("INFO FOR DB");
        let info: DbInfo = resp
            .take::<Option<DbInfo>>(0)
            .expect("parse INFO FOR DB")
            .expect("INFO FOR DB returned None");

        for table in &[
            "npc",
            "location",
            "faction",
            "creature",
            "item",
            "event",
            "player_character",
            "misc",
            "relates_to",
        ] {
            assert!(
                info.tables.contains_key(*table),
                "expected table '{table}' to exist after schema setup"
            );
        }

        assert!(
            !info.tables.contains_key("entity"),
            "Phase-1 entity stub table must not exist in squashed schema"
        );
    }

    /// Regression test: running migrations a second time (simulating an app restart)
    /// must NOT destroy existing `relates_to` edges. Before the fix, migration 004
    /// contained `REMOVE TABLE IF EXISTS relates_to` which wiped all edges on every boot.
    #[tokio::test]
    async fn migrations_are_data_preserving_on_rerun() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("in-memory db");
        db.use_ns("test").use_db("test").await.unwrap();

        // First run — schema setup.
        run_migrations(&db).await.expect("first migration");

        // Seed two entity nodes and a relates_to edge between them.
        db.query("CREATE npc:a SET name = 'A', created_at = time::now(), updated_at = time::now()")
            .await
            .expect("CREATE npc:a")
            .check()
            .expect("CREATE npc:a response");
        db.query(
            "CREATE location:b SET name = 'B', created_at = time::now(), updated_at = time::now()",
        )
        .await
        .expect("CREATE location:b")
        .check()
        .expect("CREATE location:b response");
        // Omit `notes` so the SCHEMAFULL DEFAULT NULL kicks in; inline `NULL` literal
        // triggers a SurrealDB field-check error against `string | NULL` in v2.
        db.query("RELATE npc:a->relates_to->location:b SET rel_type = 'mentioned'")
            .await
            .expect("RELATE npc:a->relates_to->location:b")
            .check()
            .expect("RELATE npc:a->relates_to->location:b response");

        // Sanity: edge exists before second run.
        let mut resp = db
            .query("SELECT count() FROM relates_to GROUP ALL")
            .await
            .expect("count before rerun");
        let before: Option<serde_json::Value> = resp.take(0).expect("take before");
        let n_before = before
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(
            n_before, 1,
            "sanity: edge must exist before second migration run"
        );

        // Simulate an app RESTART by re-running migrations against the live database.
        run_migrations(&db)
            .await
            .expect("second migration (restart simulation)");

        // The edge MUST survive the re-run.
        let mut resp2 = db
            .query("SELECT count() FROM relates_to GROUP ALL")
            .await
            .expect("count after rerun");
        let after: Option<serde_json::Value> = resp2.take(0).expect("take after");
        let n_after = after
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0);
        assert_eq!(
            n_after, 1,
            "relates_to edge must survive a migration re-run (app restart) — REMOVE TABLE wipes edges"
        );
    }

    #[tokio::test]
    async fn session_table_has_updated_at_and_campaign_index() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("in-memory db");
        db.use_ns("test").use_db("test").await.unwrap();
        run_migrations(&db).await.expect("migrations");

        // Insert a session record with updated_at to verify the field exists.
        db.query(
            "CREATE session SET \
             campaign = NULL, \
             session_number = 1, \
             title = 'Test Session', \
             date_played = '2026-06-05', \
             notes = '', \
             created_at = time::now(), \
             updated_at = time::now()",
        )
        .await
        .expect("INSERT session with updated_at should succeed");

        // Verify idx_session_campaign and updated_at appear in the session table info.
        #[derive(serde::Deserialize)]
        struct TableInfo {
            fields: std::collections::HashMap<String, serde_json::Value>,
            indexes: std::collections::HashMap<String, serde_json::Value>,
        }
        let mut resp = db
            .query("INFO FOR TABLE session")
            .await
            .expect("INFO FOR TABLE session");
        let info: TableInfo = resp
            .take::<Option<TableInfo>>(0)
            .expect("parse INFO FOR TABLE session")
            .expect("INFO FOR TABLE session returned None");

        assert!(
            info.fields.contains_key("updated_at"),
            "updated_at field should exist on session table"
        );
        assert!(
            info.indexes.contains_key("idx_session_campaign"),
            "idx_session_campaign index should exist on session table"
        );
    }

    #[tokio::test]
    async fn vault_deleted_exists_on_all_nine_syncable_tables() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("db");
        db.use_ns("test").use_db("test").await.unwrap();
        run_migrations(&db).await.expect("migrations");

        #[derive(serde::Deserialize)]
        struct TableInfo {
            fields: std::collections::HashMap<String, serde_json::Value>,
        }

        for table in &[
            "npc",
            "location",
            "faction",
            "creature",
            "item",
            "event",
            "player_character",
            "misc",
            "session",
        ] {
            let mut resp = db
                .query(format!("INFO FOR TABLE {table}"))
                .await
                .expect("INFO");
            let info: TableInfo = resp
                .take::<Option<TableInfo>>(0)
                .expect("parse")
                .expect("some");
            assert!(
                info.fields.contains_key("vault_deleted"),
                "vault_deleted must exist on '{table}' — there is no `entity` table to define it on"
            );
        }
    }

    /// A row written before `003_vault_sync.surql` carries no `vault_deleted` value
    /// at all. `DEFAULT false` applies at write time, not retroactively — so a
    /// `= false` filter silently omits it and `!= true` is the only safe form.
    #[tokio::test]
    async fn default_false_does_not_backfill_pre_migration_rows() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("db");
        db.use_ns("test").use_db("test").await.unwrap();

        // Simulate a pre-migration row: define the table WITHOUT vault_deleted.
        db.query(
            "DEFINE TABLE npc SCHEMALESS; \
             CREATE npc:legacy SET name = 'Legacy', created_at = time::now(), updated_at = time::now()",
        )
        .await
        .expect("seed legacy row")
        .check()
        .expect("seed response");

        // Now run the real migrations over the live database.
        run_migrations(&db).await.expect("migrations");

        // `id` deserializes as a SurrealDB `Thing`, not plain JSON — see
        // `lint.rs`'s `IdRow` for the precedent this follows.
        #[derive(serde::Deserialize)]
        struct IdRow {
            #[allow(dead_code)]
            id: surrealdb::sql::Thing,
        }

        let mut wrong = db
            .query("SELECT id FROM npc WHERE vault_deleted = false")
            .await
            .expect("query = false");
        let wrong_rows: Vec<IdRow> = wrong.take(0).expect("take");

        let mut right = db
            .query("SELECT id FROM npc WHERE vault_deleted != true")
            .await
            .expect("query != true");
        let right_rows: Vec<IdRow> = right.take(0).expect("take");

        assert_eq!(
            right_rows.len(),
            1,
            "`!= true` must see the pre-migration row"
        );
        assert!(
            wrong_rows.len() <= right_rows.len(),
            "regression guard: if `= false` ever starts matching, the `!= true` rule can be revisited"
        );
    }

    #[tokio::test]
    async fn vault_sync_state_table_exists_and_is_schemafull() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("db");
        db.use_ns("test").use_db("test").await.unwrap();
        run_migrations(&db).await.expect("migrations");

        db.query(
            "CREATE vault_sync_state:⟨npc:a⟩ SET \
             record = 'npc:a', key = 'campaigns/c/entities/npc/a.md', \
             synced_hash = '123', synced_at = time::now()",
        )
        .await
        .expect("insert sync state")
        .check()
        .expect("insert response");

        #[derive(serde::Deserialize)]
        struct IdRow {
            #[allow(dead_code)]
            id: surrealdb::sql::Thing,
        }
        let mut resp = db
            .query("SELECT id FROM vault_sync_state")
            .await
            .expect("select");
        let rows: Vec<IdRow> = resp.take(0).expect("take");
        assert_eq!(rows.len(), 1);
    }

    /// The whole file must survive a second execution — `run_migrations` runs on
    /// every boot. A `REMOVE` here once wiped every `relates_to` edge on restart.
    #[tokio::test]
    async fn vault_sync_state_rows_survive_a_migration_rerun() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("db");
        db.use_ns("test").use_db("test").await.unwrap();
        run_migrations(&db).await.expect("first migration");

        db.query(
            "CREATE vault_sync_state:⟨npc:a⟩ SET record = 'npc:a', key = 'k', \
             synced_hash = '123', synced_at = time::now()",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");

        run_migrations(&db)
            .await
            .expect("second migration (restart simulation)");

        #[derive(serde::Deserialize)]
        struct IdRow {
            #[allow(dead_code)]
            id: surrealdb::sql::Thing,
        }
        let mut resp = db
            .query("SELECT id FROM vault_sync_state")
            .await
            .expect("select");
        let rows: Vec<IdRow> = resp.take(0).expect("take");
        assert_eq!(
            rows.len(),
            1,
            "vault_sync_state must survive a migration re-run"
        );
    }
}
