use super::*;
use crate::entity_service::{create, EntityInput, EntityKind};
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

async fn create_collection(db: &Surreal<Db>) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query(
            "CREATE collection SET name='Test Collection', description=NULL, \
         created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    rows.into_iter().next().unwrap().id.id.to_raw()
}

#[tokio::test]
async fn duplicate_wikilink_in_notes_produces_single_edge() {
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
        "[[Torvin]] met [[Torvin]] again",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    let source_record = format!("npc:{}", source.id);
    let torvin_record = format!("npc:{}", torvin.id);
    let mut resp = db
        .query(format!(
            "SELECT count() FROM relates_to \
         WHERE in = {source_record} AND out = {torvin_record} \
         AND rel_type = 'mentioned' GROUP ALL"
        ))
        .await
        .unwrap();
    let count: Option<serde_json::Value> = resp.take(0).unwrap();
    let n = count
        .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
        .unwrap_or(0);
    assert_eq!(
        n, 1,
        "duplicate wikilinks must produce exactly one edge, got {n}"
    );
}

#[tokio::test]
async fn collection_scope_resolves_same_collection_entities() {
    let db = setup_db().await;
    let col_id = create_collection(&db).await;
    let npc = create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        make_npc("Goblin"),
    )
    .await
    .unwrap();
    let expected_id = format!("npc:{}", npc.id);
    let campaign_id = create_campaign(&db).await;
    create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("Goblin"),
    )
    .await
    .unwrap();
    let source = create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        make_npc("SourceNPC"),
    )
    .await
    .unwrap();
    let result = parse_and_sync_wikilinks(
        &db,
        "npc",
        &source.id,
        "We fought [[Goblin]].",
        WikilinkScope::Collection {
            collection_id: &col_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0], expected_id,
        "should match collection entity, not campaign entity"
    );
}

#[tokio::test]
async fn campaign_scope_resolves_subscribed_collection_entities() {
    let db = setup_db().await;
    let campaign_id = create_campaign(&db).await;
    let col_id = create_collection(&db).await;
    db.query(
        "LET $in  = type::thing('campaign',   $cid); \
         LET $out = type::thing('collection', $colid); \
         RELATE $in->subscribes_to->$out SET created_at = time::now()",
    )
    .bind(("cid", campaign_id.clone()))
    .bind(("colid", col_id.clone()))
    .await
    .unwrap();
    let col_npc = create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        make_npc("Dungeon Master"),
    )
    .await
    .unwrap();
    let expected_id = format!("npc:{}", col_npc.id);
    let source = create(
        &db,
        Some(&campaign_id),
        None,
        EntityKind::Npc,
        make_npc("Player"),
    )
    .await
    .unwrap();
    let result = parse_and_sync_wikilinks(
        &db,
        "npc",
        &source.id,
        "Asked the [[Dungeon Master]] for help.",
        WikilinkScope::Campaign {
            campaign_id: &campaign_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], expected_id);
}
