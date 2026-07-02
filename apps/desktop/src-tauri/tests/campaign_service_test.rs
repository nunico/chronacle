use chronacle_domain::campaign_service::{
    create, delete, get_all, get_by_id, update, OnOwnedCollection,
};
use chronacle_domain::collection_service;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

// ── Test 1 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_campaign_changes_name_and_system() {
    let db = setup_db().await;

    let created = create(&db, "Original Name", "D&D 5e").await.unwrap();
    let original_id = created.id.clone();

    let updated = update(&db, &original_id, "Updated Name", "Pathfinder 2e")
        .await
        .unwrap();

    assert_eq!(updated.id, original_id, "id must not change after update");
    assert_eq!(updated.name, "Updated Name");
    assert_eq!(updated.system, "Pathfinder 2e");
}

// ── Test 2 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_campaign_removes_it_from_listing() {
    let db = setup_db().await;

    let campaign = create(&db, "To Be Deleted", "OSR").await.unwrap();
    let deleted_id = campaign.id.clone();

    // A1a: `delete` requires an explicit choice about the auto-created owned
    // collection. Use `Delete` here to match the pre-A1a behaviour of "remove
    // everything the campaign owns".
    delete(&db, &deleted_id, OnOwnedCollection::Delete)
        .await
        .unwrap();

    let all = get_all(&db).await.unwrap();
    let found = all.iter().any(|c| c.id == deleted_id);
    assert!(!found, "deleted campaign must not appear in get_all");
}

// ── Test 3 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_by_id_returns_correct_campaign() {
    let db = setup_db().await;

    let created = create(&db, "Exact Campaign", "Shadowrun").await.unwrap();

    let fetched = get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Exact Campaign");
    assert_eq!(fetched.system, "Shadowrun");
}

// ── Test 4 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_by_id_not_found_returns_error() {
    let db = setup_db().await;

    let result = get_by_id(&db, "nonexistent_id").await;

    assert!(result.is_err(), "expected Err for nonexistent id");
    let msg = result.unwrap_err();
    assert!(
        msg.to_lowercase().contains("not found"),
        "error message must contain 'not found', got: {msg}"
    );
}

// ── Test 5 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_all_returns_multiple_campaigns() {
    let db = setup_db().await;

    let first = create(&db, "Alpha Campaign", "D&D 5e").await.unwrap();
    let second = create(&db, "Beta Campaign", "Call of Cthulhu")
        .await
        .unwrap();

    let all = get_all(&db).await.unwrap();

    assert_eq!(all.len(), 2, "Expected exactly 2 campaigns");

    let ids: Vec<&str> = all.iter().map(|c| c.id.as_str()).collect();
    assert!(
        ids.contains(&first.id.as_str()),
        "get_all must contain first campaign"
    );
    assert!(
        ids.contains(&second.id.as_str()),
        "get_all must contain second campaign"
    );
}

// ── Test 6 ───────────────────────────────────────────────────────────────────

// NOTE: SurrealDB's UPDATE on a nonexistent record returns an empty result set.
// The `update` implementation maps that empty set to Err via `.ok_or_else(...)`,
// so this test expects is_err() = true.
#[tokio::test]
async fn update_nonexistent_campaign_returns_error() {
    let db = setup_db().await;

    let result = update(&db, "nonexistent_id", "Any Name", "Any System").await;

    assert!(
        result.is_err(),
        "expected Err when updating a nonexistent campaign"
    );
}

// ── LLM Wiki layer / PR-A1a: owned collections + two-mode delete ────────────
//
// See `docs/superpowers/specs/2026-07-02-compiled-world-model-a1-design.md`
// for the domain rationale. These tests specify the observable contract of the
// A1a slice:
//
//   * `create` auto-creates one owned collection per campaign,
//   * `collection_service::owned_by` finds it,
//   * `delete` with `Delete` tears it down cascade-style,
//   * `delete` with `ConvertToRegular` keeps it and orphans intra-only edges
//     while preserving edges that cross into regular collections.

use serde::Deserialize;

/// Helper: count rows on a table via `SELECT count() GROUP ALL`.
async fn count(db: &Surreal<Db>, query: &str) -> i64 {
    #[derive(Deserialize)]
    struct Row {
        count: i64,
    }
    let mut resp = db.query(query).await.expect("count query");
    let rows: Vec<Row> = resp.take(0).expect("parse count");
    rows.first().map(|r| r.count).unwrap_or(0)
}

#[tokio::test]
async fn creating_campaign_auto_creates_owned_collection_with_matching_name() {
    let db = setup_db().await;
    let campaign = create(&db, "Curse of the Crimson Throne", "Pathfinder 2e")
        .await
        .unwrap();

    let owned = collection_service::owned_by(&db, &campaign.id)
        .await
        .expect("owned_by must succeed");

    let owned = owned.expect("campaign must have an owned collection");
    assert_eq!(owned.name, "Curse of the Crimson Throne");
}

#[tokio::test]
async fn creating_campaign_auto_subscribes_to_owned_collection() {
    let db = setup_db().await;
    let campaign = create(&db, "Descent Into Avernus", "D&D 5e").await.unwrap();

    let owned = collection_service::owned_by(&db, &campaign.id)
        .await
        .unwrap()
        .expect("owned collection");

    // The `subscribes_to` edge must exist between the campaign and its owned
    // collection so all existing collection-list UIs pick it up automatically.
    let n = count(
        &db,
        &format!(
            "SELECT count() FROM subscribes_to \
             WHERE in  = type::thing('campaign',   '{cid}') \
               AND out = type::thing('collection', '{col}') GROUP ALL",
            cid = campaign.id,
            col = owned.id,
        ),
    )
    .await;
    assert_eq!(n, 1, "campaign must be subscribed to its owned collection");
}

#[tokio::test]
async fn owned_by_returns_none_when_campaign_has_no_owned_collection() {
    // Simulate a legacy campaign row that was created before A1a auto-create.
    let db = setup_db().await;
    db.query(
        "CREATE campaign SET \
            id = 'legacy', \
            name = 'Old Homebrew', \
            system = 'GURPS', \
            created_at = time::now(), \
            updated_at = time::now()",
    )
    .await
    .unwrap();

    let owned = collection_service::owned_by(&db, "legacy").await.unwrap();
    assert!(owned.is_none());
}

#[tokio::test]
async fn delete_cascade_removes_owned_collection() {
    let db = setup_db().await;
    let campaign = create(&db, "Rime of the Frostmaiden", "D&D 5e")
        .await
        .unwrap();
    let owned = collection_service::owned_by(&db, &campaign.id)
        .await
        .unwrap()
        .unwrap();

    delete(&db, &campaign.id, OnOwnedCollection::Delete)
        .await
        .unwrap();

    let n = count(
        &db,
        &format!(
            "SELECT count() FROM collection \
             WHERE id = type::thing('collection', '{col}') GROUP ALL",
            col = owned.id
        ),
    )
    .await;
    assert_eq!(n, 0, "cascade must remove the owned collection row");
}

#[tokio::test]
async fn delete_cascade_removes_entities_inside_owned_collection() {
    let db = setup_db().await;
    let campaign = create(&db, "Storm King's Thunder", "D&D 5e").await.unwrap();
    let owned = collection_service::owned_by(&db, &campaign.id)
        .await
        .unwrap()
        .unwrap();

    // Seed an NPC inside the owned collection, plus its scope edge.
    db.query(
        "CREATE npc SET \
            id = 'harshnag', name = 'Harshnag', \
            created_at = time::now(), updated_at = time::now(); \
         RELATE type::thing('collection', $col)->in_collection->npc:harshnag \
            SET created_at = time::now()",
    )
    .bind(("col", owned.id.clone()))
    .await
    .unwrap();

    delete(&db, &campaign.id, OnOwnedCollection::Delete)
        .await
        .unwrap();

    let n_npc = count(&db, "SELECT count() FROM npc:harshnag GROUP ALL").await;
    assert_eq!(
        n_npc, 0,
        "cascade must remove entities inside the collection"
    );
    let n_edge = count(&db, "SELECT count() FROM in_collection GROUP ALL").await;
    assert_eq!(n_edge, 0, "cascade must sweep the scope edge");
}

#[tokio::test]
async fn delete_cascade_leaves_regular_collections_untouched() {
    let db = setup_db().await;
    let campaign = create(&db, "Waterdeep: Dragon Heist", "D&D 5e")
        .await
        .unwrap();

    // Create a shared regular collection and subscribe the campaign to it.
    let regular = collection_service::create(&db, "Monster Manual", None)
        .await
        .unwrap();
    collection_service::add_campaign_collection(&db, &campaign.id, &regular.id)
        .await
        .unwrap();

    delete(&db, &campaign.id, OnOwnedCollection::Delete)
        .await
        .unwrap();

    let n = count(
        &db,
        &format!(
            "SELECT count() FROM collection \
             WHERE id = type::thing('collection', '{col}') GROUP ALL",
            col = regular.id
        ),
    )
    .await;
    assert_eq!(
        n, 1,
        "regular collections must survive cascade of another campaign"
    );
}

#[tokio::test]
async fn delete_convert_keeps_owned_collection_but_drops_owner_field() {
    let db = setup_db().await;
    let campaign = create(&db, "Wildemount Chronicles", "D&D 5e")
        .await
        .unwrap();
    let owned = collection_service::owned_by(&db, &campaign.id)
        .await
        .unwrap()
        .unwrap();

    delete(&db, &campaign.id, OnOwnedCollection::ConvertToRegular)
        .await
        .unwrap();

    // Collection still there.
    let n = count(
        &db,
        &format!(
            "SELECT count() FROM collection \
             WHERE id = type::thing('collection', '{col}') GROUP ALL",
            col = owned.id
        ),
    )
    .await;
    assert_eq!(n, 1, "convert must keep the collection row");

    // owner_campaign now NONE (it should no longer resolve for the campaign,
    // and the field itself is NONE regardless of whether the campaign row
    // still exists).
    let n_owned = count(
        &db,
        &format!(
            "SELECT count() FROM collection \
             WHERE id = type::thing('collection', '{col}') \
               AND owner_campaign = NONE GROUP ALL",
            col = owned.id
        ),
    )
    .await;
    assert_eq!(n_owned, 1, "owner_campaign must be cleared after convert");
}

#[tokio::test]
async fn delete_convert_orphans_only_intra_owned_edges_and_logs_findings() {
    let db = setup_db().await;
    let campaign = create(&db, "Tomb of Annihilation", "D&D 5e").await.unwrap();
    let owned = collection_service::owned_by(&db, &campaign.id)
        .await
        .unwrap()
        .unwrap();

    // Create a second (regular) collection with one NPC in it.
    let shared = collection_service::create(&db, "Chult Bestiary", None)
        .await
        .unwrap();

    // Two NPCs inside the owned collection, one NPC inside the shared one.
    db.query(
        "CREATE npc SET id = 'a', name = 'Artus Cimber', created_at = time::now(), updated_at = time::now(); \
         CREATE npc SET id = 'b', name = 'Dragonbait',   created_at = time::now(), updated_at = time::now(); \
         CREATE npc SET id = 's', name = 'T-Rex',        created_at = time::now(), updated_at = time::now(); \
         RELATE type::thing('collection', $col_owned) ->in_collection->npc:a SET created_at = time::now(); \
         RELATE type::thing('collection', $col_owned) ->in_collection->npc:b SET created_at = time::now(); \
         RELATE type::thing('collection', $col_shared)->in_collection->npc:s SET created_at = time::now(); \
         RELATE npc:a->relates_to->npc:b SET rel_type = 'allied_with', created_at = time::now(); \
         RELATE npc:a->relates_to->npc:s SET rel_type = 'hunts',       created_at = time::now();",
    )
    .bind(("col_owned",  owned.id.clone()))
    .bind(("col_shared", shared.id.clone()))
    .await
    .unwrap();

    delete(&db, &campaign.id, OnOwnedCollection::ConvertToRegular)
        .await
        .unwrap();

    // Intra edge (a<->b): dropped, one lint_finding.
    let intra = count(
        &db,
        "SELECT count() FROM relates_to WHERE in = npc:a AND out = npc:b GROUP ALL",
    )
    .await;
    assert_eq!(intra, 0, "intra-collection edge must be dropped");

    // Cross-scope edge (a->s): preserved.
    let cross = count(
        &db,
        "SELECT count() FROM relates_to WHERE in = npc:a AND out = npc:s GROUP ALL",
    )
    .await;
    assert_eq!(
        cross, 1,
        "edge crossing into a regular collection must be preserved"
    );

    // Exactly one lint_finding with kind = 'orphaned_edge'.
    let findings = count(
        &db,
        "SELECT count() FROM lint_finding WHERE kind = 'orphaned_edge' GROUP ALL",
    )
    .await;
    assert_eq!(
        findings, 1,
        "exactly one orphaned_edge finding must be recorded"
    );
}
