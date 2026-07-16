//! Merge folds two records into one WITHOUT losing anything cheap to keep.
//!
//! The Maintenance inbox can DETECT a duplicate entity but, until this
//! operation existed, could never FIX one. Merge is the fix: it re-points every
//! edge onto the survivor, keeps the loser's name as an alias (so every
//! `[[Free League]]` link ever written keeps resolving), applies per-field
//! choices, marks the codex article stale, and soft-deletes the loser LAST.

use chronacle_extraction::codex_service::run_lint_campaign;
use chronacle_extraction::entity_service::{
    self, EntityInput, EntityKind, FieldChoice, MergeChoices,
};
use chronacle_extraction::wikilink::{parse_and_sync_wikilinks, WikilinkScope};

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

/// FINDING 1 regression: the survivor `UPDATE` binds `summary`/`notes` as raw
/// `Option<String>`, which serializes `None` to SurrealDB `NONE`. The schema
/// types both fields `string | NULL`, and `UPDATE` (unlike `CREATE`) never
/// applies `DEFAULT NULL`, so a kept side that is empty must not crash the
/// merge. Both entities here have `notes = NULL` and `KeepSurvivor` is chosen,
/// so the write must go through with `notes` staying `NULL` on the survivor.
#[tokio::test]
async fn merge_succeeds_when_the_kept_field_is_empty_on_both_sides() {
    let db = db().await;
    db.query(
        "CREATE faction:a SET name = 'The Free League', summary = 'Survivor summary.';
         CREATE faction:b SET name = 'Free League', summary = 'Loser summary.';",
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
    .expect("merge must succeed even when the kept `notes` field is empty on both sides");

    let notes: Vec<Option<String>> = db
        .query("SELECT VALUE notes FROM faction:a")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(
        notes[0], None,
        "the survivor's notes must stay NULL, not crash the write"
    );
}

// ── Soft-deleted-loser regression (tranche 6) ───────────────────────────────
//
// `query_all_entity_names`/`query_all_entity_notes` in `wikilink::query` feed
// wikilink resolution (tier-1/2/3 + tier-4 fuzzy, alias-collision lint) and
// the duplicate/staleness lint detectors. Before the fix, those SELECTs had
// no `vault_deleted != true` filter, so a merge's soft-deleted loser stayed
// visible everywhere: a `[[Free League]]` link would still resolve to the
// dead loser row (tier-1 exact match beating the survivor's tier-2 alias),
// and the lint pass would re-flag the pair the merge just resolved.

fn make_faction(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        aliases: None,
        summary: None,
        notes: None,
        date_start: None,
        date_end: None,
        is_ongoing: None,
        sequence_index: None,
        era: None,
        duration_label: None,
        session_id: None,
        player_name: None,
        character_class: None,
        character_level: None,
        status: None,
    }
}

async fn create_campaign(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query(
            "CREATE campaign SET name='Test Campaign', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    rows.into_iter().next().unwrap().id.id.to_raw()
}

/// MERGE -> RESOLUTION seam: after merging the loser into the survivor, a
/// `[[Free League]]` wikilink must resolve to the SURVIVOR, never the
/// soft-deleted loser.
#[tokio::test]
async fn wikilink_to_a_merged_away_name_resolves_to_the_survivor_not_the_soft_deleted_loser() {
    let db = db().await;
    let campaign_id = create_campaign(&db).await;

    let survivor = entity_service::create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Faction,
        make_faction("The Free League"),
    )
    .await
    .expect("create survivor");
    let loser = entity_service::create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Faction,
        make_faction("Free League"),
    )
    .await
    .expect("create loser");

    // Give each side an edge, matching the "with an edge on each" seeding
    // shape used elsewhere in this file.
    db.query("CREATE npc:x SET name = 'X'; CREATE npc:y SET name = 'Y';")
        .await
        .unwrap()
        .check()
        .unwrap();
    entity_service::relate(
        &db,
        &survivor.id,
        "faction",
        "x",
        "npc",
        "allied_with",
        None,
    )
    .await
    .expect("seed survivor edge");
    entity_service::relate(&db, &loser.id, "faction", "y", "npc", "enemy_of", None)
        .await
        .expect("seed loser edge");

    let survivor_full_id = format!("faction:{}", survivor.id);
    let loser_full_id = format!("faction:{}", loser.id);

    entity_service::merge(
        &db,
        &survivor_full_id,
        &loser_full_id,
        MergeChoices {
            summary: FieldChoice::KeepSurvivor,
            notes: FieldChoice::KeepSurvivor,
        },
    )
    .await
    .expect("merge");

    // A fresh source entity links back with the loser's old name.
    let source = entity_service::create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "SourceNPC".to_string(),
            aliases: None,
            summary: None,
            notes: None,
            date_start: None,
            date_end: None,
            is_ongoing: None,
            sequence_index: None,
            era: None,
            duration_label: None,
            session_id: None,
            player_name: None,
            character_class: None,
            character_level: None,
            status: None,
        },
    )
    .await
    .expect("create source npc");

    let result = parse_and_sync_wikilinks(
        &db,
        "npc",
        &source.id,
        "We deal with [[Free League]] now.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .expect("resolve wikilink");

    assert_eq!(
        result,
        vec![survivor_full_id.clone()],
        "the [[Free League]] link must resolve to the survivor {survivor_full_id}, \
         never the soft-deleted loser {loser_full_id}"
    );
}

/// MERGE -> LINT seam: after the merge, a lint pass must produce NO
/// `duplicate_entity` and NO `alias_collision` finding for the merged pair —
/// the loser is gone, so there is nothing left to pair against.
#[tokio::test]
async fn lint_pass_after_merge_does_not_resurrect_findings_for_the_soft_deleted_loser() {
    let db = db().await;
    let campaign_id = create_campaign(&db).await;

    let survivor = entity_service::create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Faction,
        make_faction("The Free League"),
    )
    .await
    .expect("create survivor");
    let loser = entity_service::create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Faction,
        make_faction("Free League"),
    )
    .await
    .expect("create loser");

    let survivor_full_id = format!("faction:{}", survivor.id);
    let loser_full_id = format!("faction:{}", loser.id);

    entity_service::merge(
        &db,
        &survivor_full_id,
        &loser_full_id,
        MergeChoices {
            summary: FieldChoice::KeepSurvivor,
            notes: FieldChoice::KeepSurvivor,
        },
    )
    .await
    .expect("merge");

    run_lint_campaign(&db, &campaign_id)
        .await
        .expect("lint pass");

    #[derive(Debug, serde::Deserialize)]
    struct Row {
        kind: String,
        payload: serde_json::Value,
    }
    let findings: Vec<Row> = db
        .query("SELECT kind, payload FROM lint_finding WHERE resolved_at = NONE")
        .await
        .unwrap()
        .take(0)
        .unwrap();

    let touches_the_merged_pair = |payload: &serde_json::Value| {
        let ids: Vec<String> = payload
            .as_object()
            .into_iter()
            .flat_map(|o| o.values())
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        ids.iter()
            .any(|id| id == &loser_full_id || id == &survivor_full_id)
    };

    let bad: Vec<&Row> = findings
        .iter()
        .filter(|r| {
            (r.kind == "duplicate_entity" || r.kind == "alias_collision")
                && touches_the_merged_pair(&r.payload)
        })
        .collect();

    assert!(
        bad.is_empty(),
        "lint must not resurrect duplicate_entity/alias_collision findings for \
         the merged pair, got {bad:?}",
    );
}

/// FINDING 2 regression: re-pointing a loser's edge onto the survivor must
/// carry the edge's free-text `notes` along, not drop them.
#[tokio::test]
async fn merge_preserves_edge_notes_when_repointing() {
    let db = seeded().await;
    db.query("CREATE npc:z SET name = 'Z';")
        .await
        .unwrap()
        .check()
        .unwrap();
    entity_service::relate(
        &db,
        "z",
        "npc",
        "b",
        "faction",
        "serves",
        Some("betrayed the party in session 4".to_string()),
    )
    .await
    .expect("seed annotated edge onto the loser");

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

    #[derive(serde::Deserialize)]
    struct NotesRow {
        notes: Option<String>,
    }
    let rows: Vec<NotesRow> = db
        .query("SELECT notes FROM relates_to WHERE in = npc:z AND out = faction:a")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "the re-pointed edge from npc:z to the survivor must exist"
    );
    assert_eq!(
        rows[0].notes.as_deref(),
        Some("betrayed the party in session 4"),
        "the edge's authored note must survive the re-point, not be dropped"
    );
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
