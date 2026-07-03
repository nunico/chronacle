//! Schema-level tests for the Codex slice of `002_wiki_layer.surql` (A2a).
//!
//! Mirrors the BDD scenarios in the codex spec that have no UI surface
//! (see apps/desktop/tests/e2e/features/README.md for the convention).

use serde::Deserialize;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[derive(Deserialize)]
struct CountRow {
    count: i64,
}

async fn count(db: &Surreal<Db>, query: &str) -> i64 {
    let mut resp = db.query(query).await.unwrap();
    let rows: Vec<CountRow> = resp.take(0).unwrap();
    rows.first().map(|r| r.count).unwrap_or(0)
}

#[tokio::test]
async fn codex_fields_default_on_entity_tables() {
    let db = setup_db().await;
    db.query("CREATE npc SET name = 'Mira'").await.unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count() FROM npc WHERE codex_stale = false \
               AND codex_article = NONE AND codex_sources = [] GROUP ALL",
        )
        .await,
        1,
        "codex fields must default to not-stale / no article / empty provenance"
    );
}

#[tokio::test]
async fn rule_entry_accepts_all_seven_categories() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    for cat in [
        "mechanic",
        "ability",
        "state",
        "procedure",
        "resource",
        "statistic",
        "entry",
    ] {
        db.query(
            "CREATE rule_entry SET collection = collection:`c1`, name = $name, \
             category = $cat, body = 'b', compiled_at = time::now()",
        )
        .bind(("name", format!("rule-{cat}")))
        .bind(("cat", cat.to_owned()))
        .await
        .unwrap()
        .check()
        .unwrap_or_else(|e| panic!("category {cat} must be accepted: {e}"));
    }
    assert_eq!(
        count(&db, "SELECT count() FROM rule_entry GROUP ALL").await,
        7
    );
}

#[tokio::test]
async fn rule_entry_rejects_unknown_category() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    let res = db
        .query(
            "CREATE rule_entry SET collection = collection:`c1`, name = 'bad', \
             category = 'vibe', body = 'b', compiled_at = time::now()",
        )
        .await
        .unwrap()
        .check();
    assert!(res.is_err(), "unknown category must be rejected by ASSERT");
}

#[tokio::test]
async fn rule_entry_name_unique_per_collection() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    let create = "CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
                  category = 'mechanic', body = 'b', compiled_at = time::now()";
    db.query(create).await.unwrap().check().unwrap();
    let dup = db.query(create).await.unwrap().check();
    assert!(dup.is_err(), "(collection, name) must be UNIQUE");
}

#[tokio::test]
async fn codex_proposal_defaults_to_pending() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Notes', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    db.query(
        "CREATE codex_proposal SET kind = 'entity_article_update', \
         target = npc:`n1`, collection = collection:`c1`, \
         payload = { proposed_text: 'x', rationale: 'y' }, \
         origin = { kind: 'manual' }",
    )
    .await
    .unwrap()
    .check()
    .expect("codex_proposal must accept a minimal row");
    assert_eq!(
        count(
            &db,
            "SELECT count() FROM codex_proposal WHERE status = 'pending' \
               AND resolved_at = NONE GROUP ALL",
        )
        .await,
        1
    );
}

#[tokio::test]
async fn migration_002_a2_is_idempotent_and_preserves_rule_entries() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    db.query(
        "CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
         category = 'mechanic', body = 'b', compiled_at = time::now()",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    assert_eq!(
        count(&db, "SELECT count() FROM rule_entry GROUP ALL").await,
        1
    );
}

/// SCHEMAFULL tables strip undefined nested keys from object-typed fields
/// unless the field is declared FLEXIBLE. This is the invariant test for
/// every opaque-payload field in the codex schema: nested content must
/// round-trip, not silently collapse to `{}` / `[{}]`.
#[tokio::test]
async fn opaque_object_fields_preserve_nested_content() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Notes', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    db.query(
        "CREATE lint_finding SET kind = 'scope_violation', \
             payload = { from: 'npc:a', to: 'npc:b' };
         CREATE codex_proposal SET kind = 'entity_article_update', \
             target = npc:`n1`, collection = collection:`c1`, \
             payload = { proposed_text: 'x', rationale: 'y' }, \
             origin = { kind: 'chat', message: 'message:m1' };
         CREATE npc SET name = 'Mira', \
             codex_sources = [{ kind: 'chunk', source: 'source:s1', page_start: 3, page_end: 4 }];
         CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
             category = 'mechanic', body = 'b', compiled_at = time::now(), \
             page_refs = [{ source: 'source:s1', page_start: 12, page_end: 13 }];",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    assert_eq!(
        count(
            &db,
            "SELECT count() FROM lint_finding WHERE payload.`from` = 'npc:a' \
               AND payload.`to` = 'npc:b' GROUP ALL",
        )
        .await,
        1,
        "lint_finding.payload must preserve nested keys (FLEXIBLE)"
    );
    assert_eq!(
        count(
            &db,
            "SELECT count() FROM codex_proposal WHERE payload.proposed_text = 'x' \
               AND origin.kind = 'chat' GROUP ALL",
        )
        .await,
        1,
        "codex_proposal payload/origin must preserve nested keys (FLEXIBLE)"
    );
    assert_eq!(
        count(
            &db,
            "SELECT count() FROM npc WHERE codex_sources[0].page_start = 3 GROUP ALL",
        )
        .await,
        1,
        "entity codex_sources entries must preserve nested keys (FLEXIBLE)"
    );
    assert_eq!(
        count(
            &db,
            "SELECT count() FROM rule_entry WHERE page_refs[0].page_start = 12 GROUP ALL",
        )
        .await,
        1,
        "rule_entry.page_refs entries must preserve nested keys (FLEXIBLE)"
    );
}
