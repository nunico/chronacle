//! Tests for the manual lint pass detectors (ADR-009 C2).

use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

use super::lint::*;

async fn setup_db() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

/// Seed campaign `camp1` with an owned collection `own1`.
async fn seed_campaign(db: &Surreal<Db>) {
    db.query(
        "CREATE campaign:`camp1` SET name='C', system='5e', created_at=time::now(), updated_at=time::now();
         CREATE collection:`own1` SET name='C — Notes', description=NULL, owner_campaign=campaign:`camp1`,
             created_at=time::now(), updated_at=time::now();
         RELATE campaign:`camp1`->subscribes_to->collection:`own1` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
}

/// Count unresolved findings of `kind`.
///
/// Deliberately NOT `SELECT count() ... GROUP ALL`: in this SurrealDB
/// version that aggregate form silently ignores the `WHERE` filter when the
/// filtered field (`kind`) is indexed, returning the table's total row
/// count instead (verified empirically against a two-kind fixture). Plain
/// row selection + `.len()` is unaffected and used throughout `lint.rs` for
/// the same reason.
async fn kind_count(db: &Surreal<Db>, kind: &str) -> i64 {
    #[derive(serde::Deserialize)]
    struct IdRow {
        // Existence check only; the id value itself is never read.
        #[allow(dead_code)]
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query("SELECT id FROM lint_finding WHERE kind = $kind AND resolved_at = NONE")
        .bind(("kind", kind.to_owned()))
        .await
        .unwrap();
    let rows: Vec<IdRow> = resp.take(0).unwrap();
    rows.len() as i64
}

#[tokio::test]
async fn broken_wikilink_is_found_and_clears_when_entity_exists() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`mira` SET name='Mira', summary='A sage', \
             notes='See [[Nonexistent]] and [[Mira]]', \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(kind_count(&db, "broken_wikilink").await, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        payload: serde_json::Value,
    }
    let mut resp = db
        .query("SELECT payload FROM lint_finding WHERE kind = 'broken_wikilink'")
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].payload.get("link_text").and_then(|v| v.as_str()),
        Some("Nonexistent")
    );

    // Create the previously-missing entity; re-running must not add a NEW
    // broken_wikilink finding (the old one persists until resolved).
    db.query(
        "CREATE npc:`nonexistent` SET name='Nonexistent', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`nonexistent` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(
        kind_count(&db, "broken_wikilink").await,
        1,
        "no new broken_wikilink finding once the target exists"
    );
}

#[tokio::test]
async fn duplicate_entity_flags_same_named_pairs_in_scope() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`k1` SET name='Korim', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`k2` SET name='korim', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`k1` SET created_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`k2` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(kind_count(&db, "duplicate_entity").await, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        payload: serde_json::Value,
    }
    let mut resp = db
        .query("SELECT payload FROM lint_finding WHERE kind = 'duplicate_entity'")
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    let p = &rows[0].payload;
    assert!(p.get("a").and_then(|v| v.as_str()).is_some());
    assert!(p.get("b").and_then(|v| v.as_str()).is_some());
    assert_eq!(p.get("similarity").and_then(|v| v.as_f64()), Some(1.0));

    // Idempotent re-run: no second finding for the same pair.
    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(kind_count(&db, "duplicate_entity").await, 1);
}

#[tokio::test]
async fn stale_article_aggregates_needs_compile_entities() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`mira` SET name='Mira', summary='S', notes=NULL, codex_stale=true, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(kind_count(&db, "stale_article").await, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        payload: serde_json::Value,
    }
    let mut resp = db
        .query("SELECT payload FROM lint_finding WHERE kind = 'stale_article'")
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0]
        .payload
        .get("reason")
        .and_then(|v| v.as_str())
        .is_some());
}

#[tokio::test]
async fn scope_violation_found_for_pre_enforcement_edge() {
    let db = setup_db().await;
    // Two independent regular collections (not campaign-subscribed to each
    // other), each with one entity, and a relates_to edge created RAW via
    // db.query — bypassing entity_service's check_scope, simulating legacy
    // pre-enforcement data.
    db.query(
        "CREATE collection:`ca` SET name='CA', description=NULL, owner_campaign=NONE, \
             created_at=time::now(), updated_at=time::now();
         CREATE collection:`cb` SET name='CB', description=NULL, owner_campaign=NONE, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`a` SET name='A', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`b` SET name='B', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`ca`->in_collection->npc:`a` SET created_at=time::now();
         RELATE collection:`cb`->in_collection->npc:`b` SET created_at=time::now();
         RELATE npc:`a`->relates_to->npc:`b` SET rel_type='knows', notes=NULL, created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_collection(&db, "ca").await.unwrap();
    assert_eq!(kind_count(&db, "scope_violation").await, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        payload: serde_json::Value,
    }
    let mut resp = db
        .query("SELECT payload FROM lint_finding WHERE kind = 'scope_violation'")
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0]
        .payload
        .get("edge")
        .and_then(|v| v.as_str())
        .is_some());
    assert_eq!(
        rows[0].payload.get("from").and_then(|v| v.as_str()),
        Some("npc:a")
    );
    assert_eq!(
        rows[0].payload.get("to").and_then(|v| v.as_str()),
        Some("npc:b")
    );
}

#[tokio::test]
async fn lint_pass_is_idempotent_no_duplicate_findings() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`mira` SET name='Mira', summary='A sage', \
             notes='See [[Nonexistent]]', \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    run_lint_campaign(&db, "camp1").await.unwrap();
    let summary = run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(
        summary.new_findings, 0,
        "no new entities added between runs → zero new findings on repeat"
    );
}

#[tokio::test]
async fn resolve_lint_finding_sets_resolved_at() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`mira` SET name='Mira', summary='S', notes=NULL, codex_stale=true, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    let findings = list_lint_findings(&db).await.unwrap();
    assert!(!findings.is_empty());
    let before = findings.len();

    resolve_lint_finding(&db, &findings[0].id).await.unwrap();

    let after = list_lint_findings(&db).await.unwrap();
    assert_eq!(after.len(), before - 1);
}
