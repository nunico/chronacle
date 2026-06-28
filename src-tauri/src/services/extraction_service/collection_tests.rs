use std::sync::Arc;

use crate::services::entity_service::{self, EntityInput, EntityKind};
use crate::services::extraction_service::{ExtractionPhase, ExtractionProgress};
use chronacle_providers::embedding::{EmbeddingProvider, MockEmbeddingProvider};
use chronacle_providers::llm_provider::LlmProvider;

use super::super::test_support::{setup_db_with_collection, MockLlm};
use super::extract_from_collection;

#[tokio::test]
async fn extract_creates_entities_with_collection_edge() {
    let (db, col_id) = setup_db_with_collection().await;
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{
            "entities": [{
                "name": "The Iron Fist",
                "kind": "faction",
                "summary": "Militant faction.",
                "notes": null,
                "relations": [{
                    "name": "Commander Varn",
                    "kind": "npc",
                    "rel_type": "commands",
                    "summary": "Leader.",
                    "notes": null
                }]
            }]
        }"#
        .to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let result = extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(result.entities_created, 2);
    assert_eq!(result.relations_created, 1);
    let mut resp = db
        .query("SELECT count() FROM in_collection WHERE in = type::thing('collection', $cid) GROUP ALL")
        .bind(("cid", col_id.clone()))
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let counts: Vec<C> = resp.take(0).unwrap();
    assert_eq!(counts.first().map(|c| c.count).unwrap_or(0), 2);
}

#[tokio::test]
async fn extract_normalizes_inverse_rel_type_and_preserves_unknown() {
    let (db, col_id) = setup_db_with_collection().await;
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{
        "entities": [{
            "name": "Commander Varn",
            "kind": "npc",
            "summary": "Leader.",
            "notes": null,
            "relations": [
                {"name": "The Iron Fist", "kind": "faction", "rel_type": "led_by", "summary": "Militia.", "notes": null},
                {"name": "The Dark Pact", "kind": "faction", "rel_type": "betrays", "summary": "A pact.", "notes": null}
            ]
        }]
    }"#
        .to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct Edge {
        #[serde(rename = "in")]
        in_thing: surrealdb::sql::Thing,
        #[serde(rename = "out")]
        out_thing: surrealdb::sql::Thing,
        rel_type: String,
    }
    let mut resp = db
        .query("SELECT in, out, rel_type FROM relates_to")
        .await
        .unwrap();
    let edges: Vec<Edge> = resp.take(0).unwrap();
    assert_eq!(
        edges.len(),
        2,
        "exactly the two relations should be persisted"
    );
    let leads = edges
        .iter()
        .find(|e| e.rel_type == "leads")
        .expect("inverse 'led_by' must normalize to canonical 'leads'");
    assert_eq!(
        leads.in_thing.tb, "faction",
        "edge must be flipped: faction is 'in'"
    );
    assert_eq!(
        leads.out_thing.tb, "npc",
        "edge must be flipped: npc is 'out'"
    );
    let betrays = edges
        .iter()
        .find(|e| e.rel_type == "betrays")
        .expect("unknown 'betrays' must be stored verbatim");
    assert_eq!(
        betrays.in_thing.tb, "npc",
        "unknown edge keeps original direction"
    );
    assert_eq!(betrays.out_thing.tb, "faction");
}

#[tokio::test]
async fn extract_deduplicates_on_second_run() {
    let (db, col_id) = setup_db_with_collection().await;
    let fixed_json = r#"{"entities":[{"name":"The Iron Fist","kind":"faction","summary":"Militant faction.","notes":null,"relations":[]}]}"#.to_string();
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let r1 = extract_from_collection(
        &db,
        &(Arc::new(MockLlm {
            response: fixed_json.clone(),
        }) as Arc<dyn LlmProvider>),
        &embed,
        &col_id,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(r1.entities_created, 1);
    let r2 = extract_from_collection(
        &db,
        &(Arc::new(MockLlm {
            response: fixed_json,
        }) as Arc<dyn LlmProvider>),
        &embed,
        &col_id,
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(
        r2.entities_created, 0,
        "duplicate entity must not be re-created"
    );
}

#[tokio::test]
async fn extract_level2_refs_stay_as_wikilinks_not_entities() {
    let (db, col_id) = setup_db_with_collection().await;
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entities":[{"name":"The Iron Fist","kind":"faction","summary":"Militant faction.","notes":"Allied with [[The Emperor's Court]].","relations":[]}]}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let result = extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(result.entities_created, 1);
    let factions = entity_service::get_by_collection(&db, &col_id, EntityKind::Faction)
        .await
        .unwrap();
    assert!(factions[0]
        .notes
        .as_deref()
        .unwrap_or("")
        .contains("[[The Emperor's Court]]"));
}

#[tokio::test]
async fn extract_from_collection_emits_done_phase_with_cumulative_counts() {
    let (db, col_id) = setup_db_with_collection().await;
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entities":[{"name":"The Iron Fist","kind":"faction","summary":"x","notes":null,"relations":[{"name":"Commander Varn","kind":"npc","rel_type":"commands","summary":"y","notes":null}]}]}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let phases = std::sync::Mutex::new(Vec::<ExtractionProgress>::new());
    extract_from_collection(&db, &llm, &embed, &col_id, |p| {
        phases.lock().unwrap().push(p);
    })
    .await
    .unwrap();
    let phases = phases.into_inner().unwrap();
    let done = phases.last().expect("at least one progress event");
    assert_eq!(done.phase, ExtractionPhase::Done);
    assert_eq!(done.entities_found, 2);
    assert_eq!(done.relations_found, 1);
}

#[tokio::test]
async fn extract_cross_link_collection_to_campaign_is_skipped() {
    let (db, col_id) = setup_db_with_collection().await;
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    entity_service::create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Campaign NPC".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entities":[{"name":"Collection Faction","kind":"faction","summary":"A faction.","notes":null,"relations":[]}]}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let result = extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();
    assert_eq!(result.entities_created, 1);
    assert_eq!(result.relations_created, 0);
}
