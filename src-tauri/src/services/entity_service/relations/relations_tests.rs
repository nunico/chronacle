use serde::Deserialize;

use super::edge;
use super::{relate, relate_collapsing};
use crate::services::entity_service::{create, EntityInput, EntityKind, GraphEdge};

#[test]
fn rel_specificity_tiers_match_vocab() {
    assert_eq!(edge::rel_specificity("mentioned"), 0);
    assert_eq!(edge::rel_specificity("related_to"), 1);
    assert_eq!(edge::rel_specificity("knows"), 1);
    // Specific directional types and unknown custom verbs are all tier 2.
    assert_eq!(edge::rel_specificity("member_of"), 2);
    assert_eq!(edge::rel_specificity("located_in"), 2);
    assert_eq!(edge::rel_specificity("enemy_of"), 2);
    assert_eq!(edge::rel_specificity("betrays"), 2);
}

fn make_edge(from: &str, to: &str, rel: &str) -> GraphEdge {
    let (ft, fi) = from.split_once(':').unwrap();
    let (tt, ti) = to.split_once(':').unwrap();
    GraphEdge {
        from_id: fi.to_string(),
        from_kind: ft.to_string(),
        to_id: ti.to_string(),
        to_kind: tt.to_string(),
        rel_type: rel.to_string(),
        notes: None,
    }
}

fn pair_key(e: &GraphEdge) -> (String, String) {
    let a = format!("{}:{}", e.from_kind, e.from_id);
    let b = format!("{}:{}", e.to_kind, e.to_id);
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn identity(e: &GraphEdge) -> (String, String, String, String, String) {
    (
        e.from_kind.clone(),
        e.from_id.clone(),
        e.to_kind.clone(),
        e.to_id.clone(),
        e.rel_type.clone(),
    )
}

fn collapse(edges: Vec<GraphEdge>) -> Vec<GraphEdge> {
    edge::keep_most_specific(edges, pair_key, identity, |e| e.rel_type.as_str())
}

#[test]
fn collapse_drops_generic_when_specific_exists_for_pair() {
    // Hegemony located_in Spire AND related_to Spire → keep only located_in.
    let kept = collapse(vec![
        make_edge("faction:heg", "location:spire", "located_in"),
        make_edge("faction:heg", "location:spire", "related_to"),
    ]);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].rel_type, "located_in");
}

#[test]
fn collapse_drops_mentioned_against_specific_regardless_of_direction() {
    // member_of one way, mentioned the other way, same pair → keep member_of.
    let kept = collapse(vec![
        make_edge("faction:heg", "faction:other", "member_of"),
        make_edge("faction:other", "faction:heg", "mentioned"),
    ]);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].rel_type, "member_of");
}

#[test]
fn collapse_keeps_mentioned_when_it_is_the_only_edge() {
    let kept = collapse(vec![make_edge("faction:heg", "npc:bob", "mentioned")]);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].rel_type, "mentioned");
}

#[test]
fn collapse_keeps_distinct_specific_types_for_same_pair() {
    // Two contradictory-but-specific edges both survive (same tier).
    let kept = collapse(vec![
        make_edge("faction:a", "faction:b", "allied_with"),
        make_edge("faction:a", "faction:b", "enemy_of"),
    ]);
    assert_eq!(kept.len(), 2);
}

#[test]
fn collapse_dedupes_identical_edges() {
    let kept = collapse(vec![
        make_edge("faction:a", "faction:b", "member_of"),
        make_edge("faction:a", "faction:b", "member_of"),
    ]);
    assert_eq!(kept.len(), 1);
}

#[test]
fn collapse_does_not_mix_unrelated_pairs() {
    // related_to to one entity stays when there is no specific edge to it,
    // even though a specific edge exists to a different entity.
    let kept = collapse(vec![
        make_edge("faction:heg", "location:spire", "located_in"),
        make_edge("faction:heg", "npc:bob", "related_to"),
    ]);
    assert_eq!(kept.len(), 2);
}

/// Create a campaign + two factions and return their ids.
async fn setup_pair<C: surrealdb::Connection>(db: &surrealdb::Surreal<C>) -> (String, String) {
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(db).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    let a = create(
        db,
        Some("camp1"),
        None,
        EntityKind::Faction,
        EntityInput {
            name: "Hegemony".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let b = create(
        db,
        Some("camp1"),
        None,
        EntityKind::Faction,
        EntityInput {
            name: "Syndicate".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    (a.id, b.id)
}

/// Return the rel_types of every `relates_to` edge between the two factions
/// (either direction), sorted.
async fn rel_types_between<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    a: &str,
    b: &str,
) -> Vec<String> {
    #[derive(Deserialize)]
    struct Row {
        rel_type: String,
    }
    let sql = format!(
        "SELECT rel_type FROM relates_to WHERE \
         (in = faction:{a} AND out = faction:{b}) OR \
         (in = faction:{b} AND out = faction:{a})"
    );
    let mut resp = db.query(sql).await.unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    let mut types: Vec<String> = rows.into_iter().map(|r| r.rel_type).collect();
    types.sort();
    types
}

#[tokio::test]
async fn relate_collapsing_specific_removes_existing_mentioned() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    let (a, b) = setup_pair(&db).await;
    // A pre-existing `mentioned` edge (as a wikilink would create).
    relate(&db, &a, "faction", &b, "faction", "mentioned", None)
        .await
        .unwrap();
    // A specific relationship supersedes it.
    let created = relate_collapsing(&db, &a, "faction", &b, "faction", "member_of", None)
        .await
        .unwrap();
    assert!(created, "specific edge should be created");
    assert_eq!(rel_types_between(&db, &a, &b).await, vec!["member_of"]);
}

#[tokio::test]
async fn relate_collapsing_generic_skipped_when_specific_exists() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    let (a, b) = setup_pair(&db).await;
    relate_collapsing(&db, &a, "faction", &b, "faction", "member_of", None)
        .await
        .unwrap();
    // related_to is generic (tier 1) and must not be added over member_of (tier 2).
    let created = relate_collapsing(&db, &a, "faction", &b, "faction", "related_to", None)
        .await
        .unwrap();
    assert!(!created, "generic edge should be skipped");
    assert_eq!(rel_types_between(&db, &a, &b).await, vec!["member_of"]);
}

#[tokio::test]
async fn relate_collapsing_drops_lower_tier_even_in_opposite_direction() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    let (a, b) = setup_pair(&db).await;
    // mentioned B→A, then specific A→B: the unordered pair collapses to the specific.
    relate(&db, &b, "faction", &a, "faction", "mentioned", None)
        .await
        .unwrap();
    relate_collapsing(&db, &a, "faction", &b, "faction", "enemy_of", None)
        .await
        .unwrap();
    assert_eq!(rel_types_between(&db, &a, &b).await, vec!["enemy_of"]);
}

#[tokio::test]
async fn relate_collapsing_keeps_lone_mentioned_and_coexisting_specifics() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    let (a, b) = setup_pair(&db).await;
    // A lone mentioned survives (it is the only connection).
    let created = relate_collapsing(&db, &a, "faction", &b, "faction", "mentioned", None)
        .await
        .unwrap();
    assert!(created);
    // Two same-tier specifics coexist; the mentioned is dropped.
    relate_collapsing(&db, &a, "faction", &b, "faction", "allied_with", None)
        .await
        .unwrap();
    relate_collapsing(&db, &a, "faction", &b, "faction", "enemy_of", None)
        .await
        .unwrap();
    assert_eq!(
        rel_types_between(&db, &a, &b).await,
        vec!["allied_with", "enemy_of"]
    );
}
