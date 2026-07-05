//! Tests for proposal distillation, resolution, and the accept/reject service.

use std::sync::Arc;

use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

use super::proposals::*;
use crate::extraction_service::test_support::{MockEmbeddingProvider, MockLlm};

async fn setup_db() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

/// Seed campaign `camp1` with an owned collection `own1`, a subscription to it,
/// and one npc `Mira` in `own1`.
async fn seed_campaign(db: &Surreal<Db>) {
    db.query(
        "CREATE campaign:`camp1` SET name='C', system='5e', created_at=time::now(), updated_at=time::now();
         CREATE collection:`own1` SET name='C — Notes', description=NULL, owner_campaign=campaign:`camp1`,
             created_at=time::now(), updated_at=time::now();
         RELATE campaign:`camp1`->subscribes_to->collection:`own1` SET created_at=time::now();
         CREATE npc:`mira` SET name='Mira', summary='A sage', notes=NULL,
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
}

#[tokio::test]
async fn distill_chat_answer_creates_targeted_pending_proposals() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[
            {"kind":"entity_article_update","target_name":"Mira",
             "proposed_text":"Mira is the sage of Vethara.","rationale":"Answer established her origin."},
            {"kind":"new_entity","target_name":"Vethara","entity_kind":"location",
             "proposed_text":"A mountain city.","rationale":"New place named in the answer."}
        ]}"#,
    ));
    let n = distill_chat_answer(&db, &llm, "camp1", "Mira hails from Vethara …")
        .await
        .unwrap();
    assert_eq!(n, 2);

    let rows = list_proposals(&db, Some("pending")).await.unwrap();
    assert_eq!(rows.len(), 2);
    let update = rows
        .iter()
        .find(|p| p.kind == "entity_article_update")
        .unwrap();
    assert_eq!(update.target_name.as_deref(), Some("Mira"));
    assert!(update.target.as_deref().unwrap().starts_with("npc:"));
    assert_eq!(update.origin_kind, "chat");
    let fresh = rows.iter().find(|p| p.kind == "new_entity").unwrap();
    assert!(fresh.target.is_none());
    assert_eq!(fresh.payload.entity_kind.as_deref(), Some("location"));
}

#[tokio::test]
async fn distill_skips_unresolvable_update_targets_and_caps_output() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    // 10 proposals: 1 unresolvable update + 9 new entities; cap is 8.
    let mut items = vec![
        r#"{"kind":"entity_notes_update","target_name":"Nobody","proposed_text":"x","rationale":"r"}"#
            .to_string(),
    ];
    for i in 0..9 {
        items.push(format!(
            r#"{{"kind":"new_entity","target_name":"E{i}","entity_kind":"npc","proposed_text":"t","rationale":"r"}}"#
        ));
    }
    let json = format!(r#"{{"proposals":[{}]}}"#, items.join(","));
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(&json));
    let n = distill_chat_answer(&db, &llm, "camp1", "answer")
        .await
        .unwrap();
    assert_eq!(
        n, MAX_PROPOSALS_PER_DISTILL,
        "capped and unresolvable skipped"
    );
}

#[tokio::test]
async fn garbage_llm_output_yields_zero_proposals_not_an_error() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> =
        Arc::new(MockLlm::with_response("not json at all"));
    let n = distill_chat_answer(&db, &llm, "camp1", "answer")
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn distill_session_notes_marks_mentions_stale_and_is_idempotent_on_resave() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE session:`s1` SET campaign = campaign:`camp1`, session_number = 1, \
             title = 'Session 1', date_played = '2026-07-05', \
             notes = 'Mira revealed she hails from Vethara.', created_at = time::now()",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[
            {"kind":"entity_article_update","target_name":"Mira",
             "proposed_text":"Mira is the sage of Vethara.","rationale":"Session notes established her origin."}
        ],"mentioned":["Mira"]}"#,
    ));

    let n = distill_session_notes(&db, &llm, "s1").await.unwrap();
    assert_eq!(n, 1);

    // The mentioned known entity is marked stale for recompilation.
    #[derive(serde::Deserialize)]
    struct StaleRow {
        codex_stale: bool,
    }
    let mut resp = db
        .query("SELECT codex_stale FROM npc:`mira`")
        .await
        .unwrap();
    let rows: Vec<StaleRow> = resp.take(0).unwrap();
    assert!(rows.first().unwrap().codex_stale);

    let pending = list_proposals(&db, Some("pending")).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].origin_kind, "session");

    // Re-saving the same session must not accumulate duplicate pending
    // proposals: the prior pending session-origin proposal is cleared first.
    let n2 = distill_session_notes(&db, &llm, "s1").await.unwrap();
    assert_eq!(n2, 1);
    let pending_after = list_proposals(&db, Some("pending")).await.unwrap();
    assert_eq!(
        pending_after.len(),
        1,
        "idempotent re-save must not accumulate"
    );

    // Clearing the notes and re-saving must purge the stale pending
    // proposal rather than leaving it in the queue forever.
    db.query("UPDATE session:`s1` SET notes = ''")
        .await
        .unwrap()
        .check()
        .unwrap();
    let n3 = distill_session_notes(&db, &llm, "s1").await.unwrap();
    assert_eq!(n3, 0, "empty notes create nothing");
    let pending_cleared = list_proposals(&db, Some("pending")).await.unwrap();
    assert!(
        pending_cleared.is_empty(),
        "clearing notes must purge this session's stale pending proposals"
    );
}

#[tokio::test]
async fn update_kind_with_missing_target_name_is_not_persisted_untargeted() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[
            {"kind":"entity_article_update","proposed_text":"x","rationale":"r"}
        ]}"#,
    ));
    let n = distill_chat_answer(&db, &llm, "camp1", "answer")
        .await
        .unwrap();
    assert_eq!(
        n, 0,
        "update-kind draft without target_name must be skipped"
    );

    let pending = list_proposals(&db, Some("pending")).await.unwrap();
    assert!(
        pending.is_empty(),
        "must not persist an untargeted update proposal"
    );
}

#[tokio::test]
async fn accept_article_update_applies_text_provenance_and_resolves() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"entity_article_update","target_name":"Mira",
            "proposed_text":"Mira, sage of Vethara.","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "answer")
        .await
        .unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0]
        .id
        .clone();

    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, &id).await.unwrap();

    // `codex_sources` entries embed a record link (`proposal: type::thing(...)`)
    // which surrealdb cannot deserialize into `serde_json::Value` directly, so
    // the provenance check below runs in SurrealQL instead (mirrors
    // compile_tests.rs's `codex_sources[0].source_name` pattern).
    #[derive(serde::Deserialize)]
    struct Npc {
        codex_article: Option<String>,
        codex_stale: Option<bool>,
    }
    let mut r = db
        .query("SELECT codex_article, codex_stale FROM npc:`mira`")
        .await
        .unwrap();
    let npc: Option<Npc> = r.take(0).unwrap();
    let npc = npc.unwrap();
    assert_eq!(npc.codex_article.as_deref(), Some("Mira, sage of Vethara."));
    assert_eq!(
        npc.codex_stale,
        Some(false),
        "direct article write is not stale"
    );

    #[derive(serde::Deserialize)]
    struct CountRow {
        count: i64,
    }
    let mut sr = db
        .query(
            "SELECT count() FROM npc:`mira` \
             WHERE codex_sources[0].kind = 'proposal' GROUP ALL",
        )
        .await
        .unwrap();
    let provenance_count: Option<CountRow> = sr.take(0).unwrap();
    assert_eq!(
        provenance_count.map(|c| c.count).unwrap_or(0),
        1,
        "provenance appended"
    );

    let pending = list_proposals(&db, Some("pending")).await.unwrap();
    assert!(pending.is_empty());
    let accepted = list_proposals(&db, Some("accepted")).await.unwrap();
    assert_eq!(accepted.len(), 1);
}

#[tokio::test]
async fn accept_notes_update_is_the_only_machine_path_into_notes_and_marks_stale() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"entity_notes_update","target_name":"Mira",
            "proposed_text":"Party owes Mira a favor.","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "a").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0]
        .id
        .clone();
    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, &id).await.unwrap();

    #[derive(serde::Deserialize)]
    struct Npc {
        notes: Option<String>,
        codex_stale: Option<bool>,
    }
    let mut r = db
        .query("SELECT notes, codex_stale FROM npc:`mira`")
        .await
        .unwrap();
    let npc: Option<Npc> = r.take(0).unwrap();
    let npc = npc.unwrap();
    assert_eq!(npc.notes.as_deref(), Some("Party owes Mira a favor."));
    assert_eq!(
        npc.codex_stale,
        Some(true),
        "notes edit marks the article stale"
    );
}

#[tokio::test]
async fn accept_new_entity_creates_it_in_the_proposal_collection() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"new_entity","target_name":"Vethara","entity_kind":"location",
            "proposed_text":"A mountain city.","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "a").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0]
        .id
        .clone();
    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, &id).await.unwrap();

    #[derive(Debug, serde::Deserialize)]
    struct Row {
        name: String,
    }
    let mut r = db
        .query(
            "SELECT name FROM location WHERE <-in_collection<-collection CONTAINS collection:`own1`",
        )
        .await
        .unwrap();
    let rows: Vec<Row> = r.take(0).unwrap();
    assert!(rows.iter().any(|l| l.name == "Vethara"), "{rows:?}");

    #[derive(Debug, serde::Deserialize)]
    struct EmbedRow {
        embedding: Option<Vec<f32>>,
    }
    let mut er = db
        .query("SELECT embedding FROM location WHERE name = 'Vethara'")
        .await
        .unwrap();
    let embed_rows: Vec<EmbedRow> = er.take(0).unwrap();
    assert!(
        embed_rows
            .into_iter()
            .next()
            .and_then(|r| r.embedding)
            .is_some(),
        "new_entity accept must embed the created entity"
    );
}

#[tokio::test]
async fn accept_rule_entry_update_applies_body_provenance_and_reembeds() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    // `resolve_target` only resolves against `query_all_entity_names`, which
    // is entity-table-only (ENTITY_TABLES) — rule_entry targets can never be
    // resolved via the distillation path. This is a known limitation, not
    // something this fix redesigns; the proposal row is created directly.
    db.query(
        "CREATE rule_entry:`initiative` SET collection = collection:`own1`, name = 'Initiative', \
             category = 'mechanic', body = 'old', compiled_at = time::now(), stale = true;
         CREATE codex_proposal:`p1` SET kind = 'rule_entry_update', \
             target = rule_entry:`initiative`, collection = collection:`own1`, campaign = NONE, \
             payload = { proposed_text: 'new body text', rationale: 'r' }, \
             origin = { kind: 'manual' }, status = 'pending'",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, "p1").await.unwrap();

    #[derive(serde::Deserialize)]
    struct Rule {
        body: String,
        stale: bool,
        embedding: Option<Vec<f32>>,
    }
    let mut r = db
        .query("SELECT body, stale, embedding FROM rule_entry:`initiative`")
        .await
        .unwrap();
    let rule: Option<Rule> = r.take(0).unwrap();
    let rule = rule.unwrap();
    assert_eq!(rule.body, "new body text");
    assert!(!rule.stale);
    assert!(rule.embedding.is_some(), "re-embedded after accept");

    #[derive(serde::Deserialize)]
    struct CountRow {
        count: i64,
    }
    let mut sr = db
        .query(
            "SELECT count() FROM rule_entry:`initiative` \
             WHERE sources[0].kind = 'proposal' GROUP ALL",
        )
        .await
        .unwrap();
    let provenance_count: Option<CountRow> = sr.take(0).unwrap();
    assert_eq!(
        provenance_count.map(|c| c.count).unwrap_or(0),
        1,
        "provenance appended"
    );
}

#[tokio::test]
async fn accept_new_rule_entry_creates_categorized_embedded_entry() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"new_rule_entry","target_name":"Flanking","category":"mechanic",
            "proposed_text":"body text","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "a").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0]
        .id
        .clone();
    let embed: Arc<dyn chronacle_core::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(768));
    accept_proposal(&db, &embed, &id).await.unwrap();

    #[derive(serde::Deserialize)]
    struct Rule {
        category: String,
        body: String,
        embedding: Option<Vec<f32>>,
    }
    let mut r = db
        .query(
            "SELECT category, body, embedding FROM rule_entry \
             WHERE collection = collection:`own1` AND name = 'Flanking'",
        )
        .await
        .unwrap();
    let rows: Vec<Rule> = r.take(0).unwrap();
    let rule = rows.into_iter().next().expect("rule_entry created");
    assert_eq!(rule.category, "mechanic");
    assert_eq!(rule.body, "body text");
    assert!(rule.embedding.is_some(), "re-embedded after accept");

    #[derive(serde::Deserialize)]
    struct CountRow {
        count: i64,
    }
    let mut sr = db
        .query(
            "SELECT count() FROM rule_entry \
             WHERE collection = collection:`own1` AND name = 'Flanking' \
             AND sources[0].kind = 'proposal' GROUP ALL",
        )
        .await
        .unwrap();
    let provenance_count: Option<CountRow> = sr.take(0).unwrap();
    assert_eq!(
        provenance_count.map(|c| c.count).unwrap_or(0),
        1,
        "provenance recorded on creation"
    );
}

#[tokio::test]
async fn reject_changes_nothing_and_resolves() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    let llm: Arc<dyn chronacle_core::llm::LlmProvider> = Arc::new(MockLlm::with_response(
        r#"{"proposals":[{"kind":"entity_article_update","target_name":"Mira",
            "proposed_text":"X","rationale":"r"}]}"#,
    ));
    distill_chat_answer(&db, &llm, "camp1", "a").await.unwrap();
    let id = list_proposals(&db, Some("pending")).await.unwrap()[0]
        .id
        .clone();
    reject_proposal(&db, &id).await.unwrap();

    #[derive(serde::Deserialize)]
    struct Npc {
        codex_article: Option<String>,
    }
    let mut r = db
        .query("SELECT codex_article FROM npc:`mira`")
        .await
        .unwrap();
    let npc: Option<Npc> = r.take(0).unwrap();
    assert!(
        npc.unwrap().codex_article.is_none(),
        "reject must not touch the target"
    );
    assert_eq!(maintenance_counts(&db).await.unwrap().pending_proposals, 0);

    // Rejecting an already-resolved proposal is refused, symmetric with accept.
    let err = reject_proposal(&db, &id).await.unwrap_err();
    assert!(err.contains("already"), "{err}");
}
