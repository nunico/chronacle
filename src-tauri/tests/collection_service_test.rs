use chronacle_domain::campaign_service;
use chronacle_domain::collection_service::{
    add_campaign_collection, create, delete, get_all, get_by_id, get_campaign_collections,
    remove_campaign_collection, update,
};
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

async fn create_test_campaign(db: &Surreal<Db>) -> campaign_service::Campaign {
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
async fn get_all_returns_collections_ordered_by_name() {
    let db = setup_db().await;

    create(&db, "Zzz", None).await.unwrap();
    create(&db, "Aaa", None).await.unwrap();

    let all = get_all(&db).await.unwrap();

    assert_eq!(all.len(), 2);
    assert_eq!(all[0].name, "Aaa");
    assert_eq!(all[1].name, "Zzz");
}

// ── Test 4 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_by_id_returns_collection() {
    let db = setup_db().await;

    let created = create(&db, "Pathfinder 2e", Some("PF2e core"))
        .await
        .unwrap();
    let fetched = get_by_id(&db, &created.id).await.unwrap();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Pathfinder 2e");
    assert_eq!(fetched.description.as_deref(), Some("PF2e core"));
}

// ── Test 5 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_by_id_not_found_returns_error() {
    let db = setup_db().await;

    let result = get_by_id(&db, "nonexistent_id").await;

    let err = result.unwrap_err();
    assert!(
        err.contains("not found"),
        "Expected not-found error, got: {err}"
    );
}

// ── Test 6 ───────────────────────────────────────────────────────────────────

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

// ── Test 7 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_collection_removes_it() {
    let db = setup_db().await;

    let col = create(&db, "Temporary", None).await.unwrap();
    delete(&db, &col.id).await.unwrap();

    let all = get_all(&db).await.unwrap();
    assert!(all.is_empty());
}

// ── Test 8 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_collection_blocked_when_campaign_subscribed() {
    let db = setup_db().await;

    let col = create(&db, "Protected", None).await.unwrap();
    let campaign = create_test_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let result = delete(&db, &col.id).await;

    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.contains("campaigns are subscribed"),
        "Expected 'campaigns are subscribed' error, got: {msg}"
    );
}

// ── Test 9 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_collection_blocked_when_source_exists() {
    let db = setup_db().await;
    let col = create(&db, "With Sources", None).await.unwrap();

    // Insert a source record that references this collection.
    // Must set campaign=NULL explicitly (SCHEMAFULL, no DEFAULT for this field in 001)
    // and indexed_at because it has no DEFAULT.
    db.query(
        "CREATE source SET campaign=NULL, \
         collection=type::thing('collection', $cid), \
         filename='test.pdf', display_name='Test', source_type='rules', \
         page_count=1, indexed_at=time::now(), index_status='done', \
         embed_model='nomic-embed-text-v1.5'",
    )
    .bind(("cid", col.id.clone()))
    .await
    .unwrap()
    .check()
    .expect("source INSERT must succeed for test precondition to hold");

    let result = delete(&db, &col.id).await;

    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.contains("sources exist"),
        "Expected 'sources exist' error, got: {msg}"
    );
}

// ── Test 10 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn add_campaign_collection_creates_subscription() {
    let db = setup_db().await;

    let col = create(&db, "D&D 5e Core", None).await.unwrap();
    let campaign = create_test_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let cols = get_campaign_collections(&db, &campaign.id).await.unwrap();

    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].id, col.id);
}

// ── Test 11 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn add_campaign_collection_is_idempotent() {
    let db = setup_db().await;

    let col = create(&db, "Idempotent Test", None).await.unwrap();
    let campaign = create_test_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();
    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let cols = get_campaign_collections(&db, &campaign.id).await.unwrap();
    assert_eq!(
        cols.len(),
        1,
        "expected exactly 1 entry, got {}",
        cols.len()
    );
}

// ── Test 12 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn remove_campaign_collection_unsubscribes() {
    let db = setup_db().await;

    let col = create(&db, "To Remove", None).await.unwrap();
    let campaign = create_test_campaign(&db).await;

    add_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();
    remove_campaign_collection(&db, &campaign.id, &col.id)
        .await
        .unwrap();

    let subscribed = get_campaign_collections(&db, &campaign.id).await.unwrap();
    assert!(subscribed.is_empty());
}

// ── Test 13 ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_campaign_collections_excludes_unsubscribed() {
    let db = setup_db().await;

    let col_a = create(&db, "Collection A", None).await.unwrap();
    let col_b = create(&db, "Collection B", None).await.unwrap();
    let campaign = create_test_campaign(&db).await;

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
