use super::{validate_identifier, validate_record_id, split_record_id, WikilinkError};

/// Upsert `relates_to` edges from `source_table:source_id` to each of
/// `matched_ids` with `rel_type = "mentioned"`.
///
/// SurrealDB `RELATE` does not deduplicate — it always creates a new edge.
/// Each target edge is explicitly deleted before being re-created.
pub(super) async fn upsert_mentioned_edges<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    source_table: &str,
    source_id: &str,
    matched_ids: &[String],
) -> Result<(), WikilinkError> {
    for target_full_id in matched_ids {
        let (to_table, to_id) = split_record_id(target_full_id)?;
        validate_identifier(to_table)?;
        validate_identifier(to_id)?;

        let delete_query = format!(
            "DELETE relates_to \
             WHERE in = {source_table}:{source_id} \
             AND out = {to_table}:{to_id} \
             AND rel_type = 'mentioned'"
        );
        db.query(delete_query)
            .await
            .map_err(|e| WikilinkError::Database { message: e.to_string() })?;

        if has_higher_tier_edge(db, source_table, source_id, to_table, to_id).await? {
            continue;
        }

        let relate_query = format!(
            "RELATE {source_table}:{source_id}->relates_to->{to_table}:{to_id} \
             SET rel_type = 'mentioned', notes = NULL, created_at = time::now()"
        );
        db.query(relate_query)
            .await
            .map_err(|e| WikilinkError::Database { message: e.to_string() })?;
    }
    Ok(())
}

/// Returns `true` when a non-`mentioned` `relates_to` edge already connects
/// this pair of entities in either direction.
///
/// Callers must validate all four identifier components before calling.
pub(super) async fn has_higher_tier_edge<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    a_table: &str,
    a_id: &str,
    b_table: &str,
    b_id: &str,
) -> Result<bool, WikilinkError> {
    let query = format!(
        "SELECT VALUE id FROM relates_to WHERE \
         ((in = {a_table}:{a_id} AND out = {b_table}:{b_id}) OR \
          (in = {b_table}:{b_id} AND out = {a_table}:{a_id})) \
         AND rel_type != 'mentioned' LIMIT 1"
    );
    let mut resp = db.query(query).await.map_err(|e| WikilinkError::Database {
        message: e.to_string(),
    })?;
    let ids: Vec<surrealdb::sql::Thing> = resp.take(0).map_err(|e| WikilinkError::Database {
        message: e.to_string(),
    })?;
    Ok(!ids.is_empty())
}

/// Delete stale `relates_to` edges where the source is this record,
/// `rel_type = "mentioned"`, and the target is NOT in `keep_ids`.
pub(super) async fn delete_stale_mentioned_edges<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    source_table: &str,
    source_id: &str,
    keep_ids: &[String],
) -> Result<(), WikilinkError> {
    if keep_ids.is_empty() {
        let query = format!(
            "DELETE relates_to \
             WHERE in = {source_table}:{source_id} \
             AND rel_type = 'mentioned'"
        );
        db.query(query).await.map_err(|e| WikilinkError::Database {
            message: e.to_string(),
        })?;
    } else {
        for id in keep_ids {
            validate_record_id(id)?;
        }
        let keep_list = keep_ids.join(", ");
        let query = format!(
            "DELETE relates_to \
             WHERE in = {source_table}:{source_id} \
             AND rel_type = 'mentioned' \
             AND out NOT IN [{keep_list}]"
        );
        db.query(query).await.map_err(|e| WikilinkError::Database {
            message: e.to_string(),
        })?;
    }
    Ok(())
}
