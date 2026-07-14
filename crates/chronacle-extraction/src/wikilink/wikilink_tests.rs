use super::*;
use crate::entity_service::{create, relate, EntityInput, EntityKind};
use surrealdb::{engine::local::Db, Surreal};

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

fn make_npc(name: &str) -> EntityInput {
    EntityInput {
        name: name.to_string(),
        aliases: None,
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
async fn empty_notes_returns_empty_vec_no_db_changes() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let result = parse_and_sync_wikilinks(
        &db,
        "npc",
        "someId",
        "",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert!(result.is_empty());
    let mut resp = db
        .query("SELECT count() FROM relates_to GROUP ALL")
        .await
        .unwrap();
    let count: Option<serde_json::Value> = resp.take(0).unwrap();
    let n = count
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(n, 0, "no relates_to edges should exist");
}

#[tokio::test]
async fn nonexistent_wikilink_returns_empty_vec() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let result = parse_and_sync_wikilinks(
        &db,
        "npc",
        "someId",
        "[[NonExistentName]]",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn case_insensitive_match_returns_entity_id() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let npc = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("torvin"),
    )
    .await
    .unwrap();
    let expected_id = format!("npc:{}", npc.id);
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
        "We met [[Torvin]] at the inn.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(result, vec![expected_id]);
}

#[tokio::test]
async fn stale_relates_to_edge_deleted_on_second_call() {
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
    parse_and_sync_wikilinks(
        &db,
        "npc",
        &source.id,
        "We met [[Torvin]] at the inn.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    let mut resp = db
        .query("SELECT count() FROM relates_to GROUP ALL")
        .await
        .unwrap();
    let count: Option<serde_json::Value> = resp.take(0).unwrap();
    let n = count
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(n, 1, "edge should exist after first call");
    let result2 = parse_and_sync_wikilinks(
        &db,
        "npc",
        &source.id,
        "The inn was empty.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert!(result2.is_empty());
    let mut resp2 = db
        .query("SELECT count() FROM relates_to GROUP ALL")
        .await
        .unwrap();
    let count2: Option<serde_json::Value> = resp2.take(0).unwrap();
    let n2 = count2
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(n2, 0, "stale edge should be deleted after second call");
    let _ = torvin;
}

#[tokio::test]
async fn mentioned_edge_skipped_when_higher_tier_edge_exists() {
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
    relate(&db, &torvin.id, "npc", &source.id, "npc", "leads", None)
        .await
        .unwrap();
    let result = parse_and_sync_wikilinks(
        &db,
        "npc",
        &source.id,
        "We met [[Torvin]] at the inn.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(result, vec![format!("npc:{}", torvin.id)]);
    #[derive(serde::Deserialize)]
    struct Row {
        rel_type: String,
    }
    let mut resp = db.query("SELECT rel_type FROM relates_to").await.unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    let types: Vec<String> = rows.into_iter().map(|r| r.rel_type).collect();
    assert_eq!(types, vec!["leads"], "no mentioned edge should be added");
}
