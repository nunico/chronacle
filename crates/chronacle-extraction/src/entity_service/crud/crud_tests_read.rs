use crate::entity_service::{EntityInput, EntityKind};

use super::{create, find_by_name_and_collection, get_by_campaign, get_by_collection, get_by_id};

async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

fn input(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn get_by_campaign_returns_only_campaign_entities() {
    let db = setup_db().await;
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now(); \
         CREATE collection SET id='col1', name='PHB', description=NULL, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        input("Torvin"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    create(
        &db,
        None,
        Some("col1"),
        EntityKind::Npc,
        input("Goblin"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let results = get_by_campaign(&db, "camp1", EntityKind::Npc)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Torvin");
}

/// Regression: entities extracted from a rulebook are collection-scoped.
/// The campaign entity browser must surface them when the campaign subscribes_to
/// that collection.
#[tokio::test]
async fn get_by_campaign_includes_subscribed_collection_entities() {
    let db = setup_db().await;
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now(); \
         CREATE collection SET id='col1', name='PHB', description=NULL, \
         created_at=time::now(), updated_at=time::now(); \
         CREATE collection SET id='col2', name='DMG', description=NULL, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "LET $in = type::thing('campaign','camp1'); \
         LET $out1 = type::thing('collection','col1'); \
         RELATE $in->subscribes_to->$out1 SET created_at=time::now()",
    )
    .await
    .unwrap();
    create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        input("Torvin"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    create(
        &db,
        None,
        Some("col1"),
        EntityKind::Npc,
        input("Goblin"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    create(
        &db,
        None,
        Some("col2"),
        EntityKind::Npc,
        input("Lich"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let results = get_by_campaign(&db, "camp1", EntityKind::Npc)
        .await
        .unwrap();
    let names: Vec<_> = results.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["Goblin", "Torvin"], "ordered by name ASC");
}

#[tokio::test]
async fn get_by_collection_returns_only_collection_entities() {
    let db = setup_db().await;
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now(); \
         CREATE collection SET id='col1', name='PHB', description=NULL, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        input("Torvin"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    create(
        &db,
        None,
        Some("col1"),
        EntityKind::Npc,
        input("Goblin"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let results = get_by_collection(&db, "col1", EntityKind::Npc)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Goblin");
}

#[tokio::test]
async fn find_by_name_and_collection_is_case_insensitive() {
    let db = setup_db().await;
    db.query(
        "CREATE collection SET id='col1', name='PHB', description=NULL, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    create(
        &db,
        None,
        Some("col1"),
        EntityKind::Npc,
        input("The Iron Fist"),
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let found = find_by_name_and_collection(&db, "col1", "the iron fist", EntityKind::Npc)
        .await
        .unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "The Iron Fist");
    let not_found = find_by_name_and_collection(&db, "col1", "other", EntityKind::Npc)
        .await
        .unwrap();
    assert!(not_found.is_none());
}

#[tokio::test]
async fn get_by_id_exposes_codex_fields() {
    let db = setup_db().await;
    let node = create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Mira".to_string(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    db.query(
        "UPDATE type::thing('npc', $id) SET codex_article = 'An article.', codex_stale = true",
    )
    .bind(("id", node.id.clone()))
    .await
    .unwrap();
    let got = get_by_id(&db, &node.id, EntityKind::Npc).await.unwrap();
    assert_eq!(got.codex_article.as_deref(), Some("An article."));
    assert_eq!(got.codex_stale, Some(true));
}
