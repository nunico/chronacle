use chronacle_lib::services::{
    campaign_service,
    collection_service::{
        add_campaign_collection, create, delete, get_all, get_by_id,
        get_campaign_collections, remove_campaign_collection, update,
    },
};
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

async fn make_campaign(db: &Surreal<Db>) -> campaign_service::Campaign {
    campaign_service::create(db, "Test Campaign", "D&D 5e")
        .await
        .unwrap()
}

// ── Test 1 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_collection_returns_correct_fields() {
    let db = setup_db().await;

    let col = create(&db, "Core Rules", Some("Main rulebooks"))
        .await
        .unwrap();

    assert_eq!(col.name, "Core Rules");
    assert_eq!(col.description.as_deref(), Some("Main rulebooks"));
    assert!(!col.id.is_empty());
}

// ── Test 2 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_collection_without_description() {
    let db = setup_db().await;

    let col = create(&db, "No Desc", None).await.unwrap();

    assert_eq!(col.name, "No Desc");
    assert!(col.description.is_none());
}

// ── Test 3 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_all_returns_all_collections() {
    let db = setup_db().await;

    create(&db, "Alpha", None).await.unwrap();
    create(&db, "Beta", None).await.unwrap();

    let all = get_all(&db).await.unwrap();

    assert_eq!(all.len(), 2);
    let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
    assert!(names.contains(&"Beta"));
}

// ── Test 4 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_by_id_returns_collection() {
    let db = setup_db().await;

    let created = create(&db, "Pathfinder 2e", Some("PF2e core")).await.unwrap();
    let fetched = get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Pathfinder 2e");
    assert_eq!(fetched.description.as_deref(), Some("PF2e core"));
}

// ── Test 5 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_collection_changes_name_and_description() {
    let db = setup_db().await;

    let created = create(&db, "Old Name", Some("Old desc")).await.unwrap();
    let updated = update(&db, &created.id, "New Name", Some("New desc"))
        .await
        .unwrap();

    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.description.as_deref(), Some("New desc"));
}

// ── Test 6 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_collection_removes_it() {
    let db = setup_db().await;

    let col = create(&db, "Temporary", None).await.unwrap();
    delete(&db, &col.id).await.unwrap();

    let all = get_all(&db).await.unwrap();
    assert!(!all.iter().any(|c| c.id == col.id));
}

// ── Test 7 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_collection_blocked_when_campaign_subscribed() {
    let db = setup_db().await;

    let col = create(&db, "Protected", None).await.unwrap();
    let campaign = make_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let result = delete(&db, &col.id).await;

    assert!(result.is_err());
    let msg = result.unwrap_err().to_lowercase();
    assert!(
        msg.contains("subscribed") || msg.contains("subscription"),
        "expected error message to contain 'subscribed' or 'subscription', got: {msg}"
    );
}

// ── Test 8 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn add_campaign_collection_creates_subscription() {
    let db = setup_db().await;

    let col = create(&db, "D&D 5e Core", None).await.unwrap();
    let campaign = make_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let cols = get_campaign_collections(&db, &campaign.id).await.unwrap();

    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].id, col.id);
}

// ── Test 9 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn add_campaign_collection_is_idempotent() {
    let db = setup_db().await;

    let col = create(&db, "Idempotent Test", None).await.unwrap();
    let campaign = make_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();
    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let cols = get_campaign_collections(&db, &campaign.id).await.unwrap();
    assert_eq!(cols.len(), 1, "expected exactly 1 entry, got {}", cols.len());
}

// ── Test 10 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn remove_campaign_collection_unsubscribes() {
    let db = setup_db().await;

    let col = create(&db, "To Remove", None).await.unwrap();
    let campaign = make_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();
    remove_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let cols = get_campaign_collections(&db, &campaign.id).await.unwrap();
    assert!(!cols.iter().any(|c| c.id == col.id));
}

// ── Test 11 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_campaign_collections_excludes_unsubscribed() {
    let db = setup_db().await;

    let col_a = create(&db, "Collection A", None).await.unwrap();
    let col_b = create(&db, "Collection B", None).await.unwrap();
    let campaign = make_campaign(&db).await;

    // Subscribe to col_a only
    add_campaign_collection(&db, &campaign.id, &col_a.id)
        .await
        .unwrap();

    let cols = get_campaign_collections(&db, &campaign.id).await.unwrap();

    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].id, col_a.id);
    assert!(
        !cols.iter().any(|c| c.id == col_b.id),
        "col_b must not appear in campaign collections"
    );
}
