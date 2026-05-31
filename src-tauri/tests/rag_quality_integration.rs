//! Regression test for the GM agent's reply quality.
//!
//! Constructs a small in-memory "Coriolis-like" document with multi-line
//! sentences, hyphenated line breaks, and a list of named factions, then
//! verifies that retrieval returns the correct chunk for two factoid queries
//! that were failing in production.
//!
//! The tests use the real Nomic embedding model so they exercise the
//! prefix logic end-to-end. They skip cleanly if the model isn't cached.

use chronacle_lib::providers::embedding::{EmbeddingProvider, FastEmbedProvider};
use chronacle_lib::providers::vector_store::{IndexedChunk, SurrealDbVector, VectorStore};
use chronacle_lib::services::chunker::{chunk_document, ExtractedDoc, PageContent};
use chronacle_lib::services::text_normalizer::normalize;
use std::sync::Arc;
use surrealdb::Surreal;

fn coriolis_like_fixture() -> ExtractedDoc {
    let raw = "The center of the Third Horizon is the Kua system, where the space station\n\
        Coriolis orbits the green jungles of the planet Kua.\n\n\
        The council factions of today are the Consortium, a group of power-\n\
        ful corporations; the Zenithian Hegemony, the descen-\n\
        dents of the captain family onboard Zenith; the Free League, the union\n\
        of free traders; the mercenaries of the Legion; the secretive Draconites;\n\
        the divine iconocrates of the Order of the Pariah; Ahlam's Temple;\n\
        and the Church of the Icons.";
    let normalized = normalize(raw);
    ExtractedDoc {
        page_count: 1,
        text: normalized.clone(),
        pages: vec![PageContent {
            page_num: 1,
            text: normalized,
        }],
    }
}

async fn seed_index(
    db: &Surreal<surrealdb::engine::local::Db>,
    embed: &Arc<dyn EmbeddingProvider>,
) -> SurrealDbVector<surrealdb::engine::local::Db> {
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_lib::schema::run_migrations(db).await.unwrap();
    db.query(
        "CREATE source SET id='s1', filename='quickstart.pdf', display_name='Quickstart', \
         source_type='rules', page_count=1, indexed_at=time::now(), index_status='done', \
         embed_model='nomic-embed-text-v1.5'",
    )
    .await
    .unwrap();

    let doc = coriolis_like_fixture();
    let chunks = chunk_document(&doc);
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let vectors = embed.embed_documents(texts).await.unwrap();

    let store = SurrealDbVector::new(db.clone());
    let indexed: Vec<IndexedChunk> = chunks
        .iter()
        .zip(vectors)
        .enumerate()
        .map(|(i, (c, v))| IndexedChunk {
            chunk_id: format!("s1-{i}"),
            campaign_id: None,
            text: c.text.clone(),
            page_start: c.page_start,
            page_end: c.page_end,
            section_heading: c.section_heading.clone(),
            source_type: "rules".into(),
            embedding: v,
            embed_model: "nomic-embed-text-v1.5".into(),
        })
        .collect();
    store.upsert("s1", &indexed).await.unwrap();
    store
}

#[tokio::test]
async fn coriolis_orbit_question_retrieves_correct_chunk() {
    let Ok(provider) = FastEmbedProvider::try_new(None) else {
        eprintln!("Skipping — nomic model not cached");
        return;
    };
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(provider);

    // Use in-memory RocksDB so the test runs without on-disk side effects.
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let db = Surreal::new::<surrealdb::engine::local::RocksDb>(db_path)
        .await
        .unwrap();
    let store = seed_index(&db, &embed).await;

    let qv = embed
        .embed_query("What planet is Coriolis orbiting?")
        .await
        .unwrap();
    let results = store.search(&qv, None, 3).await.unwrap();
    assert!(!results.is_empty(), "search returned no results");
    let top = &results[0];
    let lower = top.text.to_lowercase();
    assert!(
        lower.contains("kua") && lower.contains("coriolis"),
        "top chunk should mention Kua + Coriolis; got: {:?}",
        top.text
    );
}

#[tokio::test]
async fn council_factions_question_retrieves_correct_chunk() {
    let Ok(provider) = FastEmbedProvider::try_new(None) else {
        eprintln!("Skipping — nomic model not cached");
        return;
    };
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(provider);

    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let db = Surreal::new::<surrealdb::engine::local::RocksDb>(db_path)
        .await
        .unwrap();
    let store = seed_index(&db, &embed).await;

    let qv = embed
        .embed_query("Which are the council factions?")
        .await
        .unwrap();
    let results = store.search(&qv, None, 3).await.unwrap();
    assert!(!results.is_empty(), "search returned no results");
    let top = &results[0];
    let lower = top.text.to_lowercase();
    assert!(
        lower.contains("consortium") && lower.contains("free league"),
        "top chunk should list factions; got: {:?}",
        top.text
    );
    // De-hyphenation worked end-to-end: no raw "-\n" or split words
    assert!(
        !top.text.contains("power-") && !top.text.contains("descen-"),
        "soft hyphens leaked into chunk: {:?}",
        top.text
    );
}
