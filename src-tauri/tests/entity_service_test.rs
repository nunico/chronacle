use chronacle_lib::services::entity_service::{
    create, get_by_campaign, get_by_id, EntityInput, EntityKind,
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

fn npc_input(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        summary: Some("A shady merchant".to_string()),
        notes: None,
        date_start: None,
        date_end: None,
        is_ongoing: None,
        sequence_index: None,
        era: None,
        duration_label: None,
        player_name: None,
        character_class: None,
        character_level: None,
        status: None,
    }
}

#[tokio::test]
async fn create_npc_returns_node_with_correct_kind() {
    let db = setup_db().await;
    let node = create(&db, None, EntityKind::Npc, npc_input("Torvin"))
        .await
        .unwrap();
    assert_eq!(node.kind, "npc");
    assert_eq!(node.name, "Torvin");
    assert_eq!(node.summary.as_deref(), Some("A shady merchant"));
    assert!(node.campaign_id.is_none());
    assert!(!node.id.is_empty());
}

#[tokio::test]
async fn create_event_stores_temporal_fields() {
    let db = setup_db().await;
    let input = EntityInput {
        name: "Battle of the Ashfields".to_string(),
        summary: None,
        notes: None,
        date_start: Some("Year 312".to_string()),
        date_end: Some("Year 312".to_string()),
        is_ongoing: Some(false),
        sequence_index: Some(42),
        era: Some("Third Age".to_string()),
        duration_label: Some("3 days".to_string()),
        player_name: None,
        character_class: None,
        character_level: None,
        status: None,
    };
    let node = create(&db, None, EntityKind::Event, input).await.unwrap();
    assert_eq!(node.kind, "event");
    assert_eq!(node.date_start.as_deref(), Some("Year 312"));
    assert_eq!(node.sequence_index, Some(42));
    assert_eq!(node.era.as_deref(), Some("Third Age"));
}

#[tokio::test]
async fn create_player_character_stores_pc_fields() {
    let db = setup_db().await;
    let input = EntityInput {
        name: "Aeris".to_string(),
        summary: None,
        notes: None,
        date_start: None,
        date_end: None,
        is_ongoing: None,
        sequence_index: None,
        era: None,
        duration_label: None,
        player_name: Some("Alice".to_string()),
        character_class: Some("Wizard".to_string()),
        character_level: Some(7),
        status: Some("active".to_string()),
    };
    let node = create(&db, None, EntityKind::PlayerCharacter, input)
        .await
        .unwrap();
    assert_eq!(node.kind, "player_character");
    assert_eq!(node.player_name.as_deref(), Some("Alice"));
    assert_eq!(node.character_level, Some(7));
    assert_eq!(node.status.as_deref(), Some("active"));
}

#[tokio::test]
async fn get_by_id_returns_created_node() {
    let db = setup_db().await;
    let created = create(
        &db,
        None,
        EntityKind::Location,
        EntityInput {
            name: "Shadowmere".to_string(),
            summary: None,
            notes: None,
            date_start: None,
            date_end: None,
            is_ongoing: None,
            sequence_index: None,
            era: None,
            duration_label: None,
            player_name: None,
            character_class: None,
            character_level: None,
            status: None,
        },
    )
    .await
    .unwrap();

    let fetched = get_by_id(&db, &created.id, EntityKind::Location)
        .await
        .unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Shadowmere");
}

#[tokio::test]
async fn get_by_id_not_found_returns_error() {
    use chronacle_lib::services::entity_service::EntityError;
    let db = setup_db().await;
    let err = get_by_id(&db, "nonexistent", EntityKind::Npc)
        .await
        .unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}

#[tokio::test]
async fn get_by_campaign_returns_only_matching_entities() {
    let db = setup_db().await;

    let campaign =
        chronacle_lib::services::campaign_service::create(&db, "Test Campaign", "D&D 5e")
            .await
            .unwrap();

    let n1 = create(&db, Some(&campaign.id), EntityKind::Npc, npc_input("Nym"))
        .await
        .unwrap();
    let _n2 = create(&db, None, EntityKind::Npc, npc_input("Orphan NPC"))
        .await
        .unwrap();

    let results = get_by_campaign(&db, &campaign.id, EntityKind::Npc)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, n1.id);
}

#[tokio::test]
async fn create_with_empty_name_returns_validation_error() {
    use chronacle_lib::services::entity_service::EntityError;
    let db = setup_db().await;
    let input = EntityInput {
        name: "   ".to_string(),
        summary: None,
        notes: None,
        date_start: None,
        date_end: None,
        is_ongoing: None,
        sequence_index: None,
        era: None,
        duration_label: None,
        player_name: None,
        character_class: None,
        character_level: None,
        status: None,
    };
    let err = create(&db, None, EntityKind::Npc, input).await.unwrap_err();
    assert!(matches!(err, EntityError::Validation { ref field, .. } if field == "name"));
}

#[tokio::test]
async fn get_by_campaign_excludes_other_campaign_entities() {
    let db = setup_db().await;
    let c1 = chronacle_lib::services::campaign_service::create(&db, "Campaign One", "D&D 5e")
        .await
        .unwrap();
    let c2 = chronacle_lib::services::campaign_service::create(&db, "Campaign Two", "PF2e")
        .await
        .unwrap();
    create(
        &db,
        Some(&c1.id),
        EntityKind::Npc,
        npc_input("Belongs to C1"),
    )
    .await
    .unwrap();
    create(
        &db,
        Some(&c2.id),
        EntityKind::Npc,
        npc_input("Belongs to C2"),
    )
    .await
    .unwrap();
    let results = get_by_campaign(&db, &c1.id, EntityKind::Npc).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Belongs to C1");
}

#[tokio::test]
async fn get_by_id_with_wrong_kind_returns_not_found() {
    use chronacle_lib::services::entity_service::EntityError;
    let db = setup_db().await;
    let node = create(&db, None, EntityKind::Npc, npc_input("Torvin"))
        .await
        .unwrap();
    let err = get_by_id(&db, &node.id, EntityKind::Location)
        .await
        .unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}
