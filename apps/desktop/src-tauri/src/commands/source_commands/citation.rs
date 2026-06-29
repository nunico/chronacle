use std::sync::Arc;

use serde::Deserialize;
use tauri::State;

use crate::AppState;

use super::CitationChunk;

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
