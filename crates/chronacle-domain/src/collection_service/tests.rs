use super::crud::{create, delete, get_all, get_by_id, update};
use super::subscriptions::{
    add_campaign_collection, get_campaign_collections, remove_campaign_collection,
};
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[tokio::test]
async fn create_and_get_all() {
    let db = setup().await;
    let c = create(&db, "D&D 5e Core", Some("Core rulebooks"))
        .await
        .unwrap();
    assert_eq!(c.name, "D&D 5e Core");
    assert_eq!(c.description.as_deref(), Some("Core rulebooks"));
    let all = get_all(&db).await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, c.id);
}

#[tokio::test]
async fn get_by_id_returns_collection() {
    let db = setup().await;
    let c = create(&db, "Pathfinder 2e", None).await.unwrap();
    let found = get_by_id(&db, &c.id).await.unwrap();
    assert_eq!(found.name, "Pathfinder 2e");
}

#[tokio::test]
async fn update_changes_name_and_description() {
    let db = setup().await;
    let c = create(&db, "Old Name", None).await.unwrap();
    let updated = update(&db, &c.id, "New Name", Some("desc")).await.unwrap();
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.description.as_deref(), Some("desc"));
}

#[tokio::test]
async fn delete_removes_collection() {
    let db = setup().await;
    let c = create(&db, "Temp", None).await.unwrap();
    delete(&db, &c.id).await.unwrap();
    let all = get_all(&db).await.unwrap();
    assert!(all.is_empty());
}

#[tokio::test]
async fn delete_blocked_when_source_exists() {
    let db = setup().await;
    let c = create(&db, "Protected", None).await.unwrap();
    // `campaign` is TYPE record<campaign> | NULL (no DEFAULT), so SCHEMAFULL
    // validation requires it to be set explicitly to NULL rather than omitted.
    db.query(
        "CREATE source SET id='s1', campaign=NULL, \
         collection=type::thing('collection', $cid), \
         filename='a.pdf', display_name='A', source_type='rules', page_count=0, \
         indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'",
    )
    .bind(("cid", c.id.clone()))
    .await
    .unwrap();
    let result = delete(&db, &c.id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("sources exist"));
}

#[tokio::test]
async fn delete_blocked_when_campaign_subscribed() {
    let db = setup().await;
    let c = create(&db, "Protected", None).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='Test Campaign', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    add_campaign_collection(&db, "camp1", &c.id).await.unwrap();
    let result = delete(&db, &c.id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("campaigns are subscribed"));
}

#[tokio::test]
async fn add_and_remove_campaign_collection() {
    let db = setup().await;
    let c = create(&db, "D&D 5e Core", None).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='My Campaign', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();

    add_campaign_collection(&db, "camp1", &c.id).await.unwrap();
    let cols = get_campaign_collections(&db, "camp1").await.unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].id, c.id);

    remove_campaign_collection(&db, "camp1", &c.id)
        .await
        .unwrap();
    let cols = get_campaign_collections(&db, "camp1").await.unwrap();
    assert!(cols.is_empty());
}

#[tokio::test]
async fn get_campaign_collections_returns_only_subscribed() {
    let db = setup().await;
    let c1 = create(&db, "D&D 5e Core", None).await.unwrap();
    let _c2 = create(&db, "Pathfinder 2e", None).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='My Campaign', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    add_campaign_collection(&db, "camp1", &c1.id).await.unwrap();

    let cols = get_campaign_collections(&db, "camp1").await.unwrap();
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].name, "D&D 5e Core");
}

#[tokio::test]
async fn add_campaign_collection_is_idempotent() {
    let db = setup().await;
    let c = create(&db, "D&D 5e Core", None).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='D&D', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    add_campaign_collection(&db, "camp1", &c.id).await.unwrap();
    add_campaign_collection(&db, "camp1", &c.id).await.unwrap(); // second call must succeed
    let cols = get_campaign_collections(&db, "camp1").await.unwrap();
    assert_eq!(cols.len(), 1); // must not return duplicates
}

#[tokio::test]
async fn get_by_id_missing_returns_err() {
    let db = setup().await;
    let result = get_by_id(&db, "does-not-exist").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[tokio::test]
async fn get_all_returns_ordered_by_name() {
    let db = setup().await;
    create(&db, "Zzz Collection", None).await.unwrap();
    create(&db, "Aaa Collection", None).await.unwrap();
    let all = get_all(&db).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].name, "Aaa Collection");
    assert_eq!(all[1].name, "Zzz Collection");
}
