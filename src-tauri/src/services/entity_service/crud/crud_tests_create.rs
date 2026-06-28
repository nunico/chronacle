use crate::services::entity_service::{EntityInput, EntityKind};

use super::{count_by_campaign, create};

async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[tokio::test]
async fn count_by_campaign_returns_per_kind_counts() {
    let db = setup_db().await;
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    let input = |name: &str| EntityInput {
        name: name.to_string(),
        ..Default::default()
    };
    create(&db, Some("camp1"), None, EntityKind::Npc, input("Torvin"))
        .await
        .unwrap();
    create(&db, Some("camp1"), None, EntityKind::Npc, input("Mira"))
        .await
        .unwrap();
    create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Location,
        input("Docks"),
    )
    .await
    .unwrap();
    let counts = count_by_campaign(&db, "camp1").await.unwrap();
    assert_eq!(counts.get("npc"), Some(&2));
    assert_eq!(counts.get("location"), Some(&1));
    assert_eq!(counts.get("faction"), Some(&0));
    assert_eq!(counts.len(), 8, "every kind should be present");
}

#[tokio::test]
async fn count_by_campaign_does_not_count_other_campaigns() {
    let db = setup_db().await;
    for c in ["camp1", "camp2"] {
        db.query(format!(
            "CREATE campaign SET id='{c}', name='Test', system='5e', \
             created_at=time::now(), updated_at=time::now()"
        ))
        .await
        .unwrap();
    }
    create(
        &db,
        Some("camp2"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Elsewhere".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let counts = count_by_campaign(&db, "camp1").await.unwrap();
    assert_eq!(counts.get("npc"), Some(&0));
}

#[tokio::test]
async fn create_with_campaign_id_populates_campaign_via_edge() {
    let db = setup_db().await;
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    let node = create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Torvin".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(node.campaign_id.as_deref(), Some("camp1"));
    assert!(node.collection_id.is_none());
    let mut resp = db
        .query(
            "SELECT count() FROM in_campaign WHERE in = type::thing('campaign','camp1') GROUP ALL",
        )
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let counts: Vec<C> = resp.take(0).unwrap();
    assert_eq!(counts.first().map(|c| c.count).unwrap_or(0), 1);
}

#[tokio::test]
async fn create_with_collection_id_populates_collection_via_edge() {
    let db = setup_db().await;
    db.query(
        "CREATE collection SET id='col1', name='PHB', description=NULL, \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    let node = create(
        &db,
        None,
        Some("col1"),
        EntityKind::Npc,
        EntityInput {
            name: "Goblin".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(node.campaign_id.is_none());
    assert_eq!(node.collection_id.as_deref(), Some("col1"));
}
