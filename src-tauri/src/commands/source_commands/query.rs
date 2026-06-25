use std::sync::Arc;

use serde::Deserialize;
use tauri::State;

use crate::AppState;

use super::SourceResponse;

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
pub async fn delete_source(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
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

/// Enumerate all source IDs in the database.
pub(super) async fn list_all_source_ids<C: surrealdb::Connection>(
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
