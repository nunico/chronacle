//! Source commands — upload, list, delete PDF sources, re-index, and the
//! citation→chunk lookup that backs the chat citation popover.

use std::sync::Arc;

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tauri::State;

/// Response shape for a source record returned over IPC.
#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub id: String,
    pub filename: String,
    pub display_name: String,
    pub source_type: String,
    pub page_count: i64,
    pub index_status: String,
    pub embed_model: String,
    pub collection_id: Option<String>,
}

/// Uploads a source PDF file, storing it in the blob store and triggering
/// ingestion (extraction → chunking → embedding).
///
/// After the DB INSERT succeeds the ingestion pipeline runs synchronously and
/// emits `ingestion-progress` / `ingestion-error` Tauri events.
#[tauri::command]
pub async fn upload_source(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    file_path: String,
    display_name: Option<String>,
    source_type: Option<String>,
    collection_id: String,
) -> Result<serde_json::Value, String> {
    if collection_id.trim().is_empty() {
        return Err("collection_id is required".to_string());
    }

    let path = std::path::PathBuf::from(&file_path);
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let display_name = display_name.unwrap_or_else(|| filename.clone());
    let source_type = source_type.unwrap_or_else(|| "rules".to_string());
    let source_id = uuid::Uuid::new_v4().to_string();
    let embed_model = "nomic-embed-text-v1.5".to_string();

    // Read file contents
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // Store in blob store
    state
        .blob_store
        .store(&source_id, &filename, &data)
        .await
        .map_err(|e| format!("Failed to store blob: {e}"))?;

    // Insert source record — collection is bound via parameter, never interpolated.
    #[derive(Deserialize)]
    struct CreatedSource {
        #[expect(dead_code)]
        id: surrealdb::sql::Thing,
    }

    let created: Vec<CreatedSource> = state
        .db
        .query(
            "CREATE source SET
                id = $id,
                campaign = NULL,
                filename = $filename,
                display_name = $display_name,
                source_type = $source_type,
                page_count = 0,
                indexed_at = time::now(),
                index_status = 'pending',
                embed_model = $embed_model,
                collection = type::thing('collection', $collection_id)",
        )
        .bind(("id", source_id.to_owned()))
        .bind(("filename", filename.to_owned()))
        .bind(("display_name", display_name.to_owned()))
        .bind(("source_type", source_type.to_owned()))
        .bind(("embed_model", embed_model.to_owned()))
        .bind(("collection_id", collection_id.clone()))
        .await
        .map_err(|e| format!("Failed to create source record: {e}"))?
        .check()
        .map_err(|e| format!("Source INSERT violated a schema constraint: {e}"))?
        .take(0)
        .map_err(|e| format!("Failed to parse created source: {e}"))?;

    if created.is_empty() {
        return Err("Source creation failed: no record returned".to_string());
    }

    let source_json = serde_json::json!({
        "id": source_id,
        "filename": filename,
        "display_name": display_name,
        "source_type": source_type,
        "index_status": "pending",
        "embed_model": embed_model,
        "collection_id": collection_id,
    });

    // Build the progress callback — emits Tauri events from each pipeline stage
    let sid = source_id.clone();
    let handle = app_handle.clone();
    let on_progress: std::sync::Arc<
        dyn Fn(crate::services::ingestion_service::IngestionProgress) + Send + Sync,
    > = std::sync::Arc::new(
        move |p: crate::services::ingestion_service::IngestionProgress| {
            let _ = handle.emit(
                "ingestion-progress",
                serde_json::json!({
                    "source_id": sid,
                    "status": "indexing",
                    "progress": p.fraction,
                    "step": p.step,
                    "current": p.current,
                    "total": p.total,
                }),
            );
        },
    );

    let state_ref = state.inner().clone();
    let sid = source_id.clone();

    match crate::services::ingestion_service::ingest_source(&state_ref, &sid, on_progress).await {
        Ok(()) => {
            let _ = app_handle.emit(
                "ingestion-progress",
                serde_json::json!({
                    "source_id": &sid,
                    "status": "done",
                    "progress": 1.0,
                    "step": "Complete",
                }),
            );
            Ok(source_json)
        }
        Err(e) => {
            let err_msg = e.to_string();
            eprintln!("Ingestion failed for source {sid}: {err_msg}");
            let _ = state_ref
                .db
                .query(
                    "UPDATE source SET index_status = 'error' \
                     WHERE id = type::thing('source', $id)",
                )
                .bind(("id", sid.clone()))
                .await;
            let _ = app_handle.emit(
                "ingestion-error",
                serde_json::json!({
                    "source_id": &sid,
                    "error": &err_msg,
                }),
            );
            Err(format!("PDF ingestion failed: {err_msg}"))
        }
    }
}

/// Returns all sources, optionally filtered to a specific collection.
///
/// When `collection_id` is provided the query uses a parameterised binding
/// (never string interpolation) to avoid SQL-injection risks.
#[tauri::command]
pub async fn get_sources(
    state: State<'_, Arc<AppState>>,
    collection_id: Option<String>,
) -> Result<Vec<SourceResponse>, String> {
    /// Raw row shape as SurrealDB returns it.
    #[derive(Deserialize)]
    struct SourceRow {
        id: surrealdb::sql::Thing,
        filename: String,
        display_name: String,
        source_type: String,
        page_count: i64,
        index_status: String,
        embed_model: String,
        collection: Option<surrealdb::sql::Thing>,
    }

    let mut response = if let Some(ref cid) = collection_id {
        state
            .db
            .query(
                "SELECT * FROM source \
                 WHERE collection = type::thing('collection', $cid) \
                 ORDER BY display_name ASC",
            )
            .bind(("cid", cid.clone()))
            .await
            .map_err(|e| format!("Failed to query sources: {e}"))?
    } else {
        state
            .db
            .query("SELECT * FROM source ORDER BY display_name ASC")
            .await
            .map_err(|e| format!("Failed to query sources: {e}"))?
    };

    let rows: Vec<SourceRow> = response
        .take(0)
        .map_err(|e| format!("Failed to parse sources: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|r| SourceResponse {
            id: r.id.id.to_raw(),
            filename: r.filename,
            display_name: r.display_name,
            source_type: r.source_type,
            page_count: r.page_count,
            index_status: r.index_status,
            embed_model: r.embed_model,
            collection_id: r.collection.map(|t| t.id.to_raw()),
        })
        .collect())
}

/// Delete a source, its blob data, and all associated chunks.
#[tauri::command]
pub async fn delete_source(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    // Check source exists before deleting
    let mut exists = state
        .db
        .query("SELECT count() FROM source WHERE id = type::thing('source', $id) GROUP ALL")
        .bind(("id", id.clone()))
        .await
        .map_err(|e| format!("Failed to query source: {e}"))?;

    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }
    let counts: Vec<CountRow> = exists
        .take(0)
        .map_err(|e| format!("Failed to parse source count: {e}"))?;

    if counts.first().map(|c| c.count).unwrap_or(0) > 0 {
        // Delete blob
        state
            .blob_store
            .delete(&id)
            .await
            .map_err(|e| format!("Failed to delete blob: {e}"))?;

        // Delete vector chunks
        state
            .vector_store
            .delete_by_source(&id)
            .await
            .map_err(|e| format!("Failed to delete chunks: {e}"))?;

        // Delete source record
        state
            .db
            .query("DELETE type::thing('source', $id)")
            .bind(("id", id))
            .await
            .map_err(|e| format!("Failed to delete source: {e}"))?;
    }

    Ok(())
}

// ── Re-index all sources ──────────────────────────────────────────────

/// Enumerate all source IDs in the database.
async fn list_all_source_ids<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
) -> Result<Vec<String>, String> {
    #[derive(Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
    }
    let mut resp = db
        .query("SELECT id FROM source")
        .await
        .map_err(|e| e.to_string())?;
    let rows: Vec<Row> = resp.take(0).map_err(|e| e.to_string())?;
    // Use `.to_raw()` not `.to_string()`. SurrealDB's `Id::to_string()`
    // wraps string values that need escaping (e.g. UUIDs with hyphens) in
    // backticks; passing that back through `type::thing('source', $id)`
    // produces a mangled `source:`\`uuid\`` reference that never matches
    // the real record. See commit e099a79 for the prior occurrence.
    Ok(rows.into_iter().map(|r| r.id.id.to_raw()).collect())
}

/// Re-run ingestion for every source currently in the database.
///
/// For each source: delete existing chunks, then call `ingest_source` again.
/// Emits a `reindex-progress` event per pipeline tick so the UI can show
/// progress across sources.
#[tauri::command]
pub async fn reindex_all_sources(
    app_handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<usize, String> {
    let ids = list_all_source_ids(&state.db).await?;
    let total = ids.len();

    for (idx, sid) in ids.iter().enumerate() {
        let sid_for_progress = sid.clone();
        let handle = app_handle.clone();
        let on_progress: std::sync::Arc<
            dyn Fn(crate::services::ingestion_service::IngestionProgress) + Send + Sync,
        > = std::sync::Arc::new(move |p| {
            let _ = handle.emit(
                "reindex-progress",
                serde_json::json!({
                    "source_id": &sid_for_progress,
                    "current": idx + 1,
                    "total": total,
                    "progress": p.fraction,
                    "step": p.step,
                }),
            );
        });

        state
            .vector_store
            .delete_by_source(sid)
            .await
            .map_err(|e| format!("delete chunks for {sid}: {e}"))?;

        let state_ref = state.inner().clone();
        crate::services::ingestion_service::ingest_source(&state_ref, sid, on_progress)
            .await
            .map_err(|e| format!("re-ingest {sid}: {e}"))?;
    }

    Ok(total)
}

// ── Citation chunk lookup ─────────────────────────────────────────────

/// The chunk text + locator returned for a citation popover.
#[derive(Serialize)]
pub struct CitationChunk {
    pub text: String,
    pub page_start: i64,
    pub page_end: i64,
    pub section_heading: String,
}

/// Look up the chunk that backs a citation, so the UI can show the source
/// passage when the user clicks the citation badge.
///
/// `source_name` matches `source.filename`. `page` is the cited page (the
/// first number when the citation says `p.45-49`). If multiple chunks span
/// the page, the earliest one is returned. None if no chunk matches.
#[tauri::command]
pub async fn get_chunk_for_citation(
    state: State<'_, Arc<AppState>>,
    source_name: String,
    page: Option<i64>,
) -> Result<Option<CitationChunk>, String> {
    // Resolve the source.id first via filename so the chunk query can use
    // it directly. Doing this in two steps avoids relying on SurrealDB's
    // record-link filtering inside WHERE, which the MTREE optimizer has
    // surprised us with before.
    let mut src_resp = state
        .db
        .query("SELECT id FROM source WHERE filename = $name LIMIT 1")
        .bind(("name", source_name))
        .await
        .map_err(|e| format!("source lookup: {e}"))?;
    #[derive(Deserialize)]
    struct SourceIdRow {
        id: surrealdb::sql::Thing,
    }
    let src_rows: Vec<SourceIdRow> = src_resp
        .take(0)
        .map_err(|e| format!("source decode: {e}"))?;
    let Some(src_id) = src_rows.into_iter().next() else {
        return Ok(None);
    };

    // Build the chunk query — gate on page only when one was provided.
    let sql = if page.is_some() {
        "SELECT text, page_start, page_end, section_heading FROM chunk \
         WHERE source = $src AND page_start <= $page AND page_end >= $page \
         ORDER BY page_start ASC LIMIT 1"
    } else {
        "SELECT text, page_start, page_end, section_heading FROM chunk \
         WHERE source = $src ORDER BY page_start ASC LIMIT 1"
    };

    let mut chunk_resp = state
        .db
        .query(sql)
        .bind(("src", src_id.id))
        .bind(("page", page))
        .await
        .map_err(|e| format!("chunk lookup: {e}"))?;
    #[derive(Deserialize)]
    struct ChunkRow {
        text: String,
        page_start: i64,
        page_end: i64,
        section_heading: String,
    }
    let chunk_rows: Vec<ChunkRow> = chunk_resp
        .take(0)
        .map_err(|e| format!("chunk decode: {e}"))?;
    Ok(chunk_rows.into_iter().next().map(|r| CitationChunk {
        text: r.text,
        page_start: r.page_start,
        page_end: r.page_end,
        section_heading: r.section_heading,
    }))
}

#[cfg(test)]
mod citation_tests {
    use super::*;
    use serde::Deserialize;

    async fn seed_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("t").use_db("t").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
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
}

#[cfg(test)]
mod reindex_tests {
    use super::*;
    use serde::Deserialize;

    #[tokio::test]
    async fn list_all_source_ids_returns_all_ids() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db.query(
            "CREATE collection SET id='col1', name='Test', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(
            "CREATE source SET id='s1', filename='a.pdf', display_name='a', \
             source_type='rules', page_count=0, indexed_at=time::now(), \
             index_status='done', embed_model='nomic-embed-text-v1.5', \
             collection=type::thing('collection','col1')",
        )
        .await
        .unwrap();
        db.query(
            "CREATE source SET id='s2', filename='b.pdf', display_name='b', \
             source_type='rules', page_count=0, indexed_at=time::now(), \
             index_status='done', embed_model='nomic-embed-text-v1.5', \
             collection=type::thing('collection','col1')",
        )
        .await
        .unwrap();

        let ids = list_all_source_ids(&db).await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"s1".to_string()));
        assert!(ids.contains(&"s2".to_string()));
    }

    /// Regression test for the backtick-wrapped-ID bug. UUIDs contain hyphens,
    /// which trigger SurrealDB's `EscapeRidKey` when `Id::to_string()` is used.
    /// `list_all_source_ids` MUST return raw IDs so they can be passed back
    /// through `type::thing('source', $id)` without producing a mangled record
    /// reference. See commit e099a79 for the prior occurrence in delete_source.
    #[tokio::test]
    async fn list_all_source_ids_does_not_wrap_uuids_in_backticks() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();

        let uuid = "d5a80195-3968-44cb-8b46-270830df952f";
        db.query(
            "CREATE collection SET id='col1', name='Test', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        db.query(format!(
            "CREATE source SET id='{uuid}', filename='a.pdf', display_name='a', \
             source_type='rules', page_count=0, indexed_at=time::now(), \
             index_status='done', embed_model='nomic-embed-text-v1.5', \
             collection=type::thing('collection','col1')"
        ))
        .await
        .unwrap();

        let ids = list_all_source_ids(&db).await.unwrap();
        assert_eq!(ids.len(), 1);
        let id = &ids[0];
        assert!(
            !id.contains('`'),
            "ID must not be wrapped in backticks: got {id:?}"
        );
        assert_eq!(id, uuid);

        // Round-trip check: the returned ID must work with type::thing.
        // If the bug recurs, this query returns no rows.
        let mut resp = db
            .query("SELECT id FROM source WHERE id = type::thing('source', $id)")
            .bind(("id", id.clone()))
            .await
            .unwrap();
        #[derive(Deserialize)]
        struct Found {
            #[allow(dead_code)]
            id: surrealdb::sql::Thing,
        }
        let found: Vec<Found> = resp.take(0).unwrap();
        assert_eq!(
            found.len(),
            1,
            "round-trip lookup with raw ID must find the source"
        );
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use surrealdb::engine::local::Db;
    use surrealdb::Surreal;

    async fn setup_db() -> Surreal<Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db
    }

    // ── get_sources — collection_id filter ───────────────────────────────────

    #[tokio::test]
    async fn get_sources_filters_by_collection() {
        let db = setup_db().await;

        let col_a = crate::services::collection_service::create(&db, "Col A", None)
            .await
            .unwrap();
        let col_b = crate::services::collection_service::create(&db, "Col B", None)
            .await
            .unwrap();

        db.query(
            "CREATE source SET
                 id = 'src_a',
                 filename = 'a.pdf',
                 display_name = 'Source A',
                 source_type = 'rules',
                 page_count = 0,
                 indexed_at = time::now(),
                 index_status = 'pending',
                 embed_model = 'nomic-embed-text-v1.5',
                 campaign = NULL,
                 collection = type::thing('collection', $cid)",
        )
        .bind(("cid", col_a.id.clone()))
        .await
        .unwrap();

        db.query(
            "CREATE source SET
                 id = 'src_b',
                 filename = 'b.pdf',
                 display_name = 'Source B',
                 source_type = 'rules',
                 page_count = 0,
                 indexed_at = time::now(),
                 index_status = 'pending',
                 embed_model = 'nomic-embed-text-v1.5',
                 campaign = NULL,
                 collection = type::thing('collection', $cid)",
        )
        .bind(("cid", col_b.id.clone()))
        .await
        .unwrap();

        let mut resp_a = db
            .query(
                "SELECT * FROM source \
                 WHERE collection = type::thing('collection', $cid) \
                 ORDER BY display_name ASC",
            )
            .bind(("cid", col_a.id.clone()))
            .await
            .unwrap();

        #[derive(Deserialize)]
        struct Row {
            id: surrealdb::sql::Thing,
        }
        let rows_a: Vec<Row> = resp_a.take(0).unwrap();
        assert_eq!(rows_a.len(), 1);
        assert_eq!(rows_a[0].id.id.to_raw(), "src_a");

        let mut resp_all = db
            .query("SELECT * FROM source ORDER BY display_name ASC")
            .await
            .unwrap();
        let rows_all: Vec<Row> = resp_all.take(0).unwrap();
        assert_eq!(rows_all.len(), 2);
    }
}
