use std::sync::Arc;

use crate::chunker::{ExtractedDoc, PageContent};

use super::db::get_source_info;
use super::pipeline::{
    embed_chunks, normalize_extracted, EMBED_FRACTION_END, EMBED_FRACTION_START,
};
use super::types::{IngestionProgress, RawChunk};

#[test]
fn normalize_extracted_removes_soft_hyphen_artifacts() {
    let raw = ExtractedDoc {
        page_count: 1,
        text: "power-\nful descen-\ndents of\nthe captain family".to_string(),
        pages: vec![PageContent {
            page_num: 1,
            text: "power-\nful descen-\ndents of\nthe captain family".to_string(),
        }],
    };
    let normalized = normalize_extracted(&raw);
    assert!(
        !normalized.text.contains("-\n"),
        "soft hyphens not removed: {:?}",
        normalized.text
    );
    assert!(normalized.text.contains("powerful"));
    assert!(normalized.text.contains("descendents"));
    assert_eq!(normalized.pages[0].text, normalized.text);
    assert_eq!(normalized.page_count, 1);
}

#[test]
fn normalize_extracted_preserves_page_boundaries() {
    let p1 = "First page paragraph.";
    let p2 = "Second page paragraph.";
    let raw = ExtractedDoc {
        page_count: 2,
        text: format!("{p1}\n{p2}"),
        pages: vec![
            PageContent {
                page_num: 1,
                text: p1.to_string(),
            },
            PageContent {
                page_num: 2,
                text: p2.to_string(),
            },
        ],
    };
    let normalized = normalize_extracted(&raw);
    assert_eq!(normalized.pages.len(), 2);
    assert_eq!(normalized.pages[0].page_num, 1);
    assert_eq!(normalized.pages[1].page_num, 2);
    assert!(normalized.text.contains(p1));
    assert!(normalized.text.contains(p2));
}

#[tokio::test]
async fn embed_chunks_emits_per_batch_progress_with_counts() {
    use chronacle_providers::embedding::MockEmbeddingProvider;
    use std::sync::Mutex;

    let provider: Arc<dyn chronacle_providers::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(8));
    // 70 chunks → spans multiple EMBED_BATCH_SIZE (32) batches.
    let chunk_count = 70;
    let chunks: Vec<RawChunk> = (0..chunk_count)
        .map(|i| RawChunk {
            text: format!("chunk number {i}"),
            page_start: 1,
            page_end: 1,
            section_heading: String::new(),
        })
        .collect();

    let updates = Arc::new(Mutex::new(Vec::<IngestionProgress>::new()));
    let captured = updates.clone();
    let on_progress = move |p: IngestionProgress| captured.lock().unwrap().push(p);

    let indexed = embed_chunks(&provider, chunks, "src1", "col1", &on_progress)
        .await
        .unwrap();

    assert_eq!(indexed.len(), chunk_count);

    let ups = updates.lock().unwrap();
    // Initial 0/total plus one per batch (ceil(70/32) = 3) → 4 updates.
    assert_eq!(ups.len(), 4, "expected granular per-batch updates: {ups:?}");

    for u in ups.iter() {
        assert_eq!(u.total, Some(chunk_count as u32));
        assert!(u.current.is_some());
    }

    assert_eq!(ups.first().unwrap().current, Some(0));
    let last = ups.last().unwrap();
    assert_eq!(last.current, Some(chunk_count as u32));
    assert!(last.step.contains("70/70"), "step was: {}", last.step);

    for w in ups.windows(2) {
        assert!(w[1].fraction >= w[0].fraction, "fractions must not regress");
    }
    assert!((ups.first().unwrap().fraction - EMBED_FRACTION_START).abs() < f32::EPSILON);
    assert!(last.fraction <= EMBED_FRACTION_END + f32::EPSILON);
}

#[tokio::test]
async fn embed_chunks_empty_emits_no_progress() {
    use chronacle_providers::embedding::MockEmbeddingProvider;
    use std::sync::Mutex;

    let provider: Arc<dyn chronacle_providers::embedding::EmbeddingProvider> =
        Arc::new(MockEmbeddingProvider::new(8));
    let updates = Arc::new(Mutex::new(Vec::<IngestionProgress>::new()));
    let captured = updates.clone();
    let on_progress = move |p: IngestionProgress| captured.lock().unwrap().push(p);

    let indexed = embed_chunks(&provider, Vec::new(), "src1", "col1", &on_progress)
        .await
        .unwrap();

    assert!(indexed.is_empty());
    assert!(updates.lock().unwrap().is_empty());
}

#[tokio::test]
async fn get_source_info_reads_collection_id() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    db.query(
        "CREATE collection SET id='col1', name='Test', created_at=time::now(), updated_at=time::now()"
    ).await.unwrap();
    db.query(
        "CREATE source SET id='src1', collection=type::thing('collection','col1'), \
         filename='test.pdf', display_name='Test', source_type='rules', page_count=0, \
         indexed_at=time::now(), index_status='pending', embed_model='nomic-embed-text-v1.5'",
    )
    .await
    .unwrap();

    let info = get_source_info(&db, "src1").await.unwrap();
    assert_eq!(info.filename, "test.pdf");
    assert_eq!(info.collection_id.as_str(), "col1");
}

#[tokio::test]
async fn get_source_info_not_found_returns_err() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();

    let result = get_source_info(&db, "does-not-exist").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not found") || msg.contains("does-not-exist"),
        "Got: {msg}"
    );
}

async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[derive(serde::Deserialize)]
struct CountRow {
    count: i64,
}

#[tokio::test]
async fn ingest_completion_marks_collection_entities_and_rules_stale() {
    let db = setup_db().await;
    db.query(
        "CREATE collection:`c1` SET name = 'Rules', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         CREATE source:`s1` SET collection = collection:`c1`, campaign = NULL, \
             filename = 'f.pdf', display_name = 'F', source_type = 'rules', \
             page_count = 0, indexed_at = time::now(), index_status = 'done', \
             embed_model = 'test';
         CREATE npc:`n1` SET name = 'Mira';
         RELATE collection:`c1`->in_collection->npc:`n1` SET created_at = time::now();
         CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
             category = 'mechanic', body = 'b', compiled_at = time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    super::mark_codex_stale_for_source(&db, "s1").await.unwrap();

    let mut resp = db
        .query("SELECT count() FROM npc WHERE codex_stale = true GROUP ALL")
        .await
        .unwrap();
    let rows: Vec<CountRow> = resp.take(0).unwrap();
    assert_eq!(rows.first().map(|r| r.count).unwrap_or(0), 1);

    let mut resp2 = db
        .query("SELECT count() FROM rule_entry WHERE stale = true GROUP ALL")
        .await
        .unwrap();
    let rows2: Vec<CountRow> = resp2.take(0).unwrap();
    assert_eq!(rows2.first().map(|r| r.count).unwrap_or(0), 1);
}

#[tokio::test]
async fn lore_source_marks_entities_but_not_rules_stale() {
    let db = setup_db().await;
    db.query(
        "CREATE collection:`c1` SET name = 'Lore', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         CREATE source:`s1` SET collection = collection:`c1`, campaign = NULL, \
             filename = 'f.pdf', display_name = 'F', source_type = 'lore', \
             page_count = 0, indexed_at = time::now(), index_status = 'done', \
             embed_model = 'test';
         CREATE npc:`n1` SET name = 'Mira';
         RELATE collection:`c1`->in_collection->npc:`n1` SET created_at = time::now();
         CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
             category = 'mechanic', body = 'b', compiled_at = time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    super::mark_codex_stale_for_source(&db, "s1").await.unwrap();

    let mut resp = db
        .query("SELECT count() FROM npc WHERE codex_stale = true GROUP ALL")
        .await
        .unwrap();
    let rows: Vec<CountRow> = resp.take(0).unwrap();
    assert_eq!(rows.first().map(|r| r.count).unwrap_or(0), 1);

    let mut resp2 = db
        .query("SELECT count() FROM rule_entry WHERE stale = true GROUP ALL")
        .await
        .unwrap();
    let rows2: Vec<CountRow> = resp2.take(0).unwrap();
    assert_eq!(rows2.first().map(|r| r.count).unwrap_or(0), 0);
}
