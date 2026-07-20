//! B3 acceptance (ADR-011, backend-only): compiled rules reach the prompt in
//! RULES → CODEX/ENTITIES → CHUNKS order; uncompiled campaigns are unchanged.
//! Scenario names mirror the spec's Gherkin.

use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use tokio::sync::mpsc;

use chronacle_core::llm::{ChatMessage, LlmError, LlmProvider};
use chronacle_providers::embedding::{EmbeddingProvider, MockEmbeddingProvider};
use chronacle_providers::vector_store::{IndexedChunk, SurrealDbVector, VectorStore};
use chronacle_retrieval::agent_service::stream_response;

/// A recording mock LLM: captures the system prompt it was called with and
/// returns an empty token stream, so tests can assert on prompt contents
/// without depending on a real provider.
struct RecordingLlmProvider {
    recorded_prompt: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl LlmProvider for RecordingLlmProvider {
    fn provider_type(&self) -> &'static str {
        "recording-mock"
    }

    async fn chat_stream(
        &self,
        system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        *self.recorded_prompt.lock().unwrap() = Some(system_prompt.to_string());
        let (tx, rx) = mpsc::channel(1);
        drop(tx); // Close immediately — an empty token stream is enough for these tests.
        Ok(rx)
    }
}

/// Sets up an in-memory SurrealDB with a campaign subscribed to a collection
/// that has one compiled `rule_entry` and one indexed `chunk`, both carrying
/// 768-dim embeddings so the mock embedding provider's query vector hits both
/// via KNN.
async fn seed_campaign_with_rules_and_chunks() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    db.query(
        "CREATE collection:`ca` SET name='A', description=NONE, \
             created_at=time::now(), updated_at=time::now();
         CREATE campaign:`camp1` SET name='Test Campaign', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now();
         RELATE campaign:`camp1`->subscribes_to->collection:`ca` SET created_at=time::now();
         CREATE source SET id='s1', filename='phb.pdf', display_name='PHB', \
             source_type='rules', page_count=1, indexed_at=time::now(), \
             index_status='done', embed_model='mock', collection=collection:`ca`;
         CREATE rule_entry:`r1` SET collection=collection:`ca`, name='Initiative', \
             category='mechanic', body='Roll d20 and add DEX to determine turn order.', \
             compiled_at=time::now(), stale=false, \
             page_refs=[{ source_name: 'PHB', page_start: 14, page_end: 15 }], \
             embedding=$vec, embed_model='mock';",
    )
    .bind(("vec", vec![0.0_f32; 768]))
    .await
    .unwrap()
    .check()
    .unwrap();

    let store = SurrealDbVector::new(db.clone());
    let indexed = vec![IndexedChunk {
        chunk_id: "s1-0".to_string(),
        collection_id: "ca".to_string(),
        text: "A fighter can use Action Surge once per rest.".to_string(),
        page_start: 72,
        page_end: 72,
        section_heading: "Fighter Class Features".to_string(),
        source_type: "rules".into(),
        embedding: vec![0.0_f32; 768],
        embed_model: "mock".into(),
    }];
    store.upsert("s1", &indexed).await.unwrap();

    db
}

#[tokio::test]
async fn rules_question_gets_rules_block_before_chunks() {
    let db = seed_campaign_with_rules_and_chunks().await;

    let embedding_provider: RwLock<Arc<dyn EmbeddingProvider>> =
        RwLock::new(Arc::new(MockEmbeddingProvider::new(768)));
    let vector_store: Arc<dyn VectorStore> = Arc::new(SurrealDbVector::new(db.clone()));

    let recorded_prompt = Arc::new(Mutex::new(None));
    let llm_provider: RwLock<Arc<dyn LlmProvider>> = RwLock::new(Arc::new(RecordingLlmProvider {
        recorded_prompt: recorded_prompt.clone(),
    }));

    let mut rx = stream_response(
        &db,
        &embedding_provider,
        &vector_store,
        &llm_provider,
        "How does initiative work?",
        Some("camp1"),
        "en",
    )
    .await
    .expect("stream_response should succeed");

    // Drain the (empty) token channel so the provider's `chat_stream` call
    // has definitely completed before we inspect the recorded prompt.
    while rx.recv().await.is_some() {}

    let prompt = recorded_prompt
        .lock()
        .unwrap()
        .clone()
        .expect("recording mock LLM should have captured the system prompt");

    let i_rules = prompt
        .find("COMPILED RULES")
        .expect("prompt should contain the COMPILED RULES section");
    let i_ref = prompt
        .find("REFERENCE MATERIAL")
        .expect("prompt should contain the REFERENCE MATERIAL section");
    assert!(
        i_rules < i_ref,
        "COMPILED RULES must appear before REFERENCE MATERIAL: {prompt}"
    );
    assert!(
        prompt.contains("PHB p.14-15"),
        "prompt should include the rule entry's book+page: {prompt}"
    );
}

#[tokio::test]
async fn compiled_article_excerpt_appears_in_codex_block_between_rules_and_chunks() {
    let db = seed_campaign_with_rules_and_chunks().await;

    // A compiled entity article, subscribed via the collection so it's picked
    // up by the collection-entity KNN branch of `fetch_entity_context`.
    db.query(
        "CREATE npc:`n1` SET name='Aldric the Smith', summary='village blacksmith', \
             notes=NULL, codex_article='Compiled lore about Aldric the Smith.', \
             embedding=$vec, embed_model='mock', \
             created_at=time::now(), updated_at=time::now(); \
         RELATE collection:`ca`->in_collection->npc:`n1` SET created_at=time::now();",
    )
    .bind(("vec", vec![0.0_f32; 768]))
    .await
    .unwrap()
    .check()
    .unwrap();

    let embedding_provider: RwLock<Arc<dyn EmbeddingProvider>> =
        RwLock::new(Arc::new(MockEmbeddingProvider::new(768)));
    let vector_store: Arc<dyn VectorStore> = Arc::new(SurrealDbVector::new(db.clone()));

    let recorded_prompt = Arc::new(Mutex::new(None));
    let llm_provider: RwLock<Arc<dyn LlmProvider>> = RwLock::new(Arc::new(RecordingLlmProvider {
        recorded_prompt: recorded_prompt.clone(),
    }));

    let mut rx = stream_response(
        &db,
        &embedding_provider,
        &vector_store,
        &llm_provider,
        "Tell me about Aldric the Smith.",
        Some("camp1"),
        "en",
    )
    .await
    .expect("stream_response should succeed");

    while rx.recv().await.is_some() {}

    let prompt = recorded_prompt
        .lock()
        .unwrap()
        .clone()
        .expect("recording mock LLM should have captured the system prompt");

    let i_rules = prompt
        .find("COMPILED RULES")
        .expect("prompt should contain the COMPILED RULES section");
    let i_codex = prompt
        .find("Codex: ")
        .expect("prompt should contain a Codex excerpt line");
    let i_ref = prompt
        .find("REFERENCE MATERIAL")
        .expect("prompt should contain the REFERENCE MATERIAL section");
    assert!(
        i_rules < i_codex,
        "COMPILED RULES must appear before the Codex excerpt: {prompt}"
    );
    assert!(
        i_codex < i_ref,
        "the Codex excerpt must appear before REFERENCE MATERIAL: {prompt}"
    );
}

#[tokio::test]
async fn campaign_with_no_compiled_content_behaves_exactly_as_today() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    db.query(
        "CREATE collection:`ca` SET name='A', description=NONE, \
             created_at=time::now(), updated_at=time::now();
         CREATE campaign:`camp1` SET name='Test Campaign', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now();
         RELATE campaign:`camp1`->subscribes_to->collection:`ca` SET created_at=time::now();
         CREATE source SET id='s1', filename='phb.pdf', display_name='PHB', \
             source_type='rules', page_count=1, indexed_at=time::now(), \
             index_status='done', embed_model='mock', collection=collection:`ca`;",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    // No `rule_entry` rows and no `article` rows exist for this campaign — only
    // an indexed chunk, exercising the pre-B3 path.
    let store = SurrealDbVector::new(db.clone());
    let indexed = vec![IndexedChunk {
        chunk_id: "s1-0".to_string(),
        collection_id: "ca".to_string(),
        text: "A fighter can use Action Surge once per rest.".to_string(),
        page_start: 72,
        page_end: 72,
        section_heading: "Fighter Class Features".to_string(),
        source_type: "rules".into(),
        embedding: vec![0.0_f32; 768],
        embed_model: "mock".into(),
    }];
    store.upsert("s1", &indexed).await.unwrap();

    let embedding_provider: RwLock<Arc<dyn EmbeddingProvider>> =
        RwLock::new(Arc::new(MockEmbeddingProvider::new(768)));
    let vector_store: Arc<dyn VectorStore> = Arc::new(SurrealDbVector::new(db.clone()));

    let recorded_prompt = Arc::new(Mutex::new(None));
    let llm_provider: RwLock<Arc<dyn LlmProvider>> = RwLock::new(Arc::new(RecordingLlmProvider {
        recorded_prompt: recorded_prompt.clone(),
    }));

    let mut rx = stream_response(
        &db,
        &embedding_provider,
        &vector_store,
        &llm_provider,
        "What does Action Surge do?",
        Some("camp1"),
        "en",
    )
    .await
    .expect("stream_response should succeed");

    while rx.recv().await.is_some() {}

    let prompt = recorded_prompt
        .lock()
        .unwrap()
        .clone()
        .expect("recording mock LLM should have captured the system prompt");

    assert!(
        !prompt.contains("COMPILED RULES"),
        "no rule_entry rows exist — COMPILED RULES section must be absent: {prompt}"
    );
    assert!(
        prompt.contains("REFERENCE MATERIAL"),
        "regression: chunk block must be present and unchanged: {prompt}"
    );
    assert!(
        prompt.contains("Action Surge"),
        "chunk content should still reach the prompt: {prompt}"
    );
}
