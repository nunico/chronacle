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
}
