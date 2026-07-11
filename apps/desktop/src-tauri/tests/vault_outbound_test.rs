//! D4b: every record producer must enqueue its `VaultRef` after a successful
//! write — especially the compiler, whose `codex_article` writes never touch
//! `updated_at`, so a missed enqueue would leave the vault stale until the next
//! reconcile.
//!
//! Drives the real service functions with a `SpyOutbound` that records every
//! enqueue, asserting one enqueue per producer.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::mpsc;

use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmError, LlmProvider};
use chronacle_core::vector_store::{IndexedChunk, SearchResult, VectorStore, VectorStoreError};
use chronacle_core::{VaultOutbound, VaultRef};
use chronacle_domain::session_service::{self, SessionInput};
use chronacle_extraction::codex_service;
use chronacle_extraction::entity_service::{self, EntityInput, EntityKind};
use chronacle_providers::embedding::MockEmbeddingProvider;

/// Records every enqueue for assertion.
#[derive(Default)]
struct SpyOutbound {
    seen: Mutex<Vec<VaultRef>>,
}

impl VaultOutbound for SpyOutbound {
    fn enqueue(&self, target: VaultRef) {
        self.seen.lock().unwrap().push(target);
    }
}

impl SpyOutbound {
    fn refs(&self) -> Vec<VaultRef> {
        self.seen.lock().unwrap().clone()
    }
}

/// A mock LLM that always returns the same response body.
struct MockLlm {
    response: String,
}

#[async_trait]
impl LlmProvider for MockLlm {
    fn provider_type(&self) -> &'static str {
        "mock-outbound"
    }

    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _messages: &[ChatMessage],
    ) -> Result<mpsc::Receiver<Result<String, LlmError>>, LlmError> {
        let (tx, rx) = mpsc::channel(1);
        let resp = self.response.clone();
        tokio::spawn(async move {
            let _ = tx.send(Ok(resp)).await;
        });
        Ok(rx)
    }
}

/// A vector store returning a single fixed hit, so the compiler always has
/// grounding context.
struct OneHitVectorStore {
    hit: SearchResult,
}

#[async_trait]
impl VectorStore for OneHitVectorStore {
    async fn upsert(&self, _s: &str, _c: &[IndexedChunk]) -> Result<(), VectorStoreError> {
        Ok(())
    }
    async fn search(
        &self,
        _q: &[f32],
        _cids: &[String],
        _limit: u64,
    ) -> Result<Vec<SearchResult>, VectorStoreError> {
        Ok(vec![self.hit.clone()])
    }
    async fn delete_by_source(&self, _s: &str) -> Result<(), VectorStoreError> {
        Ok(())
    }
}

fn passage_hit() -> SearchResult {
    SearchResult {
        chunk_id: "chunk:p1".into(),
        source_id: "src1".into(),
        source_name: "Core Rulebook".into(),
        text: "Mira, innkeeper of the Gilded Flagon.".into(),
        page_start: 12,
        page_end: 13,
        section_heading: "Factions".into(),
        source_type: "lore".into(),
        distance: 0.1,
    }
}

async fn db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db
}

/// Seed a collection and return its bare id.
async fn seed_collection(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) -> String {
    #[derive(serde::Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query(
            "CREATE collection SET name='Core', description=NULL, \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    rows.into_iter().next().unwrap().id.id.to_raw()
}

async fn seed_campaign(db: &surrealdb::Surreal<surrealdb::engine::local::Db>) {
    db.query(
        "CREATE campaign:c1 SET name='C', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
}

/// Given a configured vault, when the GM creates or edits an entity, then the
/// corresponding record is enqueued for export.
#[tokio::test]
async fn creating_then_updating_an_entity_enqueues_it_each_time() {
    let db = db().await;
    seed_campaign(&db).await;
    let spy = Arc::new(SpyOutbound::default());

    let node = entity_service::create(
        &db,
        Some("c1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Seraphina Aldric".into(),
            ..Default::default()
        },
        spy.as_ref(),
    )
    .await
    .expect("create");
    assert_eq!(
        spy.refs(),
        vec![VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        }],
        "create must enqueue the new entity"
    );

    entity_service::update(
        &db,
        &node.id,
        EntityKind::Npc,
        EntityInput {
            name: "Seraphina Aldric".into(),
            notes: Some("Edited notes.".into()),
            ..Default::default()
        },
        spy.as_ref(),
    )
    .await
    .expect("update");
    assert_eq!(
        spy.refs().len(),
        2,
        "update must enqueue the entity a second time"
    );
    assert_eq!(
        spy.refs()[1],
        VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        }
    );
}

/// The compiler is the producer that matters most: it rewrites `codex_article`
/// without touching `updated_at`, so a missed enqueue would leave the vault
/// stale until the next reconcile.
#[tokio::test]
async fn compiling_an_entity_article_enqueues_it() {
    let db = db().await;
    let col_id = seed_collection(&db).await;
    let spy = Arc::new(SpyOutbound::default());

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
        // Ignore the create enqueue via a throwaway spy; this test asserts the
        // compile path only.
        &chronacle_core::NoopOutbound,
    )
    .await
    .expect("create");
    db.query("UPDATE type::thing('npc', $id) SET codex_stale = true")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: "Mira runs the Gilded Flagon. [Source: \"Core Rulebook\", p.12]".into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let vs: Arc<dyn VectorStore> = Arc::new(OneHitVectorStore { hit: passage_hit() });

    let res =
        codex_service::compile_collection(&db, &llm, &embed, &vs, &col_id, |_| {}, spy.as_ref())
            .await
            .expect("compile");
    assert_eq!(res.articles_compiled, 1);

    assert_eq!(
        spy.refs(),
        vec![VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        }],
        "compile writes codex_article but never updated_at — a missed enqueue is invisible"
    );
}

/// Accepting an `entity_notes_update` proposal must enqueue the target it wrote.
#[tokio::test]
async fn accepting_an_entity_notes_proposal_enqueues_the_target() {
    let db = db().await;
    let col_id = seed_collection(&db).await;
    let spy = Arc::new(SpyOutbound::default());

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
    .expect("create");

    db.query(
        "CREATE codex_proposal:p1 SET kind='entity_notes_update', \
             target=type::thing('npc', $id), \
             collection=type::thing('collection', $cid), \
             payload={ proposed_text: 'New notes.', rationale: 'r' }, \
             origin={ kind: 'manual' }, status='pending', created_at=time::now()",
    )
    .bind(("id", node.id.clone()))
    .bind(("cid", col_id.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();

    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    codex_service::accept_proposal(&db, &embed, "p1", spy.as_ref())
        .await
        .expect("accept");

    assert_eq!(
        spy.refs(),
        vec![VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        }]
    );
}

/// Saving a session (create then update) must enqueue it each time.
#[tokio::test]
async fn saving_a_session_enqueues_it() {
    let db = db().await;
    seed_campaign(&db).await;
    let spy = Arc::new(SpyOutbound::default());

    let session = session_service::create(
        &db,
        "c1",
        SessionInput {
            session_number: 1,
            title: "One".into(),
            date_played: "2026-01-01".into(),
            notes: String::new(),
        },
        spy.as_ref(),
    )
    .await
    .expect("create session");
    assert_eq!(
        spy.refs(),
        vec![VaultRef {
            table: "session".into(),
            id: session.id.clone()
        }]
    );

    session_service::update(
        &db,
        &session.id,
        SessionInput {
            session_number: 1,
            title: "One".into(),
            date_played: "2026-01-01".into(),
            notes: "Recap written by the GM.".into(),
        },
        spy.as_ref(),
    )
    .await
    .expect("update session");
    assert_eq!(spy.refs().len(), 2);
    assert_eq!(
        spy.refs()[1],
        VaultRef {
            table: "session".into(),
            id: session.id.clone()
        }
    );
}

/// A rules compile enqueues each written `rule_entry`.
#[tokio::test]
async fn a_rules_compile_enqueues_each_written_rule_entry() {
    let db = db().await;
    let col_id = seed_collection(&db).await;
    let spy = Arc::new(SpyOutbound::default());

    // Seed a rules-typed source + chunk so the rules pipeline has content.
    let zeros = std::iter::repeat_n("0.0", 768)
        .collect::<Vec<_>>()
        .join(",");
    db.query(format!(
        "CREATE source SET id='src1', filename='rules.pdf', display_name='Core Rules', \
             source_type='rules', page_count=10, indexed_at=time::now(), index_status='done', \
             embed_model='mock', collection=type::thing('collection',$cid); \
         CREATE chunk SET id='chunk1', text='Roll d20 to grapple.', page_start=10, page_end=11, \
             section_heading='Combat', source_type='', \
             source=type::thing('source','src1'), \
             collection=type::thing('collection',$cid), \
             embedding=[{zeros}], embed_model='mock';"
    ))
    .bind(("cid", col_id.clone()))
    .await
    .unwrap()
    .check()
    .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{"entries":[{"name":"Grappling","category":"procedure",
            "body":"Roll a contested check to grapple.",
            "page_refs":[{"source_name":"Core Rules","page_start":10,"page_end":11}]}]}"#
            .into(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));

    let res = codex_service::compile_rules(&db, &llm, &embed, &col_id, |_| {}, spy.as_ref())
        .await
        .expect("compile rules");
    assert_eq!(res.entries_created, 1);

    let refs = spy.refs();
    assert_eq!(refs.len(), 1, "one rule_entry written → one enqueue");
    assert_eq!(refs[0].table, "rule_entry");
}
