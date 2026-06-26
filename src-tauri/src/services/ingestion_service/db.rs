use surrealdb::Connection;

use super::types::{IngestionError, SourceInfo};

/// Fetch the filename and collection ID for a source record.
///
/// Exposed as `pub(crate)` so tests can call it directly without going
/// through the full `ingest_source` pipeline.
pub(crate) async fn get_source_info<C>(
    db: &surrealdb::Surreal<C>,
    source_id: &str,
) -> Result<SourceInfo, IngestionError>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT filename, collection FROM source WHERE id = type::thing('source', $id)")
        .bind(("id", source_id.to_owned()))
        .await
        .map_err(|e| IngestionError::Db(format!("Failed to query source: {e}")))?;

    #[derive(serde::Deserialize)]
    struct Row {
        filename: String,
        /// Non-optional: matches `source.collection TYPE record<collection>` schema.
        collection: surrealdb::sql::Thing,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| IngestionError::Db(e.to_string()))?;

    rows.into_iter()
        .next()
        .map(|r| SourceInfo {
            filename: r.filename,
            collection_id: r.collection.id.to_raw(),
        })
        .ok_or_else(|| IngestionError::Db(format!("Source '{source_id}' not found")))
}

/// Mark a source as `failed` and delete any chunks already written for it.
///
/// Called from the error path of `ingest_source` so a retry starts from a
/// clean slate. If the source row was already deleted by the caller (e.g.
/// `delete_source` racing with a failed ingest), the UPDATE is a no-op.
pub(super) async fn mark_failed_and_cleanup<C>(
    db: &surrealdb::Surreal<C>,
    source_id: &str,
) -> Result<(), IngestionError>
where
    C: Connection,
{
    db.query(
        "UPDATE source SET index_status = 'error' WHERE id = type::thing('source', $id); \
         DELETE chunk WHERE source = type::thing('source', $id)",
    )
    .bind(("id", source_id.to_owned()))
    .await
    .map_err(|e| IngestionError::Db(format!("cleanup query failed: {e}")))?
    .check()
    .map_err(|e| IngestionError::Db(format!("cleanup statement failed: {e}")))?;
    Ok(())
}
