//! Merge folds two records into one WITHOUT losing anything cheap to keep.
//!
//! The Maintenance inbox can DETECT a duplicate entity but, until this
//! operation existed, could never FIX one. Merge is the fix: it re-points every
//! edge onto the survivor, keeps the loser's name as an alias (so every
//! `[[Free League]]` link ever written keeps resolving), applies per-field
//! choices, marks the codex article stale, and soft-deletes the loser LAST.

use chronacle_extraction::entity_service::{self, FieldChoice, MergeChoices};

async fn db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("in-memory db");
    db.use_ns("t").use_db("t").await.expect("use ns/db");
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db
}

/// Seed two factions that are the same organisation under two names, each with
/// its own edge, plus the two neighbours those edges point at.
///
/// - `faction:a` "The Free League"  --allied_with--> `npc:x` ("X")
/// - `faction:b` "Free League"      --enemy_of-->    `npc:y` ("Y")
async fn seeded() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = db().await;
    db.query(
        "CREATE faction:a SET name = 'The Free League', summary = 'Survivor summary.', \
             notes = 'Survivor notes.';
         CREATE faction:b SET name = 'Free League', summary = 'Loser summary.', \
             notes = 'Loser notes.';
         CREATE npc:x SET name = 'X';
         CREATE npc:y SET name = 'Y';",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    entity_service::relate(&db, "a", "faction", "x", "npc", "allied_with", None)
        .await
        .expect("seed survivor edge");
    entity_service::relate(&db, "b", "faction", "y", "npc", "enemy_of", None)
        .await
        .expect("seed loser edge");
    db
}

#[tokio::test]
async fn merge_unions_every_edge_and_keeps_the_losers_name_as_an_alias() {
    let db = seeded().await;

    entity_service::merge(
        &db,
        "faction:a",
        "faction:b",
        MergeChoices {
            summary: FieldChoice::KeepSurvivor,
            notes: FieldChoice::KeepBoth,
        },
    )
    .await
    .expect("merge");

    // NO EDGE IS EVER DROPPED. A relationship is a fact about the world, not a
    // stylistic preference — the survivor must know everything both knew.
    let related = entity_service::get_entity_relations(&db, "a", "faction")
        .await
        .unwrap();
    let names: Vec<&str> = related.iter().map(|r| r.name.as_str()).collect();
    assert!(
        names.contains(&"X"),
        "the survivor's own edge must survive, got {names:?}"
    );
    assert!(
        names.contains(&"Y"),
        "the loser's edge must be re-pointed, not dropped, got {names:?}"
    );

    // Every [[Free League]] link ever written must keep working.
    let aliases: Vec<Vec<String>> = db
        .query("SELECT VALUE aliases FROM faction:a")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert!(
        aliases[0].iter().any(|a| a == "Free League"),
        "the loser's name must become a survivor alias, got {:?}",
        aliases[0]
    );

    // KeepBoth concatenated, nothing silently destroyed.
    let notes: Vec<Option<String>> = db
        .query("SELECT VALUE notes FROM faction:a")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    let notes = notes[0].clone().unwrap();
    assert!(
        notes.contains("Merged from Free League"),
        "KeepBoth must concatenate under a heading, got {notes:?}"
    );
    assert!(notes.contains("Survivor notes."));
    assert!(notes.contains("Loser notes."));

    // The article was compiled from half the facts; it must be recompiled.
    let stale: Vec<bool> = db
        .query("SELECT VALUE codex_stale FROM faction:a")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert!(
        stale[0],
        "the survivor's codex article must be marked stale"
    );

    // The loser is soft-deleted, never hard-deleted.
    let deleted: Vec<bool> = db
        .query("SELECT VALUE vault_deleted FROM faction:b")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert!(deleted[0], "the loser must be soft-deleted");
}

/// Soft-delete, never hard-delete: the loser row must still EXIST after a merge
/// (its vault file is swept by the normal reconcile path, not a raw DELETE).
#[tokio::test]
async fn the_loser_is_soft_deleted_not_hard_deleted() {
    let db = seeded().await;

    entity_service::merge(
        &db,
        "faction:a",
        "faction:b",
        MergeChoices {
            summary: FieldChoice::KeepSurvivor,
            notes: FieldChoice::KeepSurvivor,
        },
    )
    .await
    .expect("merge");

    // The row is still physically present — a hard DELETE would return nothing.
    let ids: Vec<String> = db
        .query("SELECT VALUE name FROM faction:b")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(
        ids.len(),
        1,
        "the loser row must survive as a soft-deleted record, not be destroyed"
    );
}

/// KeepLoser and KeepSurvivor select one side wholesale; nothing is concatenated.
#[tokio::test]
async fn field_choices_select_the_named_side() {
    let db = seeded().await;

    entity_service::merge(
        &db,
        "faction:a",
        "faction:b",
        MergeChoices {
            summary: FieldChoice::KeepLoser,
            notes: FieldChoice::KeepSurvivor,
        },
    )
    .await
    .expect("merge");

    let summary: Vec<Option<String>> = db
        .query("SELECT VALUE summary FROM faction:a")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(summary[0].as_deref(), Some("Loser summary."));

    let notes: Vec<Option<String>> = db
        .query("SELECT VALUE notes FROM faction:a")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(notes[0].as_deref(), Some("Survivor notes."));
}

/// You cannot merge an npc into a location. Survivor and loser must share a
/// table, or the operation is rejected before it touches any data.
#[tokio::test]
async fn merge_rejects_a_cross_kind_pair() {
    let db = seeded().await;

    let err = entity_service::merge(
        &db,
        "faction:a",
        "npc:x",
        MergeChoices {
            summary: FieldChoice::KeepSurvivor,
            notes: FieldChoice::KeepSurvivor,
        },
    )
    .await
    .expect_err("a cross-kind merge must be rejected");

    // Nothing was destroyed: npc:x must still be live.
    let deleted: Vec<bool> = db
        .query("SELECT VALUE vault_deleted FROM npc:x")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert!(
        !deleted[0],
        "a rejected merge must not soft-delete anything"
    );
    let _ = err;
}

/// Merging a record into itself is nonsense and must be refused.
#[tokio::test]
async fn merge_rejects_survivor_equals_loser() {
    let db = seeded().await;

    let err = entity_service::merge(
        &db,
        "faction:a",
        "faction:a",
        MergeChoices {
            summary: FieldChoice::KeepSurvivor,
            notes: FieldChoice::KeepSurvivor,
        },
    )
    .await;

    assert!(err.is_err(), "merging a record into itself must be refused");
}

/// The `duplicate_entity` finding that flagged the pair must be resolved by the
/// merge — the Maintenance inbox should no longer show it.
#[tokio::test]
async fn merge_resolves_the_duplicate_entity_finding_for_the_pair() {
    let db = seeded().await;
    // The linter stores the pair sorted, as full record ids.
    db.query(
        "CREATE lint_finding SET kind = 'duplicate_entity', \
             payload = { a: 'faction:a', b: 'faction:b', similarity: 1.0 }, \
             created_at = time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    entity_service::merge(
        &db,
        "faction:a",
        "faction:b",
        MergeChoices {
            summary: FieldChoice::KeepSurvivor,
            notes: FieldChoice::KeepSurvivor,
        },
    )
    .await
    .expect("merge");

    let unresolved: Vec<surrealdb::sql::Thing> = db
        .query("SELECT VALUE id FROM lint_finding WHERE kind = 'duplicate_entity' AND resolved_at = NONE")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert!(
        unresolved.is_empty(),
        "the duplicate_entity finding for the merged pair must be resolved"
    );
}
