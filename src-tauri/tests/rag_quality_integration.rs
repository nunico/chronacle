//! Regression test for the GM agent's reply quality.
//!
//! Constructs a small in-memory document about an invented fictional setting,
//! with multi-line sentences, hyphenated line breaks across line boundaries,
//! and a list of named factions, then verifies that retrieval returns the
//! correct chunk for two factoid queries that were failing in production.
//!
//! The fixture is fully original prose — no third-party game text — so the
//! test exercises the structural invariants (de-hyphenation, faction-name
//! retention, ranking) without depending on any copyrighted material.
//!
//! The tests use the real Nomic embedding model so they exercise the
//! prefix logic end-to-end. They skip cleanly if the model isn't cached.

use chronacle_lib::providers::embedding::{EmbeddingProvider, FastEmbedProvider};
use chronacle_lib::providers::vector_store::{IndexedChunk, SurrealDbVector, VectorStore};
use chronacle_lib::services::chunker::{chunk_document, ExtractedDoc, PageContent};
use chronacle_lib::services::text_normalizer::normalize;
use std::sync::Arc;
use surrealdb::Surreal;

fn multi_faction_fixture() -> ExtractedDoc {
    let raw = "The center of the Ember Reach is the Velmar system, where the space station\n\
        Lantern orbits the silver clouds of the planet Mirovia.\n\n\
        The council factions of today are the Concordat, an alliance of power-\n\
        ful corporations; the Mariner Brotherhood, the descen-\n\
        dents of the founding crews aboard Lantern; the Stellar Guild, the union\n\
        of free traders; the mercenaries of the Ironhold Pact; the secretive Silent Concord;\n\
        the devout chantkeepers of the Lumen Order; the Verdant Synod;\n\
        and the Outer Reach Cartel.";
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
        "CREATE collection SET id='col1', name='Test', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "CREATE source SET id='s1', filename='quickstart.pdf', display_name='Quickstart', \
         source_type='rules', page_count=1, indexed_at=time::now(), index_status='done', \
         embed_model='nomic-embed-text-v1.5', collection=type::thing('collection','col1')",
    )
    .await
    .unwrap();

    let doc = multi_faction_fixture();
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
            collection_id: "col1".to_string(),
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
async fn orbital_station_question_retrieves_correct_chunk() {
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
        .embed_query("What planet is Lantern orbiting?")
        .await
        .unwrap();
    let results = store.search(&qv, &["col1".to_string()], 3).await.unwrap();
    assert!(!results.is_empty(), "search returned no results");
    let top = &results[0];
    let lower = top.text.to_lowercase();
    assert!(
        lower.contains("mirovia") && lower.contains("lantern"),
        "top chunk should mention Mirovia + Lantern; got: {:?}",
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
    let results = store.search(&qv, &["col1".to_string()], 3).await.unwrap();
    assert!(!results.is_empty(), "search returned no results");
    let top = &results[0];
    let lower = top.text.to_lowercase();
    assert!(
        lower.contains("concordat") && lower.contains("stellar guild"),
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

/// Regression test for the SurrealDB KNN search bug: with `<|1|>` used in the
/// SELECT expression, every chunk got `distance = f64::MAX` and retrieval
/// returned chunks in storage order. With many chunks and one specific target,
/// random order would almost never put the target at #1 — this test would
/// catch that.
#[tokio::test]
async fn retrieval_ranks_target_chunk_above_distractors() {
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
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_lib::schema::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE collection SET id='col1', name='Test', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "CREATE source SET id='s1', filename='quickstart.pdf', display_name='Quickstart', \
         source_type='rules', page_count=1, indexed_at=time::now(), index_status='done', \
         embed_model='nomic-embed-text-v1.5', collection=type::thing('collection','col1')",
    )
    .await
    .unwrap();

    // 20 distractor chunks about unrelated TTRPG topics + 1 target chunk
    // mentioning Lantern orbiting Mirovia.
    let mut texts: Vec<String> = (0..20)
        .map(|i| {
            format!(
                "Distractor passage {i}: combat rules, dice mechanics, weapon ranges. \
                 Light weapons fit in half a row; heavy weapons need two rows. \
                 Rolling six sixes is a critical success."
            )
        })
        .collect();
    let target =
        "The space station Lantern orbits the silver clouds of the planet Mirovia in the Velmar system.";
    texts.push(target.to_string());

    let vectors = embed.embed_documents(texts.clone()).await.unwrap();
    let store = SurrealDbVector::new(db);
    let indexed: Vec<IndexedChunk> = texts
        .iter()
        .zip(vectors)
        .enumerate()
        .map(|(i, (t, v))| IndexedChunk {
            chunk_id: format!("s1-{i}"),
            collection_id: "col1".to_string(),
            text: t.clone(),
            page_start: 1,
            page_end: 1,
            section_heading: String::new(),
            source_type: "rules".into(),
            embedding: v,
            embed_model: "nomic-embed-text-v1.5".into(),
        })
        .collect();
    store.upsert("s1", &indexed).await.unwrap();

    let qv = embed
        .embed_query("What planet does Lantern orbit?")
        .await
        .unwrap();
    let results = store.search(&qv, &["col1".to_string()], 5).await.unwrap();

    assert!(!results.is_empty(), "search returned no results");

    // 1. Distance must be a real number (cosine distance ∈ [0, 2]), not the
    //    f64::MAX fallback that signalled the broken-query bug.
    for r in &results {
        assert!(
            r.distance.is_finite() && r.distance < 10.0,
            "distance must be a real cosine value, got {} (was the search query returning bool again?)",
            r.distance
        );
    }

    // 2. Distances must be DIFFERENT (the bug made them all tie at MAX).
    let distinct_distances: std::collections::HashSet<u64> =
        results.iter().map(|r| r.distance.to_bits()).collect();
    assert!(
        distinct_distances.len() > 1,
        "all distances equal — search not actually ranking. distances={:?}",
        results.iter().map(|r| r.distance).collect::<Vec<_>>()
    );

    // 3. The target chunk must be #1 — distractors are about unrelated topics
    //    and shouldn't beat a chunk that literally answers the question.
    let top = &results[0];
    assert!(
        top.text.contains("Lantern") && top.text.contains("Mirovia"),
        "target chunk should rank #1; got top: {:?}",
        top.text
    );
}
