//! Tier 4: an elided link auto-resolves ONLY when there is exactly one
//! sensible answer — and it leaves a trace when it does.

use chronacle_extraction::wikilink::{parse_and_sync_wikilinks, WikilinkScope};

async fn seed(records: &str) -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .expect("in-memory db");
    db.use_ns("t").use_db("t").await.expect("use ns/db");
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db.query(records)
        .await
        .expect("seed query")
        .check()
        .expect("seed ok");
    db
}

/// `faction:q` ("The Quassar Family") and `npc:s` ("Seraphina"), both in
/// campaign `c1`.
async fn seeded_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    seed(
        "CREATE campaign:c1 SET name = 'Test Campaign', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE faction:q SET name = 'The Quassar Family', aliases = [], \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:s SET name = 'Seraphina', aliases = [], \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->faction:q SET created_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:s SET created_at = time::now();",
    )
    .await
}

/// Two factions whose normalized names both plausibly match "The Quassars":
/// `faction:qf` ("The Quassar Family") and `faction:qc` ("The Quassar
/// Cartel"), plus `npc:s` ("Seraphina"), all in campaign `c1`.
async fn seeded_db_two_quassars() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    seed(
        "CREATE campaign:c1 SET name = 'Test Campaign', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE faction:qf SET name = 'The Quassar Family', aliases = [], \
             created_at = time::now(), updated_at = time::now(); \
         CREATE faction:qc SET name = 'The Quassar Cartel', aliases = [], \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:s SET name = 'Seraphina', aliases = [], \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->faction:qf SET created_at = time::now(); \
         RELATE campaign:c1->in_campaign->faction:qc SET created_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:s SET created_at = time::now();",
    )
    .await
}

#[tokio::test]
async fn an_elided_link_auto_resolves_and_persists_the_alias() {
    let db = seeded_db().await; // faction:q "The Quassar Family", npc:s "Seraphina"

    let matched = parse_and_sync_wikilinks(
        &db,
        "npc",
        "s",
        "Met [[The Quassars]] today.",
        WikilinkScope::Campaign { campaign_id: "c1" },
    )
    .await
    .expect("resolve");

    assert_eq!(
        matched,
        vec!["faction:q"],
        "the elided link must find the family"
    );

    // It must have REMEMBERED. The next pass hits tier 2, not the fuzzy path:
    // fuzzy runs once per variant, ever.
    let aliases: Vec<String> = db
        .query("SELECT VALUE aliases FROM faction:q")
        .await
        .unwrap()
        .take::<Vec<Vec<String>>>(0)
        .unwrap()
        .remove(0);
    assert!(aliases.iter().any(|a| a == "The Quassars"));

    // And it must be REVIEWABLE — nothing happens behind the GM's back.
    let findings: Vec<serde_json::Value> = db
        .query("SELECT payload FROM lint_finding WHERE kind = 'auto_alias'")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert_eq!(findings.len(), 1, "an auto-alias must be listed for review");
}

#[tokio::test]
async fn an_ambiguous_link_refuses_to_guess_and_offers_candidates() {
    let db = seeded_db_two_quassars().await; // "The Quassar Family" AND "The Quassar Cartel"

    let matched = parse_and_sync_wikilinks(
        &db,
        "npc",
        "s",
        "Met [[The Quassars]] today.",
        WikilinkScope::Campaign { campaign_id: "c1" },
    )
    .await
    .expect("resolve");

    assert!(
        matched.is_empty(),
        "two candidates means it must NOT pick one"
    );

    // No alias was written to either.
    let all: Vec<Vec<String>> = db
        .query("SELECT VALUE aliases FROM faction")
        .await
        .unwrap()
        .take(0)
        .unwrap();
    assert!(
        all.iter().all(|a| a.is_empty()),
        "an ambiguous link must write no alias"
    );
}
