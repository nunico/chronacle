use std::sync::Arc;

use serde::Deserialize;

use crate::providers::embedding::{EmbeddingProvider, MockEmbeddingProvider};
use crate::services::entity_service::{EntityInput, EntityKind, GraphNode};

use super::{create, embed_node, embed_text, order_events_for_timeline};

#[test]
fn order_events_for_timeline_sorts_by_sequence_then_name_nulls_last() {
    fn event(name: &str, seq: Option<i64>) -> GraphNode {
        GraphNode {
            id: name.to_string(), kind: "event".to_string(),
            campaign_id: None, collection_id: None,
            name: name.to_string(), summary: None, notes: None,
            created_at: None, updated_at: None, date_start: None, date_end: None,
            is_ongoing: None, sequence_index: seq, era: None, duration_label: None,
            session_id: None, player_name: None, character_class: None,
            character_level: None, status: None,
        }
    }
    let input = vec![
        event("Unplaced B", None),
        event("Second", Some(2)),
        event("Third", Some(3)),
        event("Also Second", Some(2)),
        event("First", Some(1)),
        event("Unplaced A", None),
    ];
    let ordered: Vec<String> = order_events_for_timeline(input)
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(
        ordered,
        vec!["First", "Also Second", "Second", "Third", "Unplaced A", "Unplaced B"]
    );
}

#[test]
fn embed_text_includes_name_summary_and_notes() {
    let text = embed_text("Seraphina", Some("the archivist"), Some("Guards the Sunstone."));
    assert!(text.contains("Seraphina"), "name missing: {text}");
    assert!(text.contains("the archivist"), "summary missing: {text}");
    assert!(text.contains("Guards the Sunstone."), "notes missing: {text}");
}

#[test]
fn embed_text_skips_empty_parts() {
    assert_eq!(embed_text("Bob", None, None), "Bob");
    assert_eq!(embed_text("Bob", Some("  "), Some("")), "Bob");
}

#[tokio::test]
async fn embed_node_populates_embedding_and_model() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    crate::schema::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    ).await.unwrap();

    let node = create(
        &db, Some("camp1"), None, EntityKind::Npc,
        EntityInput {
            name: "Seraphina".to_string(),
            notes: Some("Guards the Sunstone beneath the Iron Tower.".to_string()),
            ..Default::default()
        },
    ).await.unwrap();

    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    embed_node(&db, &embed, &node).await.unwrap();

    #[derive(Deserialize)]
    struct Row { embedding: Option<Vec<f32>>, embed_model: Option<String> }
    let mut resp = db
        .query("SELECT embedding, embed_model FROM type::thing('npc', $id)")
        .bind(("id", node.id.clone()))
        .await.unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    let row = rows.into_iter().next().expect("npc row");
    assert_eq!(row.embedding.as_ref().map(|v| v.len()), Some(768));
    assert_eq!(row.embed_model.as_deref(), Some("mock"));
}
