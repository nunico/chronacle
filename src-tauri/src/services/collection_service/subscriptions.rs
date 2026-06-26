use serde::Deserialize;
use surrealdb::Connection;

use super::types::Collection;
use super::types::CollectionRecord;

/// Subscribe a campaign to a collection via the `subscribes_to` relation.
///
/// This operation is idempotent: calling it when the relation already exists
/// returns `Ok(())` without creating a duplicate row.
pub async fn add_campaign_collection<C>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    collection_id: &str,
) -> Result<(), String>
where
    C: Connection,
{
    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }

    // Check whether the subscription already exists before inserting.
    // The unique index on subscribes_to(in, out) prevents duplicates at the
    // DB level, but we pre-check here so we can return Ok(()) silently
    // instead of surfacing a constraint error.
    let mut check = db
        .query(
            "SELECT count() FROM subscribes_to \
             WHERE in = type::thing('campaign', $cid) \
             AND out = type::thing('collection', $colid) \
             GROUP ALL",
        )
        .bind(("cid", campaign_id.to_owned()))
        .bind(("colid", collection_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to check subscription existence: {e}"))?;
    let counts: Vec<CountRow> = check
        .take(0)
        .map_err(|e| format!("Failed to parse subscription check: {e}"))?;
    if counts.first().map(|r| r.count).unwrap_or(0) > 0 {
        return Ok(());
    }

    // SurrealDB 2.x does not allow function calls in the RELATE subject/object
    // positions directly, so we bind the record IDs into variables first.
    db.query(
        "LET $in  = type::thing('campaign',   $campaign_id); \
         LET $out = type::thing('collection', $collection_id); \
         RELATE $in->subscribes_to->$out SET created_at = time::now()",
    )
    .bind(("campaign_id", campaign_id.to_owned()))
    .bind(("collection_id", collection_id.to_owned()))
    .await
    .map_err(|e| format!("Failed to subscribe campaign to collection: {e}"))?;
    Ok(())
}

/// Remove the `subscribes_to` relation between a campaign and a collection.
pub async fn remove_campaign_collection<C>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
    collection_id: &str,
) -> Result<(), String>
where
    C: Connection,
{
    db.query(
        "DELETE FROM subscribes_to \
         WHERE in = type::thing('campaign', $campaign_id) \
         AND out = type::thing('collection', $collection_id)",
    )
    .bind(("campaign_id", campaign_id.to_owned()))
    .bind(("collection_id", collection_id.to_owned()))
    .await
    .map_err(|e| format!("Failed to unsubscribe campaign from collection: {e}"))?;
    Ok(())
}

/// Return all collections subscribed to by the given campaign.
pub async fn get_campaign_collections<C>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<Collection>, String>
where
    C: Connection,
{
    let mut response = db
        .query(
            "SELECT * FROM collection WHERE id IN \
             (SELECT VALUE out FROM subscribes_to \
              WHERE in = type::thing('campaign', $id))",
        )
        .bind(("id", campaign_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to query campaign collections: {e}"))?;
    let records: Vec<CollectionRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse campaign collections: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}
