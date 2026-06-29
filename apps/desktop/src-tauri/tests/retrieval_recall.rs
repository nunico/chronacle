//! Retrieval recall@5 measurement harness for Phase 1.
//!
//! Architecture mandate (`docs/architecture.md:785,888`): Phase 1 evaluates
//! top-k ANN retrieval against a TTRPG query set. If recall@5 < 70% we add a
//! cross-encoder reranker in Phase 3; if > 85% we ship without one.
//!
//! This test is `#[ignore]` by default because it requires the real Nomic
//! embedding model (~250 MB) cached on disk. Run with:
//!
//! ```sh
//! cargo test --test retrieval_recall -- --ignored --nocapture
//! ```
//!
//! The printed `recall@5` line is the headline number recorded in
//! `docs/phase-1-retrieval-eval.md`. 12 queries against four fixture PDFs is
//! enough to catch catastrophic failure (recall < 50%) and to inform the
//! Phase 3 reranker decision; the full 50-query suite from the architecture
//! spec can grow over time without blocking Phase 1 sign-off.

use chronacle_db as schema;
use chronacle_ingestion::ingestion_service;
use chronacle_ingestion::pdf_extractor::{PdfExtractor, PdfiumExtractor};
use chronacle_lib::AppState;
use chronacle_providers::blob_store::{BlobStore, LocalFileStore};
use chronacle_providers::embedding::{EmbeddingProvider, FastEmbedProvider};
use chronacle_providers::llm_provider::{LlmProvider, NoopProvider};
use chronacle_providers::vector_store::{SurrealDbVector, VectorStore};
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

/// (query, marker_substring) — `marker_substring` is text that the
/// ground-truth chunk for `query` must contain (case-insensitive). A query
/// "hits" recall@5 if any of the top-5 retrieved chunks contains the marker.
fn query_set() -> Vec<(&'static str, &'static str)> {
    vec![
        // single-column-text.pdf — Combat
        ("How is initiative determined?", "Initiative"),
        ("What is a critical hit?", "Critical Hit"),
        ("How does cover affect armor class?", "Cover"),
        ("How long is a combat round?", "six seconds"),
        // multi-column.pdf — Spellcasting
        ("How do wizards prepare their spells?", "spellbook"),
        ("What does the Fireball spell do?", "Fireball"),
        ("What are the eight schools of magic?", "Abjuration"),
        (
            "What happens to concentration when you take damage?",
            "Constitution saving throw",
        ),
        ("Can spells be cast as rituals?", "ritual"),
        // tables.pdf — Equipment
        ("How much does a dagger cost?", "Dagger"),
        ("What damage does a greatsword deal?", "Greatsword"),
        // stat-block.pdf — Monster stats
        (
            "What is the Ancient Red Dragon's armor class?",
            "Armor Class 22",
        ),
    ]
}

#[tokio::test]
#[ignore = "requires Nomic embedding model cached locally"]
async fn measure_recall_at_5() {
    let Ok(embed) = FastEmbedProvider::try_new(None) else {
        eprintln!("Skipping recall@5 — Nomic model not cached. Run app once to download.");
        return;
    };
    if !pdfium_lib_path().exists() {
        eprintln!("Skipping recall@5 — pdfium library missing");
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("recall.db");
    let db = surrealdb::engine::any::connect(format!("rocksdb://{}", db_path.display()))
        .await
        .expect("rocksdb");
    db.use_ns("recall").use_db("recall").await.unwrap();
    schema::run_migrations(&db).await.expect("migrations");

    let pdfs_dir = tmp.path().join("pdfs");
    tokio::fs::create_dir_all(&pdfs_dir).await.unwrap();

    let blob_store: Arc<dyn BlobStore> = Arc::new(LocalFileStore::new(pdfs_dir));
    let vector_store: Arc<dyn VectorStore> = Arc::new(SurrealDbVector::new(db.clone()));
    let embedding_provider: Arc<dyn EmbeddingProvider> = Arc::new(embed);
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(NoopProvider);
    let pdf_extractor: Arc<dyn PdfExtractor> = Arc::new(PdfiumExtractor::new(pdfium_lib_path()));

    db.query(
        "CREATE collection SET id='col1', name='Recall', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    let state = Arc::new(AppState {
        db: db.clone(),
        llm_provider: RwLock::new(llm_provider),
        vector_store: vector_store.clone(),
        blob_store: blob_store.clone(),
        embedding_provider: RwLock::new(embedding_provider.clone()),
        pdf_extractor,
        chat_task: tokio::sync::Mutex::new(None),
        extract_task: tokio::sync::Mutex::new(None),
    });

    let fixtures = [
        "single-column-text.pdf",
        "multi-column.pdf",
        "tables.pdf",
        "stat-block.pdf",
    ];
    for fixture in fixtures {
        let path = fixture_path(fixture);
        if !path.exists() {
            panic!("missing fixture {fixture}");
        }
        let source_id = format!("src-{}", fixture.replace('.', "-"));
        db.query(
            "CREATE source SET id = $id, collection = type::thing('collection','col1'), \
             filename = $filename, display_name = $filename, source_type='rules', \
             page_count=0, indexed_at=time::now(), index_status='pending', \
             embed_model='nomic-embed-text-v1.5'",
        )
        .bind(("id", source_id.clone()))
        .bind(("filename", fixture.to_owned()))
        .await
        .unwrap()
        .check()
        .unwrap();
        let bytes = tokio::fs::read(&path).await.unwrap();
        blob_store.store(&source_id, fixture, &bytes).await.unwrap();
        ingestion_service::ingest_source(
            &state.db,
            &state.blob_store,
            &state.pdf_extractor,
            &state.embedding_provider,
            &state.vector_store,
            &source_id,
            std::sync::Arc::new(|_| {}),
        )
        .await
        .unwrap_or_else(|e| panic!("ingest {fixture}: {e}"));
    }

    let queries = query_set();
    let mut hits: usize = 0;
    let mut misses: Vec<&str> = Vec::new();
    for (q, marker) in &queries {
        let qv = embedding_provider.embed_query(q).await.unwrap();
        let results = vector_store
            .search(&qv, &["col1".to_string()], 5)
            .await
            .unwrap();
        let marker_lc = marker.to_lowercase();
        let hit = results
            .iter()
            .any(|r| r.text.to_lowercase().contains(&marker_lc));
        if hit {
            hits += 1;
        } else {
            misses.push(q);
            eprintln!(
                "MISS: q={q:?} marker={marker:?}\n  top results: {:?}",
                results
                    .iter()
                    .take(5)
                    .map(|r| {
                        let s = &r.text;
                        s.chars().take(120).collect::<String>()
                    })
                    .collect::<Vec<_>>()
            );
        }
    }

    let total = queries.len();
    let recall = hits as f32 / total as f32;
    eprintln!("\n==============================");
    eprintln!("recall@5 = {hits}/{total} = {:.1}%", recall * 100.0);
    eprintln!("==============================");
    if !misses.is_empty() {
        eprintln!("Missed queries:");
        for q in &misses {
            eprintln!("  - {q}");
        }
    }

    // Fail the test only if recall falls below the catastrophic-failure floor.
    // 50% is well below the 70% Phase 3 reranker trigger, so anything under
    // 50% indicates a real regression in the retrieval pipeline.
    assert!(
        recall >= 0.50,
        "recall@5 catastrophically low: {:.1}% (threshold 50%)",
        recall * 100.0
    );
}
