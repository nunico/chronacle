use chronacle_extraction::entity_service::{
    create, delete, get_by_campaign, get_by_id, get_entity_graph, get_entity_relations,
    get_events_timeline, relate, resync_all_wikilinks, update, EntityError, EntityInput,
    EntityKind,
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

fn npc_input(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        aliases: Vec::new(),
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
        aliases: Vec::new(),
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
        aliases: Vec::new(),
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
            aliases: Vec::new(),
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

    let campaign = chronacle_domain::campaign_service::create(&db, "Test Campaign", "D&D 5e")
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
        aliases: Vec::new(),
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
    let c1 = chronacle_domain::campaign_service::create(&db, "Campaign One", "D&D 5e")
        .await
        .unwrap();
    let c2 = chronacle_domain::campaign_service::create(&db, "Campaign Two", "PF2e")
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
        aliases: Vec::new(),
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
            aliases: Vec::new(),
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
            aliases: Vec::new(),
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
            aliases: Vec::new(),
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
            aliases: Vec::new(),
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
    let campaign = chronacle_domain::campaign_service::create(&db, "Test Campaign", "D&D 5e")
        .await
        .unwrap();

    // Create a session first
    let session = chronacle_domain::session_service::create(
        &db,
        &campaign.id,
        chronacle_domain::session_service::SessionInput {
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
            aliases: Vec::new(),
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
    let entities = chronacle_domain::session_service::get_entities(&db, &session.id)
        .await
        .unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "Battle");
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Create a campaign and return its raw ID string.
async fn create_campaign(db: &Surreal<Db>) -> String {
    chronacle_domain::campaign_service::create(db, "Test Campaign", "D&D 5e")
        .await
        .unwrap()
        .id
}

fn location_input(name: &str, notes: Option<&str>) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        aliases: Vec::new(),
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
    let campaign = chronacle_domain::campaign_service::create(&db, "Saga", "D&D 5e")
        .await
        .unwrap();

    let event = |name: &str, seq: Option<i64>| EntityInput {
        name: name.to_string(),
        aliases: Vec::new(),
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

#[tokio::test]
async fn get_entity_graph_returns_center_neighbors_and_edges() {
    let db = setup_db().await;

    // Center NPC + two neighbors across different tables.
    let varin = create(
        &db,
        Some("c1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Varin".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let keep = create(
        &db,
        Some("c1"),
        None,
        EntityKind::Location,
        EntityInput {
            name: "The Keep".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let pact = create(
        &db,
        Some("c1"),
        None,
        EntityKind::Faction,
        EntityInput {
            name: "The Pact".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // outbound: Varin -> Keep ; inbound: Pact -> Varin
    relate(
        &db,
        &varin.id,
        "npc",
        &keep.id,
        "location",
        "resides_in",
        None,
    )
    .await
    .unwrap();
    relate(&db, &pact.id, "faction", &varin.id, "npc", "controls", None)
        .await
        .unwrap();

    let graph = get_entity_graph(&db, &varin.id, "npc", 1).await.unwrap();

    // nodes: center + 2 neighbors, deduped — 3 total
    let mut names: Vec<&str> = graph.nodes.iter().map(|n| n.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["The Keep", "The Pact", "Varin"]);

    // edges: both directions present
    assert_eq!(graph.edges.len(), 2);
    assert!(
        graph
            .edges
            .iter()
            .any(|e| e.from_id == varin.id && e.to_id == keep.id && e.rel_type == "resides_in"),
        "outbound edge Varin->Keep missing"
    );
    assert!(
        graph
            .edges
            .iter()
            .any(|e| e.from_id == pact.id && e.to_id == varin.id && e.rel_type == "controls"),
        "inbound edge Pact->Varin missing"
    );
}

#[tokio::test]
async fn get_entity_graph_isolated_entity_returns_just_itself() {
    let db = setup_db().await;

    let lonely = create(
        &db,
        Some("c1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Hermit".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let graph = get_entity_graph(&db, &lonely.id, "npc", 1).await.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].name, "Hermit");
    assert!(graph.edges.is_empty());
}

#[tokio::test]
async fn get_entity_graph_dedupes_node_with_multiple_edges_to_same_neighbor() {
    let db = setup_db().await;

    // Center NPC
    let center = create(
        &db,
        Some("c1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Serafine".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // One neighbor location
    let tower = create(
        &db,
        Some("c1"),
        None,
        EntityKind::Location,
        EntityInput {
            name: "The Iron Tower".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Two edges from center to the same neighbor, with different rel_types.
    relate(
        &db,
        &center.id,
        "npc",
        &tower.id,
        "location",
        "resides_in",
        None,
    )
    .await
    .unwrap();
    relate(
        &db, &center.id, "npc", &tower.id, "location", "guards", None,
    )
    .await
    .unwrap();

    let graph = get_entity_graph(&db, &center.id, "npc", 1).await.unwrap();

    // The neighbor must appear as exactly ONE node (deduped), despite two edges.
    let neighbor_count = graph.nodes.iter().filter(|n| n.id == tower.id).count();
    assert_eq!(
        neighbor_count, 1,
        "neighbor location should appear exactly once in nodes even with two edges"
    );

    // Both edges must be present.
    assert_eq!(graph.edges.len(), 2, "both edges should be returned");
}

#[tokio::test]
async fn get_entity_graph_rejects_unsafe_id() {
    let db = setup_db().await;

    let err = get_entity_graph(&db, "bad id; DROP", "npc", 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, EntityError::Validation { ref field, .. } if field == "id"),
        "expected Validation error on field 'id', got: {err:?}"
    );
}

// ── Part A: forward-reference wikilink reconciliation ─────────────────────────

/// When entity A already mentions [[Brother Bram]] in notes, and then "Brother
/// Bram" is created AFTER, creating Bram must form an inbound edge A→Bram so
/// the graph shows the relationship without requiring a manual re-save of A.
#[tokio::test]
async fn forward_reference_wikilink_reconciled_on_new_entity_create() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    // Step 1: create entity A with a forward reference to "Brother Bram"
    let entity_a = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Sven".into(),
            aliases: Vec::new(),
            notes: Some("Will ally with [[Brother Bram]].".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // At this point "Brother Bram" doesn't exist, so no edge should be formed yet.
    let graph_before = get_entity_graph(&db, &entity_a.id, "npc", 1).await.unwrap();
    assert_eq!(
        graph_before.edges.len(),
        0,
        "no edge expected before Bram is created"
    );

    // Step 2: NOW create "Brother Bram" in the same campaign
    let bram = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Brother Bram".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Step 3: the graph around A must now contain Bram as a neighbor with an edge
    let graph = get_entity_graph(&db, &entity_a.id, "npc", 1).await.unwrap();

    let bram_node = graph.nodes.iter().find(|n| n.name == "Brother Bram");
    assert!(
        bram_node.is_some(),
        "Brother Bram should appear as a neighbor node in Sven's graph after being created"
    );

    let edge = graph
        .edges
        .iter()
        .find(|e| e.from_id == entity_a.id && e.to_id == bram.id && e.rel_type == "mentioned");
    assert!(
        edge.is_some(),
        "expected a 'mentioned' edge from Sven to Brother Bram, got edges: {:?}",
        graph.edges
    );
}

/// An entity whose notes do NOT mention the newly created entity must NOT get
/// a spurious edge (no false positives from the inbound reconciliation).
#[tokio::test]
async fn forward_reference_reconciliation_no_false_positive() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    // Entity A has notes that do NOT mention "Ghost"
    let entity_a = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Sven".into(),
            aliases: Vec::new(),
            notes: Some("Wanders the forest alone.".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Create a new entity — must NOT trigger an edge from A to Ghost
    let ghost = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Ghost".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let graph = get_entity_graph(&db, &entity_a.id, "npc", 1).await.unwrap();

    assert!(
        graph.edges.is_empty(),
        "no edge should be created when notes do not mention the new entity; \
         got edges: {:?}",
        graph.edges
    );
    assert!(
        !graph.nodes.iter().any(|n| n.id == ghost.id),
        "Ghost should not appear as a neighbor of Sven"
    );
}

// ── Part B: get_entity_relations ──────────────────────────────────────────────

/// Center has one outbound edge (center→X, rel_type 'mentioned') and one
/// inbound (Y→center, rel_type 'commands').  get_entity_relations must return
/// both with correct direction, rel_type, and names, and must not include a
/// self-loop.
#[tokio::test]
async fn get_entity_relations_returns_both_directions_with_correct_fields() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    let center = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Center".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let target_x = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Location,
        EntityInput {
            name: "The Vault".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let source_y = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Faction,
        EntityInput {
            name: "Iron Legion".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // center→target_x  (outbound from center)
    relate(
        &db,
        &center.id,
        "npc",
        &target_x.id,
        "location",
        "mentioned",
        None,
    )
    .await
    .unwrap();

    // source_y→center  (inbound to center)
    relate(
        &db,
        &source_y.id,
        "faction",
        &center.id,
        "npc",
        "commands",
        None,
    )
    .await
    .unwrap();

    let relations = get_entity_relations(&db, &center.id, "npc").await.unwrap();

    assert_eq!(
        relations.len(),
        2,
        "expected exactly 2 related entities, got: {relations:#?}"
    );

    // Find the outbound relation: center→The Vault
    let outbound = relations
        .iter()
        .find(|r| r.name == "The Vault")
        .expect("should have The Vault as related entity");
    assert_eq!(outbound.direction, "outbound", "center→X must be outbound");
    assert_eq!(outbound.rel_type, "mentioned");
    assert_eq!(outbound.kind, "location");

    // Find the inbound relation: Iron Legion→center
    let inbound = relations
        .iter()
        .find(|r| r.name == "Iron Legion")
        .expect("should have Iron Legion as related entity");
    assert_eq!(inbound.direction, "inbound", "Y→center must be inbound");
    assert_eq!(inbound.rel_type, "commands");
    assert_eq!(inbound.kind, "faction");

    // Self-loop sanity: center must not appear in its own relations list
    assert!(
        !relations.iter().any(|r| r.id == center.id),
        "center must not appear in its own relations list"
    );
}

// ── Fix 1: idempotent relate() ────────────────────────────────────────────────

/// Calling relate() twice with the same (from, to, rel_type) must produce
/// exactly ONE edge — not two. This is the regression guard for the idempotency fix.
#[tokio::test]
async fn relate_twice_same_triple_produces_exactly_one_edge() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    let npc = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        npc_input("Aldric"),
    )
    .await
    .unwrap();

    let loc = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Location,
        EntityInput {
            name: "Blackstone Keep".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // First call
    relate(&db, &npc.id, "npc", &loc.id, "location", "guards", None)
        .await
        .unwrap();
    // Second call — must be idempotent
    relate(&db, &npc.id, "npc", &loc.id, "location", "guards", None)
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct CountRow {
        count: i64,
    }
    let npc_record = format!("npc:{}", npc.id);
    let loc_record = format!("location:{}", loc.id);
    let mut resp = db
        .query(format!(
            "SELECT count() FROM relates_to \
             WHERE in = {npc_record} AND out = {loc_record} AND rel_type = 'guards' \
             GROUP ALL"
        ))
        .await
        .unwrap();
    let rows: Vec<CountRow> = resp.take(0).unwrap();
    let n = rows.first().map(|r| r.count).unwrap_or(0);
    assert_eq!(
        n, 1,
        "relate() called twice with the same triple must yield exactly 1 edge, got {n}"
    );
}

// ── Fix 3: resync_all_wikilinks backfill ─────────────────────────────────────

/// Prove the backfill: create entity A with notes mentioning Bram, create Bram
/// (which produces an edge via reconciliation), DELETE all relates_to edges
/// directly, assert zero edges, call resync_all_wikilinks, assert the A→Bram
/// mentioned edge is recreated, and that get_entity_graph sees Bram as a neighbor.
#[tokio::test]
async fn resync_all_wikilinks_regenerates_edges_from_existing_notes() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;

    // Create Bram first so the wikilink resolves on entity_a's creation.
    let bram = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Bram".into(),
            aliases: Vec::new(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let entity_a = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Sven".into(),
            aliases: Vec::new(),
            notes: Some("Always travels with [[Bram]].".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // At this point there should already be an edge from the create path.
    // Nuke ALL relates_to edges to simulate "edges missing" (pre-backfill state).
    db.query("DELETE relates_to").await.unwrap();

    // Confirm zero edges after the DELETE.
    let mut resp = db
        .query("SELECT count() FROM relates_to GROUP ALL")
        .await
        .unwrap();
    let after_delete: Vec<serde_json::Value> = resp.take(0).unwrap();
    let count_zero = after_delete
        .first()
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(
        count_zero, 0,
        "all edges should be gone after DELETE relates_to"
    );

    // Run the backfill.
    let processed = resync_all_wikilinks(&db).await.unwrap();
    assert!(
        processed >= 1,
        "at least entity_a (with non-empty notes) should have been processed; got {processed}"
    );

    // The A→Bram mentioned edge must be recreated.
    let a_record = format!("npc:{}", entity_a.id);
    let bram_record = format!("npc:{}", bram.id);
    let mut resp2 = db
        .query(format!(
            "SELECT count() FROM relates_to \
             WHERE in = {a_record} AND out = {bram_record} AND rel_type = 'mentioned' \
             GROUP ALL"
        ))
        .await
        .unwrap();
    let edge_rows: Vec<serde_json::Value> = resp2.take(0).unwrap();
    let edge_count = edge_rows
        .first()
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(
        edge_count, 1,
        "resync must recreate exactly one A→Bram mentioned edge, got {edge_count}"
    );

    // get_entity_graph must now see Bram as a neighbor of entity_a.
    let graph = get_entity_graph(&db, &entity_a.id, "npc", 1).await.unwrap();
    assert!(
        graph.nodes.iter().any(|n| n.id == bram.id),
        "Bram should appear as a neighbor in entity_a's ego graph after backfill; \
         nodes: {:#?}",
        graph.nodes
    );
}
