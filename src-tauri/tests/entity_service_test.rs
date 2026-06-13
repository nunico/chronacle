use chronacle_lib::services::entity_service::{
    create, delete, get_by_campaign, get_by_id, get_events_timeline, relate, update, EntityError,
    EntityInput, EntityKind,
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
        session_id: None,
        player_name: None,
        character_class: None,
        character_level: None,
        status: None,
    }
}

#[tokio::test]
async fn create_npc_returns_node_with_correct_kind() {
    let db = setup_db().await;
    let node = create(&db, None, None, EntityKind::Npc, npc_input("Torvin"))
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
        session_id: None,
        player_name: None,
        character_class: None,
        character_level: None,
        status: None,
    };
    let node = create(&db, None, None, EntityKind::Event, input)
        .await
        .unwrap();
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
        session_id: None,
        player_name: Some("Alice".to_string()),
        character_class: Some("Wizard".to_string()),
        character_level: Some(7),
        status: Some("active".to_string()),
    };
    let node = create(&db, None, None, EntityKind::PlayerCharacter, input)
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
            session_id: None,
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

    let n1 = create(
        &db,
        Some(&campaign.id),
        None,
        EntityKind::Npc,
        npc_input("Nym"),
    )
    .await
    .unwrap();
    let _n2 = create(&db, None, None, EntityKind::Npc, npc_input("Orphan NPC"))
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
        session_id: None,
        player_name: None,
        character_class: None,
        character_level: None,
        status: None,
    };
    let err = create(&db, None, None, EntityKind::Npc, input)
        .await
        .unwrap_err();
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
        None,
        EntityKind::Npc,
        npc_input("Belongs to C1"),
    )
    .await
    .unwrap();
    create(
        &db,
        Some(&c2.id),
        None,
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
    let db = setup_db().await;
    let node = create(&db, None, None, EntityKind::Npc, npc_input("Torvin"))
        .await
        .unwrap();
    let err = get_by_id(&db, &node.id, EntityKind::Location)
        .await
        .unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}

#[tokio::test]
async fn update_changes_name_and_notes() {
    let db = setup_db().await;
    let created = create(&db, None, None, EntityKind::Npc, npc_input("Old Name"))
        .await
        .unwrap();

    let updated_input = EntityInput {
        name: "New Name".to_string(),
        summary: Some("Updated summary".to_string()),
        notes: Some("Some notes".to_string()),
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
    };
    let updated = update(&db, &created.id, EntityKind::Npc, updated_input)
        .await
        .unwrap();
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.notes.as_deref(), Some("Some notes"));
}

#[tokio::test]
async fn update_not_found_returns_error() {
    let db = setup_db().await;
    let err = update(
        &db,
        "missing",
        EntityKind::Location,
        EntityInput {
            name: "Ghost".to_string(),
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
    .unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}

#[tokio::test]
async fn delete_removes_node() {
    let db = setup_db().await;
    let created = create(
        &db,
        None,
        None,
        EntityKind::Faction,
        EntityInput {
            name: "The Crimson Hand".to_string(),
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
    .unwrap();

    delete(&db, &created.id, EntityKind::Faction).await.unwrap();

    let err = get_by_id(&db, &created.id, EntityKind::Faction)
        .await
        .unwrap_err();
    assert!(matches!(err, EntityError::NotFound { .. }));
}

#[tokio::test]
async fn update_with_empty_name_returns_validation_error() {
    let db = setup_db().await;
    let created = create(&db, None, None, EntityKind::Npc, npc_input("Valid"))
        .await
        .unwrap();
    let err = update(
        &db,
        &created.id,
        EntityKind::Npc,
        EntityInput {
            name: "  ".to_string(),
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
    .unwrap_err();
    assert!(matches!(err, EntityError::Validation { ref field, .. } if field == "name"));
}

#[tokio::test]
async fn relate_creates_edge_traversable_in_both_directions() {
    let db = setup_db().await;

    let npc = create(&db, None, None, EntityKind::Npc, npc_input("Varek"))
        .await
        .unwrap();
    let loc = create(
        &db,
        None,
        None,
        EntityKind::Location,
        EntityInput {
            name: "The Rusty Flagon".to_string(),
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
    .unwrap();

    relate(&db, &npc.id, "npc", &loc.id, "location", "frequents", None)
        .await
        .unwrap();

    // Verify edge exists by selecting all records in the relates_to table.
    // A simple SELECT * is more reliable than aggregate GROUP ALL across SurrealDB versions.
    #[derive(serde::Deserialize)]
    struct EdgeRow {
        rel_type: String,
    }
    let mut resp = db.query("SELECT rel_type FROM relates_to").await.unwrap();
    let rows: Vec<EdgeRow> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].rel_type, "frequents");
}

#[tokio::test]
async fn create_event_with_session_id_stores_session_link() {
    let db = setup_db().await;

    // Create a campaign
    let campaign =
        chronacle_lib::services::campaign_service::create(&db, "Test Campaign", "D&D 5e")
            .await
            .unwrap();

    // Create a session first
    let session = chronacle_lib::services::session_service::create(
        &db,
        &campaign.id,
        chronacle_lib::services::session_service::SessionInput {
            session_number: 1,
            title: "Session One".to_string(),
            date_played: "2026-06-05".to_string(),
            notes: String::new(),
        },
    )
    .await
    .unwrap();

    // Create an event with session_id set
    let event = create(
        &db,
        Some(&campaign.id),
        None,
        EntityKind::Event,
        EntityInput {
            name: "Battle".to_string(),
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
            session_id: Some(session.id.clone()),
        },
    )
    .await
    .unwrap();

    // Verify session_id is returned
    assert_eq!(
        event.session_id,
        Some(session.id.clone()),
        "event.session_id should be the session's id"
    );

    // Verify get_session_entities returns this event
    let entities = chronacle_lib::services::session_service::get_entities(&db, &session.id)
        .await
        .unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "Battle");
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Create a campaign and return its raw ID string.
async fn create_campaign(db: &Surreal<Db>) -> String {
    chronacle_lib::services::campaign_service::create(db, "Test Campaign", "D&D 5e")
        .await
        .unwrap()
        .id
}

fn location_input(name: &str, notes: Option<&str>) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        summary: Some(String::new()),
        notes: notes.map(|s| s.to_string()),
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

// ── Wikilink integration tests ────────────────────────────────────────────────

#[tokio::test]
async fn create_entity_with_wikilink_creates_relates_to_edge() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    // Create target NPC
    let target = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        npc_input("Torvin"),
    )
    .await
    .unwrap();

    // Create source location with notes mentioning Torvin
    let source = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Location,
        location_input("The Tavern", Some("[[Torvin]] frequents this place")),
    )
    .await
    .unwrap();

    // Verify a relates_to edge was created from source → target
    #[derive(serde::Deserialize)]
    struct EdgeRow {
        rel_type: String,
    }
    let source_record = format!("location:{}", source.id);
    let target_record = format!("npc:{}", target.id);
    let mut resp = db
        .query(format!(
            "SELECT rel_type FROM relates_to \
             WHERE in = {source_record} AND out = {target_record}"
        ))
        .await
        .unwrap();
    let rows: Vec<EdgeRow> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1, "expected one relates_to edge after create");
    assert_eq!(rows[0].rel_type, "mentioned");
}

#[tokio::test]
async fn update_entity_notes_updates_wikilink_edges() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    // Create target NPC
    let target = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        npc_input("Torvin"),
    )
    .await
    .unwrap();

    // Create source location with notes mentioning Torvin
    let source = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Location,
        location_input("The Tavern", Some("[[Torvin]] frequents this place")),
    )
    .await
    .unwrap();

    // Confirm edge exists
    let source_record = format!("location:{}", source.id);
    let target_record = format!("npc:{}", target.id);
    let mut resp = db
        .query(format!(
            "SELECT rel_type FROM relates_to \
             WHERE in = {source_record} AND out = {target_record}"
        ))
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1, "edge should exist before update");

    // Update the location's notes — remove Torvin mention
    update(
        &db,
        &source.id,
        EntityKind::Location,
        location_input("The Tavern", Some("A quiet empty place.")),
    )
    .await
    .unwrap();

    // Edge should no longer exist
    let mut resp2 = db
        .query(format!(
            "SELECT rel_type FROM relates_to \
             WHERE in = {source_record} AND out = {target_record}"
        ))
        .await
        .unwrap();
    let rows2: Vec<serde_json::Value> = resp2.take(0).unwrap();
    assert_eq!(
        rows2.len(),
        0,
        "edge should be removed after notes no longer mention Torvin"
    );
}

#[tokio::test]
async fn update_entity_to_empty_notes_removes_wikilink_edges() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    // Create target NPC
    let target = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        npc_input("Torvin"),
    )
    .await
    .unwrap();

    // Create source location with notes mentioning Torvin
    let source = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Location,
        location_input("The Tavern", Some("[[Torvin]] lives here")),
    )
    .await
    .unwrap();

    // Verify the relates_to edge exists
    let source_record = format!("location:{}", source.id);
    let target_record = format!("npc:{}", target.id);
    let mut resp = db
        .query(format!(
            "SELECT rel_type FROM relates_to \
             WHERE in = {source_record} AND out = {target_record}"
        ))
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1, "expected one relates_to edge after create");

    // Update source location with empty notes — should clear the stale edge
    update(
        &db,
        &source.id,
        EntityKind::Location,
        location_input("The Tavern", Some("")),
    )
    .await
    .unwrap();

    // Edge must be gone
    let mut resp2 = db
        .query(format!(
            "SELECT rel_type FROM relates_to \
             WHERE in = {source_record} AND out = {target_record}"
        ))
        .await
        .unwrap();
    let rows2: Vec<serde_json::Value> = resp2.take(0).unwrap();
    assert_eq!(
        rows2.len(),
        0,
        "stale relates_to edge must be deleted when notes are cleared to empty string"
    );
}

#[tokio::test]
async fn get_events_timeline_orders_by_sequence_index_nulls_last() {
    let db = setup_db().await;
    let campaign = chronacle_lib::services::campaign_service::create(&db, "Saga", "D&D 5e")
        .await
        .unwrap();

    let event = |name: &str, seq: Option<i64>| EntityInput {
        name: name.to_string(),
        sequence_index: seq,
        ..Default::default()
    };

    // Insert out of order, with an unsequenced event in the middle.
    for (name, seq) in [
        ("The Cataclysm", Some(30)),
        ("The Founding", Some(10)),
        ("A Forgotten Skirmish", None),
        ("The Schism", Some(20)),
    ] {
        create(
            &db,
            Some(&campaign.id),
            None,
            EntityKind::Event,
            event(name, seq),
        )
        .await
        .unwrap();
    }

    let timeline: Vec<String> = get_events_timeline(&db, &campaign.id)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();

    assert_eq!(
        timeline,
        vec![
            "The Founding",         // seq 10
            "The Schism",           // seq 20
            "The Cataclysm",        // seq 30
            "A Forgotten Skirmish", // unsequenced — last
        ]
    );
}
