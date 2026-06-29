use surrealdb::Connection;

use super::super::AgentError;

/// Resolve the collection IDs that a campaign is subscribed to.
///
/// Queries the `subscribes_to` relation for the given `campaign_id` and
/// returns the bare IDs (no `table:` prefix) of all subscribed collections.
/// Returns an empty `Vec` when the campaign has no subscriptions.
pub async fn resolve_collection_ids<C>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Vec<String>, AgentError>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT out FROM subscribes_to WHERE in = type::thing('campaign', $id)")
        .bind(("id", campaign_id.to_owned()))
        .await
        .map_err(|e| AgentError::Db(e.to_string()))?;

    #[derive(serde::Deserialize)]
    struct Row {
        out: surrealdb::sql::Thing,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| AgentError::Db(e.to_string()))?;

    Ok(rows.into_iter().map(|r| r.out.id.to_raw()).collect())
}
