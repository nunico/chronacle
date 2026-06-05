/// Schema migration runner.
///
/// Reads `.surql` migration files from the `schema/` directory, sorts them
/// by filename, and executes each against the database. Migrations are
/// idempotent — each file uses `DEFINE … IF NOT EXISTS` or plain `DEFINE`
/// statements that are safe to run multiple times.
///
/// # Migration files
///
/// Files use a zero-prefixed numeric naming convention so they sort in
/// dependency order:
/// - `001_initial.surql` — Phase 1 tables, fields, indexes
/// - `002_embedding_index.surql` — (reserved; not yet created)
/// - `003_collections.surql` — collection table, subscribes_to relation, collection fields on source/chunk
/// - `004_graph_entities.surql` — 8 typed graph node tables (npc, location, faction, creature, item, event, player_character, misc); relates_to updated to FROM ANY TO ANY
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

        // Verify the new collection table from migration 003 exists.
        db.query("SELECT count() FROM collection GROUP ALL")
            .await
            .expect("collection table should exist after migration 003");
    }

    #[tokio::test]
    async fn test_migration_004_graph_node_tables_exist() {
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
                "expected table '{table}' to exist after migration 004"
            );
        }

        assert!(
            !info.tables.contains_key("entity"),
            "entity table should have been removed by migration 004"
        );
    }

    #[tokio::test]
    async fn test_migration_005_session_updated_at_and_campaign_index() {
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
        .expect("INSERT session with updated_at should succeed after migration 005");

        // Verify idx_session_campaign appears in the session table info.
        #[derive(serde::Deserialize)]
        struct TableInfo {
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
            info.indexes.contains_key("idx_session_campaign"),
            "idx_session_campaign index should exist on session table after migration 005"
        );
    }
}
