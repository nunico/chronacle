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

/// Look up the collection that a given campaign owns, if any.
///
/// Returns `Ok(None)` when the campaign has no owned collection — this
/// includes both legacy campaigns created before the LLM Wiki layer and
/// campaigns whose owned collection has been demoted via
/// `OnOwnedCollection::ConvertToRegular`.
pub async fn owned_by<C>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<Option<Collection>, String>
where
    C: Connection,
{
    let mut response = db
        .query(
            "SELECT * FROM collection
             WHERE owner_campaign = type::thing('campaign', $cid)",
        )
        .bind(("cid", campaign_id.to_owned()))
        .await
        .map_err(|e| format!("Failed to query owned collection: {e}"))?;
    let records: Vec<CollectionRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse owned collection: {e}"))?;
    Ok(records.into_iter().next().map(Into::into))
}

/// Cascade-delete a collection and all of its DB-side content.
///
/// Removes, in order:
/// * every `chunk` whose `source` is in this collection,
/// * every `source` in this collection,
/// * every `in_collection` edge originating from this collection
///   (which enumerates the entities to remove next),
/// * every `relates_to` edge that touches one of those entities
///   (either endpoint),
/// * every entity row those edges pointed at,
/// * the collection row itself.
///
/// **Source blob files on disk are not touched.** Callers that also want to
/// unlink files should invoke the source-command layer separately; the
/// service layer stays free of `AppState`/`blob_store` coupling.
///
/// Unlike [`delete`], this function does *not* check that the collection is
/// empty. It is intended for the campaign-cascade path in
/// `campaign_service::delete`; general "delete this collection" flows should
/// keep using [`delete`], which fails safely on non-empty state.
pub async fn hard_delete_with_content<C>(db: &surrealdb::Surreal<C>, id: &str) -> Result<(), String>
where
    C: Connection,
{
    // Single batched query so a partial failure surfaces via `.check()`.
    // Order matters: we snapshot entity ids into $entities before mutating
    // in_collection, because DELETE mid-query would clear that snapshot.
    //
    // Entities are deleted per-table (all 8 node tables enumerated) rather
    // than via `FOR $e IN $entities { DELETE $e }`. `DELETE <record>` in a
    // FOR loop is not exercised anywhere else in the codebase, and per-table
    // DELETE-WHERE uses the standard, well-covered SurrealQL path.
    db.query(
        "LET $col = type::thing('collection', $id);
         LET $sources  = (SELECT VALUE id FROM source WHERE collection = $col);
         LET $entities = (SELECT VALUE out FROM in_collection WHERE in = $col);
         DELETE chunk         WHERE source IN $sources;
         DELETE source        WHERE id IN $sources;
         DELETE relates_to    WHERE in IN $entities OR out IN $entities;
         DELETE in_collection WHERE in = $col;
         DELETE in_campaign   WHERE out IN $entities;
         DELETE npc              WHERE id IN $entities;
         DELETE location         WHERE id IN $entities;
         DELETE faction          WHERE id IN $entities;
         DELETE creature         WHERE id IN $entities;
         DELETE item             WHERE id IN $entities;
         DELETE event            WHERE id IN $entities;
         DELETE player_character WHERE id IN $entities;
         DELETE misc             WHERE id IN $entities;
         DELETE $col;",
    )
    .bind(("id", id.to_owned()))
    .await
    .map_err(|e| format!("Failed to cascade-delete collection '{id}': {e}"))?
    .check()
    .map_err(|e| format!("Failed to cascade-delete collection '{id}': {e}"))?;
    Ok(())
}
