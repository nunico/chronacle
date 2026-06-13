//! Diverse-fixture ingest integration coverage.
//!
//! Architecture mandate (`docs/architecture.md:850`): "integration: full ingest
//! → query cycle using diverse PDF fixture suite (`single-column`, `multi-column`,
//! `tables`, `stat-block`, `scanned`) + `MockLlmProvider`".
//!
//! For each fixture, run the full `ingest_source` pipeline against an in-memory
//! SurrealDB and assert (a) extraction produced text, (b) at least one chunk was
//! written, (c) the chunk is searchable via the vector store. `scanned.pdf`
//! is excluded from the text/chunk assertions because OCR is out of scope for
//! Phase 1 — for that fixture we only assert "no panic, source ends in `done`".

use chronacle_lib::providers::blob_store::{BlobStore, LocalFileStore};
use chronacle_lib::providers::embedding::{EmbeddingProvider, MockEmbeddingProvider};
use chronacle_lib::providers::llm_provider::{LlmProvider, NoopProvider};
use chronacle_lib::providers::vector_store::{SurrealDbVector, VectorStore};
use chronacle_lib::schema;
use chronacle_lib::services::ingestion_service;
use chronacle_lib::services::pdf_extractor::{PdfExtractor, PdfiumExtractor};
use chronacle_lib::AppState;
use std::sync::{Arc, RwLock};

fn pdfium_lib_path() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium");
    let name = if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else if cfg!(target_os = "linux") {
        "libpdfium.so"
    } else {
        "pdfium.dll"
    };
    dir.join(name)
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdfs")
        .join(name)
}

/// Set up an AppState backed by a fresh RocksDB in `temp_dir`, the bundled
/// pdfium extractor, a no-op LLM, and a MockEmbeddingProvider (768-dim to
/// satisfy the MTREE schema).
async fn make_state(
    temp_dir: &std::path::Path,
) -> (
    Arc<AppState>,
    Arc<dyn EmbeddingProvider>,
    Arc<dyn VectorStore>,
) {
    let db_path = temp_dir.join("test.db");
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::RocksDb>(db_path)
        .await
        .expect("rocksdb");
    db.use_ns("test").use_db("test").await.unwrap();
    schema::run_migrations(&db).await.expect("migrations");

    let pdfs_dir = temp_dir.join("pdfs");
    tokio::fs::create_dir_all(&pdfs_dir)
        .await
        .expect("pdfs dir");

    let blob_store: Arc<dyn BlobStore> = Arc::new(LocalFileStore::new(pdfs_dir));
    let vector_store: Arc<dyn VectorStore> = Arc::new(SurrealDbVector::new(db.clone()));
    let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(NoopProvider);
    let pdf_extractor: Arc<dyn PdfExtractor> = Arc::new(PdfiumExtractor::new(pdfium_lib_path()));

    db.query(
        "CREATE collection SET id='col1', name='Test', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .expect("collection")
    .check()
    .expect("collection ok");

    let state = Arc::new(AppState {
        db,
        llm_provider: RwLock::new(llm_provider),
        vector_store: vector_store.clone(),
        blob_store,
        embedding_provider: RwLock::new(embedding_provider.clone()),
        pdf_extractor,
        chat_task: tokio::sync::Mutex::new(None),
        extract_task: tokio::sync::Mutex::new(None),
    });
    (state, embedding_provider, vector_store)
}

/// Create a source record + store the PDF blob, then return the source id.
async fn seed_source(state: &Arc<AppState>, fixture: &str) -> String {
    let source_id = format!("src-{}", fixture.replace('.', "-"));
    state
        .db
        .query(
            "CREATE source SET id = $id, collection = type::thing('collection','col1'), \
             filename = $filename, display_name = $filename, source_type='rules', \
             page_count=0, indexed_at=time::now(), index_status='pending', \
             embed_model='nomic-embed-text-v1.5'",
        )
        .bind(("id", source_id.clone()))
        .bind(("filename", fixture.to_owned()))
        .await
        .expect("create source")
        .check()
        .expect("create source ok");

    let bytes = tokio::fs::read(fixture_path(fixture))
        .await
        .expect("read fixture");
    state
        .blob_store
        .store(&source_id, fixture, &bytes)
        .await
        .expect("blob store");
    source_id
}

async fn count_chunks(state: &Arc<AppState>, source_id: &str) -> i64 {
    let mut res = state
        .db
        .query("SELECT count() FROM chunk WHERE source = type::thing('source', $id) GROUP ALL")
        .bind(("id", source_id.to_owned()))
        .await
        .expect("count");
    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let rows: Vec<C> = res.take(0).expect("parse");
    rows.first().map(|c| c.count).unwrap_or(0)
}

async fn source_status(state: &Arc<AppState>, source_id: &str) -> String {
    let mut res = state
        .db
        .query("SELECT index_status FROM source WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .await
        .expect("status");
    #[derive(serde::Deserialize)]
    struct R {
        index_status: String,
    }
    let rows: Vec<R> = res.take(0).expect("parse");
    rows.into_iter().next().expect("row").index_status
}

/// Drive each text-bearing fixture (single-column, multi-column, tables,
/// stat-block) through the full ingest pipeline and assert:
///   1. Ingestion succeeds.
///   2. `index_status` ends in `'done'`.
///   3. At least one chunk is written.
///   4. The chunks are searchable via the vector store.
async fn assert_text_fixture(fixture: &str) {
    if !fixture_path(fixture).exists() {
        eprintln!("Skipping {fixture} — fixture not present");
        return;
    }
    if !pdfium_lib_path().exists() {
        eprintln!("Skipping {fixture} — pdfium library not present");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let (state, embed, vector_store) = make_state(tmp.path()).await;
    let source_id = seed_source(&state, fixture).await;

    ingestion_service::ingest_source(&state, &source_id, std::sync::Arc::new(|_| {}))
        .await
        .unwrap_or_else(|e| panic!("ingest failed for {fixture}: {e}"));

    assert_eq!(
        source_status(&state, &source_id).await,
        "done",
        "{fixture} should end in done"
    );
    let n = count_chunks(&state, &source_id).await;
    assert!(
        n >= 1,
        "{fixture} should produce at least one chunk (got {n})"
    );

    // Vector search should return at least one chunk from this collection.
    let qv = embed.embed_query("any query").await.expect("embed query");
    let results = vector_store
        .search(&qv, &["col1".to_string()], 5)
        .await
        .expect("search");
    assert!(
        !results.is_empty(),
        "{fixture}: vector search should find chunks after ingestion"
    );
}

#[tokio::test]
async fn ingest_single_column_pdf() {
    assert_text_fixture("single-column-text.pdf").await;
}

#[tokio::test]
async fn ingest_multi_column_pdf() {
    assert_text_fixture("multi-column.pdf").await;
}

#[tokio::test]
async fn ingest_tables_pdf() {
    assert_text_fixture("tables.pdf").await;
}

#[tokio::test]
async fn ingest_stat_block_pdf() {
    assert_text_fixture("stat-block.pdf").await;
}

/// `scanned.pdf` has no embedded text layer (image-only). OCR is out of scope
/// for Phase 1, so we only assert: extraction does not panic, and the source
/// ends in a terminal state (either `done` with zero/few chunks, or `error`
/// because the chunker produced nothing usable). Either is acceptable — the
/// regression we care about is "doesn't panic and doesn't get stuck in
/// `indexing`".
#[tokio::test]
async fn ingest_scanned_pdf_does_not_panic() {
    let fixture = "scanned.pdf";
    if !fixture_path(fixture).exists() {
        eprintln!("Skipping {fixture} — fixture not present");
        return;
    }
    if !pdfium_lib_path().exists() {
        eprintln!("Skipping {fixture} — pdfium library not present");
        return;
    }
    let tmp = tempfile::tempdir().expect("tempdir");
    let (state, _embed, _vs) = make_state(tmp.path()).await;
    let source_id = seed_source(&state, fixture).await;

    // Result may be Ok (no text, zero chunks) or Err (chunker rejected empty
    // input). Either way the call must return without panicking.
    let _ = ingestion_service::ingest_source(&state, &source_id, std::sync::Arc::new(|_| {})).await;

    let status = source_status(&state, &source_id).await;
    assert!(
        status == "done" || status == "error",
        "scanned.pdf should end in a terminal state (done or error), got {status}"
    );
}
