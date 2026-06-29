use serde::Deserialize;

use super::CitationChunk;

async fn seed_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("t").use_db("t").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db.query(
        "CREATE collection SET id='col1', name='Test', \
         created_at=time::now(), updated_at=time::now()",
    )
    .await
    .unwrap();
    db.query(
        "CREATE source SET id='quickstart', filename='Quickstart.pdf', \
         display_name='Quickstart', source_type='rules', page_count=10, \
         indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5', \
         collection=type::thing('collection','col1')",
    )
    .await
    .unwrap();
    // Two chunks: one on p.9, one on p.20-22. The embedding must have
    // dimension 768 to satisfy the MTREE index; the actual values don't
    // matter for citation-lookup tests.
    let zeros: String = std::iter::repeat_n("0.0", 768)
        .collect::<Vec<_>>()
        .join(",");
    db.query(format!(
        "CREATE chunk SET id='c1', source=type::thing('source','quickstart'), \
         collection=type::thing('collection','col1'), \
         text='Lantern orbits Mirovia', page_start=9, page_end=9, \
         section_heading='Intro', source_type='rules', embedding=[{zeros}], \
         embed_model='nomic-embed-text-v1.5'"
    ))
    .await
    .unwrap()
    .check()
    .unwrap();
    db.query(format!(
        "CREATE chunk SET id='c2', source=type::thing('source','quickstart'), \
         collection=type::thing('collection','col1'), \
         text='Council factions list', page_start=20, page_end=22, \
         section_heading='Factions', source_type='rules', embedding=[{zeros}], \
         embed_model='nomic-embed-text-v1.5'"
    ))
    .await
    .unwrap()
    .check()
    .unwrap();
    db
}

/// Mirrors get_chunk_for_citation without needing a Tauri State.
async fn lookup<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    source_name: &str,
    page: Option<i64>,
) -> Option<CitationChunk> {
    let mut src_resp = db
        .query("SELECT id FROM source WHERE filename = $name LIMIT 1")
        .bind(("name", source_name.to_owned()))
        .await
        .ok()?;
    #[derive(Deserialize)]
    struct SourceIdRow {
        id: surrealdb::sql::Thing,
    }
    let src: Vec<SourceIdRow> = src_resp.take(0).ok()?;
    let src_id = src.into_iter().next()?.id;
    let sql = if page.is_some() {
        "SELECT text, page_start, page_end, section_heading FROM chunk \
         WHERE source = $src AND page_start <= $page AND page_end >= $page \
         ORDER BY page_start ASC LIMIT 1"
    } else {
        "SELECT text, page_start, page_end, section_heading FROM chunk \
         WHERE source = $src ORDER BY page_start ASC LIMIT 1"
    };
    let mut resp = db
        .query(sql)
        .bind(("src", src_id))
        .bind(("page", page))
        .await
        .ok()?;
    #[derive(Deserialize)]
    struct R {
        text: String,
        page_start: i64,
        page_end: i64,
        section_heading: String,
    }
    let rows: Vec<R> = resp.take(0).ok()?;
    rows.into_iter().next().map(|r| CitationChunk {
        text: r.text,
        page_start: r.page_start,
        page_end: r.page_end,
        section_heading: r.section_heading,
    })
}

#[tokio::test]
async fn returns_chunk_for_exact_page_hit() {
    let db = seed_db().await;
    let got = lookup(&db, "Quickstart.pdf", Some(9)).await.unwrap();
    assert_eq!(got.text, "Lantern orbits Mirovia");
    assert_eq!(got.page_start, 9);
    assert_eq!(got.section_heading, "Intro");
}

#[tokio::test]
async fn returns_chunk_when_page_in_range() {
    let db = seed_db().await;
    let got = lookup(&db, "Quickstart.pdf", Some(21)).await.unwrap();
    assert_eq!(got.text, "Council factions list");
    assert_eq!(got.page_start, 20);
    assert_eq!(got.page_end, 22);
}

#[tokio::test]
async fn returns_none_for_unknown_source() {
    let db = seed_db().await;
    assert!(lookup(&db, "Nonexistent.pdf", Some(1)).await.is_none());
}

#[tokio::test]
async fn returns_none_for_page_with_no_chunk() {
    let db = seed_db().await;
    assert!(lookup(&db, "Quickstart.pdf", Some(99)).await.is_none());
}

#[tokio::test]
async fn returns_first_chunk_when_page_omitted() {
    let db = seed_db().await;
    let got = lookup(&db, "Quickstart.pdf", None).await.unwrap();
    // page_start=9 is earlier than page_start=20
    assert_eq!(got.page_start, 9);
}
