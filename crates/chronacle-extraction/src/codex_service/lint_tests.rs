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
async fn broken_wikilink_found_in_codex_article() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`mira` SET name='Mira', summary='A sage', notes=NONE, \
             codex_article='Mira once traveled with [[Ghostfell]].', \
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
        Some("Ghostfell")
    );

    // Idempotent re-run: no second finding for the same broken link.
    let summary = run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(
        summary.new_findings, 0,
        "no new broken_wikilink finding on repeat run"
    );
    assert_eq!(kind_count(&db, "broken_wikilink").await, 1);
}

#[tokio::test]
async fn broken_wikilink_deduped_across_notes_and_codex_article() {
    // The same broken link appearing in BOTH the user notes and the compiled
    // codex_article on one entity must yield exactly one finding in a single
    // run — the per-entity `seen_links` set suppresses the cross-field repeat.
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`mira` SET name='Mira', summary='A sage', \
             notes='See [[Ghostfell]] for context.', \
             codex_article='Mira once traveled with [[Ghostfell]].', \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    let summary = run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(
        summary.new_findings, 1,
        "one finding despite the link appearing in both fields"
    );
    assert_eq!(kind_count(&db, "broken_wikilink").await, 1);
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

/// Seed a `faction` in campaign `camp1`'s owned collection.
async fn seed_faction(db: &Surreal<Db>, id: &str, name: &str) {
    db.query(format!(
        "CREATE faction:`{id}` SET name=$name, summary=NULL, notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->faction:`{id}` SET created_at=time::now();"
    ))
    .bind(("name", name.to_owned()))
    .await
    .unwrap()
    .check()
    .unwrap();
}

/// Seed a `location` in campaign `camp1`'s owned collection.
async fn seed_location(db: &Surreal<Db>, id: &str, name: &str) {
    db.query(format!(
        "CREATE location:`{id}` SET name=$name, summary=NULL, notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->location:`{id}` SET created_at=time::now();"
    ))
    .bind(("name", name.to_owned()))
    .await
    .unwrap()
    .check()
    .unwrap();
}

/// Today "The Free League" / "Free League" hash to different exact-lowercase
/// keys and are never reported. Stage 1 of the fuzzy detector groups by the
/// shared `naming::normalize` engine instead, so a leading-article variant
/// like this is caught with no scoring at all (similarity 1.0).
#[tokio::test]
async fn duplicate_detection_catches_a_leading_article_variant() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    seed_faction(&db, "f1", "The Free League").await;
    seed_faction(&db, "f2", "Free League").await;

    run_lint_campaign(&db, "camp1").await.unwrap();

    assert_eq!(kind_count(&db, "duplicate_entity").await, 1);
}

/// A false duplicate proposes a MERGE — data loss if the GM accepts it. Two
/// distinct same-table factions ("The Legion" / "Iron Host") must not be
/// flagged, AND a same-named-ish location in a DIFFERENT table ("The
/// Legionnaire's Rest") must never pair with the "The Legion" faction no
/// matter how similar the strings look — duplicate detection never crosses
/// tables.
#[tokio::test]
async fn duplicate_detection_does_not_flag_distinct_entities() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    seed_faction(&db, "f1", "The Legion").await;
    seed_location(&db, "l1", "The Legionnaire's Rest").await;
    seed_faction(&db, "f2", "Iron Host").await;

    run_lint_campaign(&db, "camp1").await.unwrap();

    assert_eq!(kind_count(&db, "duplicate_entity").await, 0);
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

/// Fold-in fix (found during Task 3, same file scope): `lint_broken_wikilinks`
/// used to do its own independent tier-1-only string comparison, so a link
/// that resolves perfectly well via a confirmed ALIAS was still reported
/// broken. It must now route through the same `resolve_exact` the resolver
/// uses, so the linter and the resolver can never disagree.
#[tokio::test]
async fn broken_wikilink_does_not_fire_when_the_link_resolves_via_an_alias() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`ghost` SET name='Ghostfell', aliases=['Ghost'], summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`mira` SET name='Mira', summary='A sage', \
             notes='See [[Ghost]] for context.', \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`ghost` SET created_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(
        kind_count(&db, "broken_wikilink").await,
        0,
        "a link that resolves via a confirmed alias must never be reported broken"
    );
}

/// `broken_wikilink` payload must carry ranked `candidates` so the GM sees a
/// "did you mean …?" suggestion, using a lower bar than tier-4 auto-resolve
/// on purpose — a suggestion may be speculative because the GM adjudicates it.
#[tokio::test]
async fn broken_wikilink_payload_carries_candidates_for_a_near_miss() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`ghost` SET name='Ghostfell', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`mira` SET name='Mira', summary='A sage', \
             notes='See [[Ghostfel]] for context.', \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`ghost` SET created_at=time::now();
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
    let candidates = rows[0]
        .payload
        .get("candidates")
        .and_then(|v| v.as_array())
        .expect("candidates must be present, even if empty");
    assert!(
        !candidates.is_empty(),
        "a near-miss like 'Ghostfel' vs 'Ghostfell' must surface at least one candidate"
    );
}

/// Two entities in the same scope whose name/alias normalize to the same key
/// must be flagged — otherwise tier-2 resolution silently depends on row
/// order.
#[tokio::test]
async fn alias_collision_flags_entities_sharing_a_normalized_key() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`a` SET name='The Grunt', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         CREATE npc:`b` SET name='Grunts', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`a` SET created_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`b` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(kind_count(&db, "alias_collision").await, 1);

    #[derive(serde::Deserialize)]
    struct Row {
        payload: serde_json::Value,
    }
    let mut resp = db
        .query("SELECT payload FROM lint_finding WHERE kind = 'alias_collision'")
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0]
        .payload
        .get("alias")
        .and_then(|v| v.as_str())
        .is_some());
    assert!(rows[0].payload.get("a").and_then(|v| v.as_str()).is_some());
    assert!(rows[0].payload.get("b").and_then(|v| v.as_str()).is_some());

    // Idempotent re-run: no second finding for the same pair.
    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(kind_count(&db, "alias_collision").await, 1);
}

/// A name/alias that appears on only one entity — the overwhelmingly common
/// case — must never be flagged.
#[tokio::test]
async fn alias_collision_does_not_fire_for_a_lone_name() {
    let db = setup_db().await;
    seed_campaign(&db).await;
    db.query(
        "CREATE npc:`mira` SET name='Mira', summary='S', notes=NULL, \
             created_at=time::now(), updated_at=time::now();
         RELATE collection:`own1`->in_collection->npc:`mira` SET created_at=time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    run_lint_campaign(&db, "camp1").await.unwrap();
    assert_eq!(kind_count(&db, "alias_collision").await, 0);
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
