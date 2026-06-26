use super::*;
use crate::services::entity_service::{create, EntityInput, EntityKind};
use surrealdb::{engine::local::Db, Surreal};

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    crate::schema::run_migrations(&db).await.unwrap();
    db
}

fn make_npc(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
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
    }
}

async fn create_campaign(db: &Surreal<Db>) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query(
            "CREATE campaign SET name='Test Campaign', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    rows.into_iter().next().unwrap().id.id.to_raw()
}

#[tokio::test]
async fn multiple_wikilinks_all_returned() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let torvin = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("Torvin"),
    )
    .await
    .unwrap();
    let ironhold = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Location,
        make_npc("Ironhold"),
    )
    .await
    .unwrap();
    let source = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("SourceNPC"),
    )
    .await
    .unwrap();
    let result = parse_and_sync_wikilinks(
        &db,
        "npc",
        &source.id,
        "[[Torvin]] traveled to [[Ironhold]] yesterday.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.contains(&format!("npc:{}", torvin.id)));
    assert!(result.contains(&format!("location:{}", ironhold.id)));
}

#[tokio::test]
async fn session_source_skips_relates_to_edges_but_returns_ids() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let torvin = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("Torvin"),
    )
    .await
    .unwrap();
    let result = parse_and_sync_wikilinks(
        &db,
        "session",
        "somesessionid",
        "[[Torvin]] appeared.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(result, vec![format!("npc:{}", torvin.id)]);
    let mut resp = db
        .query("SELECT count() FROM relates_to GROUP ALL")
        .await
        .unwrap();
    let count: Option<serde_json::Value> = resp.take(0).unwrap();
    let n = count
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(n, 0, "session source must not create relates_to edges");
}

#[tokio::test]
async fn repeated_call_same_notes_produces_single_edge() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let torvin = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("Torvin"),
    )
    .await
    .unwrap();
    let source = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("SourceNPC"),
    )
    .await
    .unwrap();
    let notes = "We met [[Torvin]] at the inn.";
    for _ in 0..2 {
        parse_and_sync_wikilinks(
            &db,
            "npc",
            &source.id,
            notes,
            WikilinkScope::Campaign {
                campaign_id: &campaign_id,
            },
        )
        .await
        .unwrap();
    }
    let source_record = format!("npc:{}", source.id);
    let mut resp = db
        .query(format!(
            "SELECT count() FROM relates_to \
         WHERE in = {source_record} AND rel_type = 'mentioned' GROUP ALL"
        ))
        .await
        .unwrap();
    let count: Option<serde_json::Value> = resp.take(0).unwrap();
    let n = count
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(
        n, 1,
        "repeated calls with same notes must produce exactly one edge, got {n}"
    );
    let _ = torvin;
}

#[test]
fn validate_identifier_rejects_special_chars() {
    assert!(validate_identifier("npc").is_ok());
    assert!(validate_identifier("player_character").is_ok());
    assert!(validate_identifier("abc123").is_ok());
    assert!(validate_identifier("npc; DROP TABLE npc").is_err());
    assert!(validate_identifier("npc->relates_to").is_err());
    assert!(validate_identifier("foo:bar").is_err());
    assert!(validate_identifier("").is_err());
}

#[tokio::test]
async fn invalid_source_table_returns_error() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let result = parse_and_sync_wikilinks(
        &db,
        "npc; DROP TABLE npc",
        "someId",
        "some notes",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await;
    assert!(
        matches!(result, Err(WikilinkError::InvalidIdentifier { .. })),
        "expected InvalidIdentifier error, got {result:?}"
    );
}

#[test]
fn split_record_id_error_on_missing_colon() {
    let result = split_record_id("npcabc123");
    assert!(matches!(
        result,
        Err(WikilinkError::MalformedRecordId { .. })
    ));
    let ok = split_record_id("npc:abc123").unwrap();
    assert_eq!(ok, ("npc", "abc123"));
}

#[test]
fn entity_tables_matches_entity_kind() {
    for t in ENTITY_TABLES {
        EntityKind::from_table(t)
            .unwrap_or_else(|_| panic!("ENTITY_TABLES entry '{t}' not in EntityKind"));
    }
    let kind_count = [
        EntityKind::Npc,
        EntityKind::Location,
        EntityKind::Faction,
        EntityKind::Creature,
        EntityKind::Item,
        EntityKind::Event,
        EntityKind::PlayerCharacter,
        EntityKind::Misc,
    ]
    .len();
    assert_eq!(
        ENTITY_TABLES.len(),
        kind_count,
        "ENTITY_TABLES length doesn't match EntityKind variant count"
    );
}
