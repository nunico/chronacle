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
        aliases: Some(vec!["The Quassars".to_string(), "Quassar Clan".to_string()]),
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

/// FINDING 1 REGRESSION. `EntityInput.aliases: None` means "the caller has no
/// opinion" — e.g. the desktop entity form, which never sends the field at
/// all. `update()` must therefore PRESERVE whatever aliases are already
/// stored when `aliases` is `None`, not silently wipe them to `[]`.
#[tokio::test]
async fn aliases_survive_an_update_that_does_not_mention_them() {
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

    let node = entity_service::create(
        &db,
        Some("c1"),
        None,
        EntityKind::Faction,
        EntityInput {
            name: "The Quassar Family".to_string(),
            aliases: Some(vec!["The Quassars".to_string()]),
            ..Default::default()
        },
    )
    .await
    .expect("create");

    // Simulate the real desktop form payload: `aliases` is simply absent.
    let updated = entity_service::update(
        &db,
        &node.id,
        EntityKind::Faction,
        EntityInput {
            name: "The Quassar Family".to_string(),
            aliases: None,
            notes: Some("Updated notes from the GM.".to_string()),
            ..Default::default()
        },
    )
    .await
    .expect("update");

    assert_eq!(
        updated.aliases,
        vec!["The Quassars"],
        "an update that does not mention aliases must preserve them"
    );
    assert_eq!(updated.notes.as_deref(), Some("Updated notes from the GM."));
}

/// `Some(vec![])` is an explicit instruction to clear aliases, distinct from
/// `None` (no opinion). Both must be honored by `update()`.
#[tokio::test]
async fn some_empty_vec_explicitly_clears_aliases() {
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

    let node = entity_service::create(
        &db,
        Some("c1"),
        None,
        EntityKind::Faction,
        EntityInput {
            name: "The Quassar Family".to_string(),
            aliases: Some(vec!["The Quassars".to_string()]),
            ..Default::default()
        },
    )
    .await
    .expect("create");

    let updated = entity_service::update(
        &db,
        &node.id,
        EntityKind::Faction,
        EntityInput {
            name: "The Quassar Family".to_string(),
            aliases: Some(vec![]),
            ..Default::default()
        },
    )
    .await
    .expect("update");

    assert!(
        updated.aliases.is_empty(),
        "Some(vec![]) must explicitly clear aliases, got {:?}",
        updated.aliases
    );
}

/// FINDING 3 REGRESSION. `#[serde(default)]` on `EntityInput.aliases` must
/// keep working: a payload that omits `aliases` entirely (the real shape of
/// every IPC call from the desktop entity form today) must still deserialize.
/// If a future refactor removes `#[serde(default)]`, this test — not every
/// entity save at runtime — must be what breaks.
#[test]
fn entity_input_deserializes_with_aliases_omitted() {
    let input: EntityInput =
        serde_json::from_str(r#"{"name":"X"}"#).expect("aliases must be optional over IPC");
    assert_eq!(input.name, "X");
    assert_eq!(input.aliases, None);
}
