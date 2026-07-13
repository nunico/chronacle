use super::fetch_entity_context;

#[tokio::test]
async fn fetch_entity_context_omits_empty_sections() {
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
        "CREATE npc SET id='npc1', \
         name='Aldric the Smith', summary='village blacksmith', notes=NULL, \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('campaign','camp1'); \
         LET $dst = type::thing('npc','npc1'); \
         RELATE $src->in_campaign->$dst SET created_at = time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("[npc] Aldric the Smith"),
        "missing npc: {result}"
    );
    assert!(
        result.contains("village blacksmith"),
        "missing summary: {result}"
    );
    assert!(
        !result.contains("[player_character]"),
        "unexpected PC section: {result}"
    );
    assert!(
        !result.contains("[location]"),
        "unexpected location section: {result}"
    );
}

#[tokio::test]
async fn fetch_entity_context_includes_event_dates() {
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
        "CREATE event SET id='ev1', \
         name='Battle of Irongate', date_start='Year 312', date_end='Year 313', \
         summary=NULL, notes=NULL, is_ongoing=false, \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('campaign','camp1'); \
         LET $dst = type::thing('event','ev1'); \
         RELATE $src->in_campaign->$dst SET created_at = time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("[event] Battle of Irongate"),
        "missing event: {result}"
    );
    assert!(
        result.contains("Year 312 → Year 313"),
        "missing dates: {result}"
    );
}

#[tokio::test]
async fn fetch_entity_context_includes_entity_notes() {
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
        "CREATE npc SET id='npc1', name='Seraphina', summary='archivist', \
         notes='She secretly guards the Sunstone beneath the Iron Tower.', \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('campaign','camp1'); \
         LET $dst = type::thing('npc','npc1'); \
         RELATE $src->in_campaign->$dst SET created_at = time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("Notes: She secretly guards the Sunstone"),
        "entity notes should appear in context: {result}"
    );
}

#[tokio::test]
async fn fetch_entity_context_includes_session_notes() {
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
        "CREATE session SET id='sess1', campaign=type::thing('campaign','camp1'), \
         session_number=4, title='Shadows of the Keep', date_played='2026-06-05', \
         notes='The party freed the prisoners and burned the granary.', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("[session 4] Shadows of the Keep"),
        "session line should appear in context: {result}"
    );
    assert!(
        result.contains("Notes: The party freed the prisoners"),
        "session notes should appear in context: {result}"
    );
}

#[tokio::test]
async fn fetch_entity_context_event_empty_date_end_no_arrow() {
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
        "CREATE event SET id='ev1', \
         name='Siege of Dawnwall', date_start='Year 400', date_end='', \
         summary=NULL, notes=NULL, is_ongoing=false, \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('campaign','camp1'); \
         LET $dst = type::thing('event','ev1'); \
         RELATE $src->in_campaign->$dst SET created_at = time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("[event] Siege of Dawnwall"),
        "missing event: {result}"
    );
    assert!(result.contains("Year 400"), "missing date_start: {result}");
    assert!(
        !result.contains("→"),
        "unexpected arrow when date_end is empty: {result}"
    );
}

/// Regression test: the collection branch built `ORDER BY embedding <|10|> $vec`,
/// which SurrealDB rejects. The KNN operator must live in WHERE and ordering must
/// be by `vector::distance::knn()`.
#[tokio::test]
async fn fetch_entity_context_knn_over_collection_executes() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    // 768-dim embeddings (matches the MTREE index dimension in the schema).
    let embedding: Vec<f32> = (0..768).map(|i| (i as f32) * 0.001).collect();
    let vec_str = embedding
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");

    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now(); \
         CREATE collection SET id='col1', name='Lore', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    // NPC linked to the collection with an embedding so the KNN branch has a row.
    db.query(format!(
        "CREATE npc SET id='npc1', name='Seraphine', summary='oracle', notes=NULL, \
         embedding=[{vec_str}], embed_model='test', \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('collection','col1'); \
         LET $dst = type::thing('npc','npc1'); \
         RELATE $src->in_collection->$dst SET created_at = time::now()"
    ))
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &["col1".to_string()], Some(&embedding))
        .await
        .expect("entity-context KNN query must be valid SurrealQL");
    assert!(
        result.contains("[npc] Seraphine"),
        "collection entity missing from KNN result: {result}"
    );
}

/// Regression test: `vault_deleted != true` was added to the KNN branch's
/// WHERE clause. Verify it (a) still returns the live entity and (b) actually
/// excludes a soft-deleted one, since a plain field predicate could in
/// principle interact badly with the MTREE `<|K|>` operator the way an
/// `id IN (SELECT ...)` subquery does.
#[tokio::test]
async fn fetch_entity_context_knn_over_collection_omits_soft_deleted() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    let embedding: Vec<f32> = (0..768).map(|i| (i as f32) * 0.001).collect();
    let vec_str = embedding
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(",");

    db.query(
        "CREATE campaign SET id='camp1', name='Test', system='D&D 5e', \
         created_at=time::now(), updated_at=time::now(); \
         CREATE collection SET id='col1', name='Lore', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(format!(
        "CREATE npc SET id='npc1', name='Seraphine', summary='oracle', notes=NULL, \
         embedding=[{vec_str}], embed_model='test', \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('collection','col1'); \
         LET $dst = type::thing('npc','npc1'); \
         RELATE $src->in_collection->$dst SET created_at = time::now(); \
         CREATE npc SET id='npc2', name='Gone', summary='ghost', notes=NULL, \
         vault_deleted=true, \
         embedding=[{vec_str}], embed_model='test', \
         created_at=time::now(), updated_at=time::now(); \
         LET $src2 = type::thing('collection','col1'); \
         LET $dst2 = type::thing('npc','npc2'); \
         RELATE $src2->in_collection->$dst2 SET created_at = time::now()"
    ))
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &["col1".to_string()], Some(&embedding))
        .await
        .expect("entity-context KNN query must be valid SurrealQL");
    assert!(
        result.contains("[npc] Seraphine"),
        "live collection entity missing from KNN result: {result}"
    );
    assert!(
        !result.contains("Gone"),
        "soft-deleted entity leaked into KNN RAG context: {result}"
    );
}

#[tokio::test]
async fn entity_with_codex_article_contributes_excerpt_instead_of_summary() {
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

    // Article body longer than ARTICLE_EXCERPT_LEN (600) to pin truncation+ellipsis.
    let article = "Compiled article text ".to_string() + &"lore ".repeat(200);

    db.query(
        "CREATE npc SET id='npc1', name='Aldric the Smith', \
         summary='Old summary', notes=NULL, codex_article=$article, \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('campaign','camp1'); \
         LET $dst = type::thing('npc','npc1'); \
         RELATE $src->in_campaign->$dst SET created_at = time::now()",
    )
    .bind(("article", article))
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("Codex: Compiled article text"),
        "missing codex excerpt: {result}"
    );
    assert!(
        !result.contains("Old summary"),
        "summary should be suppressed when article present: {result}"
    );

    let excerpt_start = result.find("Codex: ").expect("codex marker missing") + "Codex: ".len();
    let excerpt_line_end = result[excerpt_start..]
        .find('\n')
        .map(|i| excerpt_start + i)
        .unwrap_or(result.len());
    let excerpt = &result[excerpt_start..excerpt_line_end];
    assert!(
        excerpt.chars().count() <= super::format::ARTICLE_EXCERPT_LEN + 1,
        "excerpt exceeds budget + ellipsis: {} chars",
        excerpt.chars().count()
    );
}

#[tokio::test]
async fn entity_without_article_renders_exactly_as_before() {
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
        "CREATE npc SET id='npc1', name='Aldric the Smith', \
         summary='village blacksmith', notes=NULL, \
         created_at=time::now(), updated_at=time::now(); \
         LET $src = type::thing('campaign','camp1'); \
         LET $dst = type::thing('npc','npc1'); \
         RELATE $src->in_campaign->$dst SET created_at = time::now()",
    )
    .await
    .unwrap();

    let result = fetch_entity_context(&db, "camp1", &[], None).await.unwrap();
    assert!(
        result.contains("[npc] Aldric the Smith · village blacksmith"),
        "regression: pre-B3b line format changed: {result}"
    );
}
