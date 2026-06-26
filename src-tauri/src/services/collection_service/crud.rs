use serde::Deserialize;
use surrealdb::Connection;

use super::types::{Collection, CollectionRecord};

/// Create a new collection with the given `name` and optional `description`.
pub async fn create<C>(
    db: &surrealdb::Surreal<C>,
    name: &str,
    description: Option<&str>,
) -> Result<Collection, String>
where
    C: Connection,
{
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let mut response = db
        .query(
            "CREATE collection SET
             id = $id,
             name = $name,
             description = $description,
             created_at = time::now(),
             updated_at = time::now()",
        )
        .bind(("id", id))
        .bind(("name", name.to_owned()))
        .bind(("description", description.map(str::to_owned)))
        .await
        .map_err(|e| format!("Failed to create collection: {e}"))?;
    let records: Vec<CollectionRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created collection: {e}"))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Failed to create collection: no record returned".to_string())
}

/// Return all collections ordered by name.
pub async fn get_all<C>(db: &surrealdb::Surreal<C>) -> Result<Vec<Collection>, String>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT * FROM collection ORDER BY name ASC")
        .await
        .map_err(|e| format!("Failed to query collections: {e}"))?;
    let records: Vec<CollectionRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse collections: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Return a single collection by its raw ID string.
pub async fn get_by_id<C>(db: &surrealdb::Surreal<C>, id: &str) -> Result<Collection, String>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT * FROM type::thing('collection', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to query collection: {e}"))?;
    let records: Vec<CollectionRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse collection: {e}"))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| format!("Collection '{id}' not found"))
}

/// Update the `name` and `description` of an existing collection.
pub async fn update<C>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<Collection, String>
where
    C: Connection,
{
    let mut response = db
        .query(
            "UPDATE type::thing('collection', $id) SET
             name = $name,
             description = $description,
             updated_at = time::now()",
        )
        .bind(("id", id.to_owned()))
        .bind(("name", name.to_owned()))
        .bind(("description", description.map(str::to_owned)))
        .await
        .map_err(|e| format!("Failed to update collection: {e}"))?;
    let records: Vec<CollectionRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse updated collection: {e}"))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| format!("Collection '{id}' not found after update"))
}

/// Delete a collection.
///
/// Returns an error (without touching the DB) if:
/// - any campaign is subscribed to it via `subscribes_to`, or
/// - any source record still references it via `source.collection`.
pub async fn delete<C>(db: &surrealdb::Surreal<C>, id: &str) -> Result<(), String>
where
    C: Connection,
{
    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }

    // Guard: reject if any campaign is subscribed
    let mut sub_resp = db
        .query(
            "SELECT count() FROM subscribes_to \
             WHERE out = type::thing('collection', $id) GROUP ALL",
        )
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to check subscriptions: {e}"))?;
    let sub_counts: Vec<CountRow> = sub_resp
        .take(0)
        .map_err(|e| format!("Failed to parse subscription count: {e}"))?;
    if sub_counts.first().map(|c| c.count).unwrap_or(0) > 0 {
        return Err(
            "Cannot delete: campaigns are subscribed to this collection. \
             Remove subscriptions first."
                .to_string(),
        );
    }

    // Guard: reject if any source is in this collection
    let mut src_resp = db
        .query(
            "SELECT count() FROM source \
             WHERE collection = type::thing('collection', $id) GROUP ALL",
        )
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to check sources: {e}"))?;
    let src_counts: Vec<CountRow> = src_resp
        .take(0)
        .map_err(|e| format!("Failed to parse source count: {e}"))?;
    if src_counts.first().map(|c| c.count).unwrap_or(0) > 0 {
        return Err(
            "Cannot delete: sources exist in this collection. Delete sources first.".to_string(),
        );
    }

    db.query("DELETE type::thing('collection', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to delete collection: {e}"))?;
    Ok(())
}
