use super::format::{notes_excerpt, NOTES_EXCERPT_LEN};
use super::{build_context, fetch_entity_context, resolve_collection_ids};

// ── Context building tests ───────────────────────────────────────────────

#[test]
fn test_build_context_empty() {
    let ctx = build_context(&[]);
    assert!(ctx.is_empty());
}

#[test]
fn test_build_context_with_results() {
    use chronacle_providers::vector_store::SearchResult;

    let results = vec![SearchResult {
        chunk_id: "chunk-1".into(),
        source_id: "source:abc".into(),
        source_name: "PHB.pdf".into(),
        text: "A fighter can use Action Surge once per rest.".into(),
        page_start: 72,
        page_end: 72,
        section_heading: "Fighter Class Features".into(),
        source_type: "rules".into(),
        distance: 0.15,
    }];

    let ctx = build_context(&results);
    assert!(!ctx.is_empty());
    assert!(ctx.contains("PHB.pdf"));
    assert!(ctx.contains("p. 72-72"));
    assert!(ctx.contains("Action Surge"));
}

#[test]
fn notes_excerpt_collapses_and_truncates() {
    assert_eq!(notes_excerpt(None), None);
    assert_eq!(notes_excerpt(Some("   ")), None);
    assert_eq!(
        notes_excerpt(Some("line one\n\nline  two")),
        Some("line one line two".to_string())
    );
    let long = "x ".repeat(400); // 400 single-char words
    let out = notes_excerpt(Some(&long)).unwrap();
    assert!(out.ends_with('…'), "expected ellipsis: {out}");
    assert_eq!(out.chars().count(), NOTES_EXCERPT_LEN + 1);
}

// ── Collection resolution tests ──────────────────────────────────────────

#[tokio::test]
async fn resolve_collection_ids_returns_subscribed_ids() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    db.query(
        "CREATE collection SET id='col1', name='C1', created_at=time::now(), updated_at=time::now(); \
         CREATE collection SET id='col2', name='C2', created_at=time::now(), updated_at=time::now(); \
         CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()"
    ).await.unwrap();
    db.query(
        "LET $in = type::thing('campaign','camp1');
         LET $out1 = type::thing('collection','col1');
         LET $out2 = type::thing('collection','col2');
         RELATE $in->subscribes_to->$out1 SET created_at=time::now();
         RELATE $in->subscribes_to->$out2 SET created_at=time::now()",
    )
    .await
    .unwrap();

    let ids = resolve_collection_ids(&db, "camp1").await.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"col1".to_string()));
    assert!(ids.contains(&"col2".to_string()));
}

#[tokio::test]
async fn resolve_collection_ids_empty_for_no_subscriptions() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();

    let ids = resolve_collection_ids(&db, "camp1").await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn fetch_entity_context_returns_empty_when_no_entities() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(result.is_empty(), "expected empty string, got: {result:?}");
}

#[tokio::test]
async fn fetch_entity_context_includes_player_character_fields() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "CREATE player_character SET id='pc1', \
         name='Nazirdijan', player_name='Nico', character_class='Wizard', \
         character_level=5, status='active', summary=NULL, notes=NULL, \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('campaign','camp1'); \
         LET $dst = type::thing('player_character','pc1'); \
         RELATE $src->in_campaign->$dst SET created_at = time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("[player_character] Nazirdijan"),
        "missing entity line: {result}"
    );
    assert!(
        result.contains("Player: Nico"),
        "missing player_name: {result}"
    );
    assert!(result.contains("Class: Wizard"), "missing class: {result}");
    assert!(result.contains("Level: 5"), "missing level: {result}");
    assert!(
        result.contains("Status: active"),
        "missing status: {result}"
    );
}
