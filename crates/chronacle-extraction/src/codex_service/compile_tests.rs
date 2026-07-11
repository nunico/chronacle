use std::sync::Arc;

use crate::codex_service::compile::compile_targets_with_cap;
use crate::codex_service::{compile_collection, compile_entity, CodexPhase, CompileProgress};
use crate::entity_service::{self, EntityInput, EntityKind};
use crate::extraction_service::test_support::{
    setup_db_with_collection, MockEmbeddingProvider, MockLlm, MockVectorStore, RecordingVectorStore,
};
use async_trait::async_trait;
use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmError, LlmProvider};
use chronacle_core::vector_store::{SearchResult, VectorStore};

fn passage_hit(text: &str) -> SearchResult {
    SearchResult {
        chunk_id: "chunk:p1".into(),
        source_id: "src1".into(),
        source_name: "Core Rulebook".into(),
        text: text.into(),
        page_start: 12,
        page_end: 13,
        section_heading: "Factions".into(),
        source_type: "lore".into(),
        distance: 0.1,
    }
}

/// Proves "nothing stale → no LLM cost": panics if the compiler ever invokes it.
struct PanickingLlm;

#[async_trait]
impl LlmProvider for PanickingLlm {
    fn provider_type(&self) -> &'static str {
        "panicking"
    }

    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        panic!("compile_collection must not call the LLM when nothing is stale");
    }
}

#[tokio::test]
async fn compile_writes_article_provenance_and_clears_stale() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Mira".into(),
            summary: Some("An innkeeper.".into()),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    db.query("UPDATE type::thing('npc', $id) SET codex_stale = true")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "Mira runs the Gilded Flagon. [Source: \"Core Rulebook\", p.12]".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![passage_hit("Mira, innkeeper of the Gilded Flagon…")],
    });

    let res = compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &col_id,
        |_| {},
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    assert_eq!(res.articles_compiled, 1);
    assert_eq!(res.remaining_stale, 0);

    let got = entity_service::get_by_id(&db, &node.id, EntityKind::Npc)
        .await
        .unwrap();
    assert!(got
        .codex_article
        .as_deref()
        .unwrap_or("")
        .contains("Gilded Flagon"));
    assert_eq!(got.codex_stale, Some(false));

    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let mut resp = db
        .query(
            "SELECT count() FROM npc WHERE codex_sources[0].source_name = 'Core Rulebook' \
               AND codex_sources[0].page_start = 12 GROUP ALL",
        )
        .await
        .unwrap();
    let rows: Vec<C> = resp.take(0).unwrap();
    assert_eq!(
        rows.first().map(|c| c.count).unwrap_or(0),
        1,
        "chunk provenance must persist"
    );
}

#[tokio::test]
async fn compile_skips_fresh_entities_and_makes_no_llm_calls() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Mira".into(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    db.query("UPDATE type::thing('npc', $id) SET codex_stale = false, codex_article = 'done'")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(PanickingLlm);
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });

    let res = compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &col_id,
        |_| {},
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    assert_eq!(res.articles_compiled, 0);
}

#[tokio::test]
async fn compile_unset_stale_legacy_entity_is_included() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Old One".into(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    db.query("UPDATE type::thing('npc', $id) UNSET codex_stale")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "Ancient.".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![passage_hit("The Old One…")],
    });
    let res = compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &col_id,
        |_| {},
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    assert_eq!(
        res.articles_compiled, 1,
        "unset codex_stale must count as stale"
    );
}

#[tokio::test]
async fn compile_emits_done_phase_with_counts() {
    let (db, col_id) = setup_db_with_collection().await;
    entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Mira".into(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "Article.".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![passage_hit("Mira…")],
    });
    let events = std::sync::Mutex::new(Vec::<CompileProgress>::new());
    compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &col_id,
        |p| {
            events.lock().unwrap().push(p);
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let events = events.into_inner().unwrap();
    assert_eq!(events.last().unwrap().phase, CodexPhase::Done);
    assert_eq!(events.last().unwrap().compiled, 1);
    assert!(
        events.iter().any(|e| e.phase == CodexPhase::Embedding),
        "compile must emit an Embedding phase event"
    );
}

/// An LLM that fails on the first call and succeeds afterwards.
struct FlakyLlm {
    calls: std::sync::atomic::AtomicUsize,
}

#[async_trait]
impl LlmProvider for FlakyLlm {
    fn provider_type(&self) -> &'static str {
        "flaky"
    }

    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        let call = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if call == 0 {
            return Err(LlmError::Connection("simulated transient failure".into()));
        }
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        tokio::spawn(async move {
            let _ = tx.send(Ok("Recovered article.".to_string())).await;
        });
        Ok(rx)
    }
}

#[tokio::test]
async fn compile_continues_past_one_failing_entity() {
    let (db, col_id) = setup_db_with_collection().await;
    // Two stale entities; names chosen so the failing one sorts first
    // (compile order is kind then name).
    for name in ["Aaa", "Bbb"] {
        entity_service::create(
            &db,
            None,
            Some(&col_id),
            EntityKind::Npc,
            EntityInput {
                name: name.to_string(),
                ..Default::default()
            },
            &chronacle_core::NoopOutbound,
        )
        .await
        .unwrap();
    }
    let llm: Arc<dyn LlmProvider> = Arc::new(FlakyLlm {
        calls: Default::default(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![passage_hit("Both NPCs…")],
    });

    let res = compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &col_id,
        |_| {},
        &chronacle_core::NoopOutbound,
    )
    .await
    .expect("one flaky entity must not fail the run");
    assert_eq!(
        res.articles_compiled, 1,
        "the non-failing entity still compiled"
    );
}

#[tokio::test]
async fn compile_entity_without_passages_returns_false_and_leaves_article() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Ghost".into(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "unused".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore { results: vec![] });
    let compiled = compile_entity(
        &db,
        &llm,
        &embed,
        &vs,
        "npc",
        &node.id,
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    assert!(
        !compiled,
        "no context → no article, no hallucinated compile"
    );
}

#[tokio::test]
async fn compile_targets_honestly_reports_remaining_past_cap() {
    let (db, col_id) = setup_db_with_collection().await;
    for name in ["Aaa", "Bbb", "Ccc"] {
        entity_service::create(
            &db,
            None,
            Some(&col_id),
            EntityKind::Npc,
            EntityInput {
                name: name.to_string(),
                ..Default::default()
            },
            &chronacle_core::NoopOutbound,
        )
        .await
        .unwrap();
    }
    let (targets, remaining) = compile_targets_with_cap(&db, &col_id, 2).await.unwrap();
    assert_eq!(targets.len(), 2, "cap must limit the batch size");
    assert_eq!(
        remaining, 1,
        "remaining_stale must honestly report entities left after the cap"
    );
}

#[tokio::test]
async fn compile_with_empty_article_leaves_entity_stale() {
    let (db, col_id) = setup_db_with_collection().await;
    let node = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Mira".into(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    db.query("UPDATE type::thing('npc', $id) SET codex_stale = true")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "   ".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(MockVectorStore {
        results: vec![passage_hit("Mira, innkeeper of the Gilded Flagon…")],
    });

    let res = compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &col_id,
        |_| {},
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    assert_eq!(
        res.articles_compiled, 0,
        "an empty LLM response must not count as a compiled article"
    );

    let got = entity_service::get_by_id(&db, &node.id, EntityKind::Npc)
        .await
        .unwrap();
    assert_eq!(
        got.codex_stale,
        Some(true),
        "entity must stay stale when the article is empty"
    );
    assert!(
        got.codex_article.is_none(),
        "an empty article must not be persisted"
    );
}

#[tokio::test]
async fn campaign_bound_compile_searches_full_subscription_set() {
    let (db, owned_id) = setup_db_with_collection().await;
    // Make the collection campaign-bound with a two-collection subscription set.
    db.query(
        "CREATE campaign:`cam1` SET name = 'C', system = 'x', \
             created_at = time::now(), updated_at = time::now();
         CREATE collection:`reg1` SET name = 'Shared', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         UPDATE type::thing('collection', $owned) SET owner_campaign = campaign:`cam1`;
         LET $own = type::thing('collection', $owned);
         RELATE campaign:`cam1`->subscribes_to->$own SET created_at = time::now();
         RELATE campaign:`cam1`->subscribes_to->collection:`reg1` SET created_at = time::now();",
    )
    .bind(("owned", owned_id.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();

    entity_service::create(
        &db,
        None,
        Some(&owned_id),
        EntityKind::Npc,
        EntityInput {
            name: "Mira".into(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "Article.".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let recording = Arc::new(RecordingVectorStore {
        results: vec![passage_hit("Mira…")],
        calls: Default::default(),
    });
    let vs: Arc<dyn VectorStore> = recording.clone();

    compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &owned_id,
        |_| {},
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();

    let calls = recording.calls.lock().unwrap();
    let ids = calls.first().expect("search was called");
    let mut sorted = ids.clone();
    sorted.sort();
    let mut expected = vec![owned_id.clone(), "reg1".to_string()];
    expected.sort();
    assert_eq!(
        sorted, expected,
        "campaign-bound compile must search the owner's full subscription set"
    );
}

#[tokio::test]
async fn regular_compile_searches_only_itself() {
    let (db, col_id) = setup_db_with_collection().await;
    entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Mira".into(),
            ..Default::default()
        },
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "Article.".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let recording = Arc::new(RecordingVectorStore {
        results: vec![passage_hit("Mira…")],
        calls: Default::default(),
    });
    let vs: Arc<dyn VectorStore> = recording.clone();
    compile_collection(
        &db,
        &llm,
        &embed,
        &vs,
        &col_id,
        |_| {},
        &chronacle_core::NoopOutbound,
    )
    .await
    .unwrap();
    let calls = recording.calls.lock().unwrap();
    assert_eq!(
        calls.first().unwrap(),
        &vec![col_id.clone()],
        "regular compile must search only its own collection"
    );
}
