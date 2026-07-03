//! Reference-rule matrix (ADR-009). Mirrors the spec's A2 BDD scenarios
//! (backend-only; see apps/desktop/tests/e2e/features/README.md).

use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

use crate::entity_service::{relate, relate_collapsing, EntityError};

async fn setup_db() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

/// Seed: campaign `cam1` owns collection `owned1` and subscribes to regular
/// `reg1`; regular `reg2` is unrelated. One npc in each collection.
async fn seed(db: &Surreal<Db>) {
    db.query(
        "CREATE campaign:`cam1` SET name = 'C', system = 'x', \
             created_at = time::now(), updated_at = time::now();
         CREATE collection:`owned1` SET name = 'Own', description = NULL, \
             owner_campaign = campaign:`cam1`, created_at = time::now(), updated_at = time::now();
         CREATE collection:`reg1` SET name = 'R1', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         CREATE collection:`reg2` SET name = 'R2', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         RELATE campaign:`cam1`->subscribes_to->collection:`owned1` SET created_at = time::now();
         RELATE campaign:`cam1`->subscribes_to->collection:`reg1` SET created_at = time::now();
         CREATE npc:`own_a` SET name = 'OwnA';
         CREATE npc:`own_b` SET name = 'OwnB';
         CREATE npc:`r1_a` SET name = 'R1A';
         CREATE npc:`r2_a` SET name = 'R2A';
         RELATE collection:`owned1`->in_collection->npc:`own_a` SET created_at = time::now();
         RELATE collection:`owned1`->in_collection->npc:`own_b` SET created_at = time::now();
         RELATE collection:`reg1`->in_collection->npc:`r1_a` SET created_at = time::now();
         RELATE collection:`reg2`->in_collection->npc:`r2_a` SET created_at = time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
}

#[tokio::test]
async fn same_collection_relation_is_allowed() {
    let db = setup_db().await;
    seed(&db).await;
    relate(&db, "own_a", "npc", "own_b", "npc", "allied_with", None)
        .await
        .expect("same-collection edges are always legal");
}

#[tokio::test]
async fn campaign_bound_to_subscribed_regular_is_allowed_both_directions() {
    let db = setup_db().await;
    seed(&db).await;
    relate(&db, "own_a", "npc", "r1_a", "npc", "knows", None)
        .await
        .expect("campaign-bound → subscribed regular is legal");
    relate(&db, "r1_a", "npc", "own_a", "npc", "knows", None)
        .await
        .expect("the check is symmetric on the pair (ADR-010 cross-edges)");
}

#[tokio::test]
async fn campaign_bound_to_unsubscribed_regular_is_rejected() {
    let db = setup_db().await;
    seed(&db).await;
    let err = relate(&db, "own_a", "npc", "r2_a", "npc", "knows", None)
        .await
        .expect_err("cam1 does not subscribe to reg2");
    assert!(matches!(err, EntityError::ScopeViolation { .. }));
}

#[tokio::test]
async fn relation_between_two_regular_collections_is_rejected() {
    let db = setup_db().await;
    seed(&db).await;
    let err = relate(&db, "r1_a", "npc", "r2_a", "npc", "knows", None)
        .await
        .expect_err("regular collections may only self-reference");
    assert!(matches!(err, EntityError::ScopeViolation { .. }));
}

#[tokio::test]
async fn unscoped_legacy_entities_are_not_blocked() {
    let db = setup_db().await;
    seed(&db).await;
    db.query("CREATE npc:`floating` SET name = 'Ghost'")
        .await
        .unwrap();
    relate(&db, "floating", "npc", "own_a", "npc", "knows", None)
        .await
        .expect("entities without scope edges (legacy/tests) must not be blocked");
}

#[tokio::test]
async fn rejected_write_preserves_existing_lower_tier_edge() {
    let db = setup_db().await;
    seed(&db).await;
    // A legacy 'mentioned' edge between a cross-scope pair (predates
    // enforcement): insert directly, bypassing relate()'s guard.
    db.query("RELATE npc:`r1_a`->relates_to->npc:`r2_a` SET rel_type = 'mentioned', created_at = time::now()")
        .await
        .unwrap()
        .check()
        .unwrap();

    // A specific relationship on the same pair must be rejected on scope…
    let err = relate_collapsing(&db, "r1_a", "npc", "r2_a", "npc", "allied_with", None)
        .await
        .expect_err("regular↔regular stays forbidden");
    assert!(matches!(err, EntityError::ScopeViolation { .. }));

    // …and must NOT have destroyed the pre-existing edge.
    let mut resp = db
        .query("SELECT count() FROM relates_to WHERE rel_type = 'mentioned' GROUP ALL")
        .await
        .unwrap();
    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let rows: Vec<C> = resp.take(0).unwrap();
    assert_eq!(
        rows.first().map(|c| c.count).unwrap_or(0),
        1,
        "rejected write must not delete pre-existing edges"
    );
}

#[tokio::test]
async fn in_campaign_direct_edge_governs_scope() {
    let db = setup_db().await;
    seed(&db).await;
    db.query(
        "CREATE npc:`direct_a` SET name = 'DirectA';
         RELATE campaign:`cam1`->in_campaign->npc:`direct_a` SET created_at = time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    // cam1 subscribes to reg1 → allowed.
    relate_collapsing(&db, "direct_a", "npc", "r1_a", "npc", "allied_with", None)
        .await
        .expect("campaign governed via direct in_campaign edge, subscribed collection is legal");

    // cam1 does not subscribe to reg2 → rejected.
    let err = relate_collapsing(&db, "direct_a", "npc", "r2_a", "npc", "allied_with", None)
        .await
        .expect_err("cam1 does not subscribe to reg2");
    assert!(matches!(err, EntityError::ScopeViolation { .. }));
}
