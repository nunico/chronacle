//! D4b: every record producer must surface the `VaultRef`(s) it changed so the
//! Tauri command layer can enqueue them — especially the compiler, whose
//! `codex_article` writes never touch `updated_at`, so a missed return would
//! leave the vault stale until the next reconcile.
//!
//! Drives the real service functions directly and asserts the returned refs
//! (or the `VaultRef`-shaped fields on their result types), since producers no
//! longer take an `outbound` parameter — enqueueing happens at the command
//! layer now.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use chronacle_core::embedding::EmbeddingProvider;
use chronacle_core::llm::{ChatMessage, LlmError, LlmProvider};
use chronacle_core::vector_store::{IndexedChunk, SearchResult, VectorStore, VectorStoreError};
use chronacle_core::VaultRef;
use chronacle_domain::session_service::{self, SessionInput};
use chronacle_extraction::codex_service;
use chronacle_extraction::entity_service::{self, EntityInput, EntityKind};
use chronacle_providers::embedding::MockEmbeddingProvider;

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
/// returned `GraphNode` gives the caller the `VaultRef` it needs to enqueue.
#[tokio::test]
async fn creating_then_updating_an_entity_returns_its_ref() {
    let db = db().await;
    seed_campaign(&db).await;

    let node = entity_service::create(
        &db,
        Some("c1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Seraphina Aldric".into(),
            ..Default::default()
        },
    )
    .await
    .expect("create");
    assert_eq!(
        VaultRef {
            table: node.kind.clone(),
            id: node.id.clone(),
        },
        VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        },
        "create must return the new entity's ref"
    );

    let updated = entity_service::update(
        &db,
        &node.id,
        EntityKind::Npc,
        EntityInput {
            name: "Seraphina Aldric".into(),
            notes: Some("Edited notes.".into()),
            ..Default::default()
        },
    )
    .await
    .expect("update");
    assert_eq!(
        VaultRef {
            table: updated.kind.clone(),
            id: updated.id.clone(),
        },
        VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        },
        "update must return the same entity's ref"
    );
}

/// The compiler is the producer that matters most: it rewrites `codex_article`
/// without touching `updated_at`, so a missed ref would leave the vault stale
/// until the next reconcile.
#[tokio::test]
async fn compiling_an_entity_article_returns_its_ref() {
    let db = db().await;
    let col_id = seed_collection(&db).await;

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

    let res = codex_service::compile_collection(&db, &llm, &embed, &vs, &col_id, |_| {})
        .await
        .expect("compile");
    assert_eq!(res.articles_compiled, 1);

    assert_eq!(
        res.compiled_refs,
        vec![VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        }],
        "compile writes codex_article but never updated_at — a missed ref is invisible"
    );
}

/// Accepting an `entity_notes_update` proposal must return the target it wrote.
#[tokio::test]
async fn accepting_an_entity_notes_proposal_returns_the_target() {
    let db = db().await;
    let col_id = seed_collection(&db).await;

    let node = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "Mira".into(),
            ..Default::default()
        },
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
    let targets = codex_service::accept_proposal(&db, &embed, "p1")
        .await
        .expect("accept");

    assert_eq!(
        targets,
        vec![VaultRef {
            table: "npc".into(),
            id: node.id.clone()
        }]
    );
}

/// Saving a session (create then update) must return the changed session's
/// ref via the returned `Session` each time.
#[tokio::test]
async fn saving_a_session_returns_its_ref() {
    let db = db().await;
    seed_campaign(&db).await;

    let session = session_service::create(
        &db,
        "c1",
        SessionInput {
            session_number: 1,
            title: "One".into(),
            date_played: "2026-01-01".into(),
            notes: String::new(),
        },
    )
    .await
    .expect("create session");
    // The command handler enqueues `VaultRef { table: "session", id: session.id }`,
    // so the contract this test guards is that `create` returns a persisted
    // session with a usable id. Assert that, not a tautology.
    assert!(
        !session.id.is_empty(),
        "create must return a persisted session id to enqueue"
    );
    assert_eq!(session.title, "One", "returned session reflects the input");

    let updated = session_service::update(
        &db,
        &session.id,
        SessionInput {
            session_number: 1,
            title: "One".into(),
            date_played: "2026-01-01".into(),
            notes: "Recap written by the GM.".into(),
        },
    )
    .await
    .expect("update session");
    assert_eq!(
        VaultRef {
            table: "session".into(),
            id: updated.id.clone(),
        },
        VaultRef {
            table: "session".into(),
            id: session.id.clone()
        }
    );
}

/// A rules compile returns each written `rule_entry`'s ref via `compiled_refs`.
#[tokio::test]
async fn a_rules_compile_returns_each_written_rule_entrys_ref() {
    let db = db().await;
    let col_id = seed_collection(&db).await;

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

    let res = codex_service::compile_rules(&db, &llm, &embed, &col_id, |_| {})
        .await
        .expect("compile rules");
    assert_eq!(res.entries_created, 1);

    assert_eq!(
        res.compiled_refs.len(),
        1,
        "one rule_entry written → one ref"
    );
    assert_eq!(res.compiled_refs[0].table, "rule_entry");
}
