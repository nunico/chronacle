//! Tests for proposal distillation, resolution, and the accept/reject service.

use std::sync::Arc;

use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

use super::proposals::*;
use crate::extraction_service::test_support::MockLlm;

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
