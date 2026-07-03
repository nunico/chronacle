use crate::entity_service::{EntityInput, EntityKind};

use super::{create, get_by_id, update};

async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db
}

#[tokio::test]
async fn update_clears_nullable_fields_to_null_not_none() {
    let db = setup_db().await;
    let node = create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Torvin".to_string(),
            summary: Some("Old summary.".to_string()),
            notes: Some("Old notes.".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Binding `None` must persist SurrealDB NULL, not NONE, which the
    // SCHEMAFULL `string | NULL` fields would reject.
    let updated = update(
        &db,
        &node.id,
        EntityKind::Npc,
        EntityInput {
            name: "Torvin".to_string(),
            summary: None,
            notes: None,
            ..Default::default()
        },
    )
    .await
    .expect("update should not error when clearing nullable fields");
    assert_eq!(updated.summary, None);
    assert_eq!(updated.notes, None);

    let refetched = get_by_id(&db, &node.id, EntityKind::Npc).await.unwrap();
    assert_eq!(
        refetched.summary, None,
        "summary should be cleared in the DB"
    );
    assert_eq!(refetched.notes, None, "notes should be cleared in the DB");
}

#[tokio::test]
async fn update_clears_nullable_event_fields() {
    let db = setup_db().await;
    let node = create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Event,
        EntityInput {
            name: "The Siege".to_string(),
            date_start: Some("1402".to_string()),
            era: Some("Third Age".to_string()),
            sequence_index: Some(3),
            is_ongoing: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let updated = update(
        &db,
        &node.id,
        EntityKind::Event,
        EntityInput {
            name: "The Siege".to_string(),
            date_start: None,
            era: None,
            sequence_index: None,
            is_ongoing: Some(false),
            ..Default::default()
        },
    )
    .await
    .expect("clearing nullable event fields should not error");
    assert_eq!(updated.date_start, None);
    assert_eq!(updated.era, None);
    assert_eq!(updated.sequence_index, None);
}

#[tokio::test]
async fn update_marks_codex_stale() {
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
    )
    .await
    .unwrap();

    update(
        &db,
        &node.id,
        EntityKind::Npc,
        EntityInput {
            name: "Mira".to_string(),
            notes: Some("She now runs the Gilded Flagon.".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    #[derive(serde::Deserialize)]
    struct Row {
        codex_stale: bool,
    }
    let mut resp = db
        .query("SELECT codex_stale FROM type::thing('npc', $id)")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0].codex_stale, "user edits must mark the article stale");
}
