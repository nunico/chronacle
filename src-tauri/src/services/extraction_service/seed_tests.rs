use std::sync::Arc;

use crate::providers::embedding::{EmbeddingProvider, MockEmbeddingProvider};
use crate::providers::llm_provider::LlmProvider;
use crate::providers::vector_store::{SearchResult, VectorStore};
use crate::services::entity_service::{self, EntityKind};
use crate::services::extraction_service::{ExtractionPhase, MAX_ENRICH};

use super::super::test_support::{
    link_campaign_to_collection, setup_db_with_collection, BranchingLlm, MockLlm, MockVectorStore,
};
use super::extract_seed_anchored;

#[tokio::test]
async fn seed_anchored_builds_named_entity_and_relations_collection_scoped() {
    let (db, col_id) = setup_db_with_collection().await;
    db.query(
        "CREATE campaign SET id='camp1', name='C', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "LET $in  = type::thing('campaign',   $campaign_id); \
         LET $out = type::thing('collection', $collection_id); \
         RELATE $in->subscribes_to->$out SET created_at=time::now()",
    )
    .bind(("campaign_id", "camp1"))
    .bind(("collection_id", col_id.clone()))
    .await
    .unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entities":[{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{"name":"The Iron Fist","kind":"faction","rel_type":"commands","summary":"Militia.","notes":null}]}]}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });
    let result =
        extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
            .await
            .unwrap();
    assert_eq!(result.entities_created, 2);
    assert_eq!(result.relations_created, 1);
    let npcs = entity_service::get_by_collection(&db, &col_id, EntityKind::Npc)
        .await
        .unwrap();
    assert!(npcs.iter().any(|n| n.name == "Commander Varn"));
}

#[tokio::test]
async fn seed_anchored_emits_empty_phase_when_no_passages() {
    let (db, col_id) = setup_db_with_collection().await;
    db.query(
        "CREATE campaign SET id='camp1', name='C', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "LET $in  = type::thing('campaign',   $campaign_id); \
         LET $out = type::thing('collection', $collection_id); \
         RELATE $in->subscribes_to->$out SET created_at=time::now()",
    )
    .bind(("campaign_id", "camp1"))
    .bind(("collection_id", col_id.clone()))
    .await
    .unwrap();
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });
    let phases = std::sync::Mutex::new(Vec::new());
    let result = extract_seed_anchored(
        &db,
        &(Arc::new(MockLlm { response: "{}".to_string() }) as Arc<dyn LlmProvider>),
        &embed,
        &vs,
        "camp1",
        "Nonexistent Entity",
        |p| {
            phases.lock().unwrap().push(p);
        },
    )
    .await
    .unwrap();
    assert_eq!(result.entities_created, 0);
    let phases = phases.into_inner().unwrap();
    assert_eq!(
        phases.last().unwrap().phase,
        ExtractionPhase::Empty
    );
}

#[tokio::test]
async fn seed_anchored_uses_semantic_hits_without_lexical_match() {
    let (db, col_id) = setup_db_with_collection().await;
    db.query(
        "CREATE campaign SET id='camp1', name='C', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "LET $in = type::thing('campaign','camp1'); \
         LET $out = type::thing('collection', $cid); \
         RELATE $in->subscribes_to->$out SET created_at=time::now()",
    )
    .bind(("cid", col_id.clone()))
    .await
    .unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entities":[{"name":"Mystery Lord","kind":"npc","summary":"A figure.","notes":null,"relations":[]}]}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![SearchResult {
            chunk_id: "chunk:semchunk".to_string(),
            source_id: "source:s1".to_string(),
            source_name: "Book".to_string(),
            text: "An enigmatic ruler governs from the shadows.".to_string(),
            page_start: 1,
            page_end: 1,
            section_heading: "Lore".to_string(),
            source_type: "lore".to_string(),
            distance: 0.1,
        }],
    });
    let result =
        extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Mystery Lord", |_| {})
            .await
            .unwrap();
    assert_eq!(result.entities_created, 1, "semantic-only hit should still extract");
}

#[tokio::test]
async fn seed_anchored_enriches_neighbor_when_setting_enabled() {
    let (db, col_id) = setup_db_with_collection().await;
    link_campaign_to_collection(&db, &col_id).await;
    crate::services::settings_service::upsert(&db, "extraction_enrich_neighbors", "true")
        .await
        .unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(BranchingLlm {
        seed: r#"{"entities":[{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{"name":"The Iron Fist","kind":"faction","rel_type":"commands","summary":"The militia Varn commands.","notes":null}]}]}"#.to_string(),
        profile: r#"{"summary":"A militant faction controlling the eastern docks.","notes":"Led by [[Commander Varn]]."}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });
    extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
        .await
        .unwrap();
    let factions = entity_service::get_by_collection(&db, &col_id, EntityKind::Faction)
        .await
        .unwrap();
    let fist = factions.iter().find(|n| n.name == "The Iron Fist").expect("neighbor should exist");
    assert_eq!(
        fist.summary.as_deref(),
        Some("A militant faction controlling the eastern docks."),
        "enrichment should replace the relation-flavored summary with an entity-centric one"
    );
    assert_eq!(fist.notes.as_deref(), Some("Led by [[Commander Varn]]."));
}

#[tokio::test]
async fn seed_anchored_skips_enrichment_when_setting_disabled() {
    let (db, col_id) = setup_db_with_collection().await;
    link_campaign_to_collection(&db, &col_id).await;
    let llm: Arc<dyn LlmProvider> = Arc::new(BranchingLlm {
        seed: r#"{"entities":[{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{"name":"The Iron Fist","kind":"faction","rel_type":"commands","summary":"The militia Varn commands.","notes":null}]}]}"#.to_string(),
        profile: r#"{"summary":"SHOULD NOT BE USED","notes":null}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });
    extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
        .await
        .unwrap();
    let factions = entity_service::get_by_collection(&db, &col_id, EntityKind::Faction)
        .await
        .unwrap();
    let fist = factions.iter().find(|n| n.name == "The Iron Fist").unwrap();
    assert_eq!(
        fist.summary.as_deref(),
        Some("The militia Varn commands."),
        "without the setting, the first-pass summary must be left untouched"
    );
}

#[tokio::test]
async fn seed_anchored_caps_enrichment_at_max() {
    let (db, col_id) = setup_db_with_collection().await;
    link_campaign_to_collection(&db, &col_id).await;
    crate::services::settings_service::upsert(&db, "extraction_enrich_neighbors", "true")
        .await
        .unwrap();

    let mut rels = String::new();
    for i in 0..(MAX_ENRICH + 1) {
        if i > 0 {
            rels.push(',');
        }
        rels.push_str(&format!(
            r#"{{"name":"Neighbor{i}","kind":"npc","rel_type":"knows","summary":"rel{i}","notes":null}}"#
        ));
    }
    let seed = format!(
        r#"{{"entities":[{{"name":"Commander Varn","kind":"npc","summary":"Leader.","notes":null,"relations":[{rels}]}}]}}"#
    );
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![SearchResult {
            chunk_id: "chunk:sem".to_string(),
            source_id: "source:s1".to_string(),
            source_name: "Book".to_string(),
            text: "Some descriptive passage about a figure.".to_string(),
            page_start: 1,
            page_end: 1,
            section_heading: "Lore".to_string(),
            source_type: "lore".to_string(),
            distance: 0.1,
        }],
    });
    let llm: Arc<dyn LlmProvider> = Arc::new(BranchingLlm {
        seed,
        profile: r#"{"summary":"PROFILED","notes":null}"#.to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    extract_seed_anchored(&db, &llm, &embed, &vs, "camp1", "Commander Varn", |_| {})
        .await
        .unwrap();
    let npcs = entity_service::get_by_collection(&db, &col_id, EntityKind::Npc)
        .await
        .unwrap();
    let enriched = npcs
        .iter()
        .filter(|n| n.summary.as_deref() == Some("PROFILED"))
        .count();
    assert_eq!(enriched, MAX_ENRICH, "enrichment must be capped at MAX_ENRICH neighbors");
}
