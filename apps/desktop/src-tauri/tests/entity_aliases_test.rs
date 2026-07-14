//! `aliases` must round-trip through create/update/read — AND must not break
//! writes to rows that predate the migration (the DEFAULT landmine).

use chronacle_extraction::entity_service::{self, EntityInput, EntityKind};

async fn db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("in-memory db");
    db.use_ns("t").use_db("t").await.expect("use ns/db");
    db
}

#[tokio::test]
async fn aliases_round_trip_through_create_and_read() {
    let db = db().await;
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db.query(
        "CREATE campaign:c1 SET name = 'SoV', system = '5e', \
              created_at = time::now(), updated_at = time::now()",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    let input = EntityInput {
        name: "The Quassar Family".to_string(),
        aliases: vec!["The Quassars".to_string(), "Quassar Clan".to_string()],
        ..Default::default()
    };
    let node = entity_service::create(&db, Some("c1"), None, EntityKind::Faction, input)
        .await
        .expect("create");

    let read = entity_service::get_by_id(&db, &node.id, EntityKind::Faction)
        .await
        .expect("read");
    assert_eq!(read.aliases, vec!["The Quassars", "Quassar Clan"]);
}

/// THE LANDMINE TEST. `DEFINE FIELD ... DEFAULT []` is a WRITE-time default: it
/// never touches rows that already exist. SurrealDB re-validates EVERY field of
/// a SCHEMAFULL record on ANY write, and `NONE` does not satisfy
/// `array<string>` — so a single unset field makes every LATER write to that
/// row fail. Seeding BEFORE run_migrations is the only way to reproduce a real
/// user's pre-migration row; fresh fixtures pick up the DEFAULT and are blind
/// to this. (Tranche 5 shipped this bug green.)
#[tokio::test]
async fn a_pre_migration_row_can_still_be_written_to() {
    let db = db().await;

    // Seeded BEFORE migrations: this row has no `aliases` value at all.
    db.query(
        "DEFINE TABLE npc SCHEMALESS; \
              CREATE npc:old SET name = 'Seraphina';",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    chronacle_db::run_migrations(&db).await.expect("migrations");

    // The write that would fail with: Found NONE for field `aliases`,
    // with record `npc:old`, but expected a array<string>
    db.query("UPDATE npc:old SET notes = 'edited'")
        .await
        .expect("query")
        .check()
        .expect("a pre-migration row must still accept writes after migrating");
}
