use chronacle_lib::services::campaign_service::{create, delete, get_all, get_by_id, update};
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

    delete(&db, &deleted_id).await.unwrap();

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
    let second = create(&db, "Beta Campaign", "Call of Cthulhu").await.unwrap();

    let all = get_all(&db).await.unwrap();

    assert!(
        all.len() >= 2,
        "expected at least 2 campaigns, got {}",
        all.len()
    );

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
