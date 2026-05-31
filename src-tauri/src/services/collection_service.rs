/// Collection service — CRUD operations and campaign subscription management.
///
/// A `collection` groups related source PDFs (e.g. "D&D 5e Core Rules").
/// Campaigns subscribe to collections via the `subscribes_to` relation so that
/// multiple campaigns can share the same rulebook set without duplication.
use serde::{Deserialize, Serialize};
use surrealdb::Connection;
use surrealdb::sql::Thing;

/// Raw record returned from SurrealDB for the `collection` table.
#[derive(Debug, Clone, Deserialize)]
pub struct CollectionRecord {
    pub id: Thing,
    pub name: String,
    pub description: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Public-facing collection DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<CollectionRecord> for Collection {
    fn from(r: CollectionRecord) -> Self {
        Self {
            id: r.id.id.to_raw(),
            name: r.name,
            description: r.description,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use surrealdb::engine::local::Db;
    use surrealdb::Surreal;

    async fn setup() -> Surreal<Db> {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        crate::schema::run_migrations(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn create_and_get_all() {
        let db = setup().await;
        let c = create(&db, "D&D 5e Core", Some("Core rulebooks"))
            .await
            .unwrap();
        assert_eq!(c.name, "D&D 5e Core");
        assert_eq!(c.description.as_deref(), Some("Core rulebooks"));
        let all = get_all(&db).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, c.id);
    }

    #[tokio::test]
    async fn get_by_id_returns_collection() {
        let db = setup().await;
        let c = create(&db, "Pathfinder 2e", None).await.unwrap();
        let found = get_by_id(&db, &c.id).await.unwrap();
        assert_eq!(found.name, "Pathfinder 2e");
    }

    #[tokio::test]
    async fn update_changes_name_and_description() {
        let db = setup().await;
        let c = create(&db, "Old Name", None).await.unwrap();
        let updated = update(&db, &c.id, "New Name", Some("desc")).await.unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.description.as_deref(), Some("desc"));
    }

    #[tokio::test]
    async fn delete_removes_collection() {
        let db = setup().await;
        let c = create(&db, "Temp", None).await.unwrap();
        delete(&db, &c.id).await.unwrap();
        let all = get_all(&db).await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn delete_blocked_when_source_exists() {
        let db = setup().await;
        let c = create(&db, "Protected", None).await.unwrap();
        // Insert a source that references this collection.
        // `campaign` is TYPE record<campaign> | NULL (no DEFAULT), so SCHEMAFULL
        // validation requires it to be set explicitly to NULL rather than omitted.
        db.query(
            "CREATE source SET id='s1', campaign=NULL, \
             collection=type::thing('collection', $cid), \
             filename='a.pdf', display_name='A', source_type='rules', page_count=0, \
             indexed_at=time::now(), index_status='done', embed_model='nomic-embed-text-v1.5'",
        )
        .bind(("cid", c.id.clone()))
        .await
        .unwrap();
        let result = delete(&db, &c.id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sources exist"));
    }

    #[tokio::test]
    async fn delete_blocked_when_campaign_subscribed() {
        let db = setup().await;
        let c = create(&db, "Protected", None).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test Campaign', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        add_campaign_collection(&db, "camp1", &c.id).await.unwrap();
        let result = delete(&db, &c.id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("campaigns are subscribed"));
    }

    #[tokio::test]
    async fn add_and_remove_campaign_collection() {
        let db = setup().await;
        let c = create(&db, "D&D 5e Core", None).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='My Campaign', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();

        add_campaign_collection(&db, "camp1", &c.id).await.unwrap();
        let cols = get_campaign_collections(&db, "camp1").await.unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].id, c.id);

        remove_campaign_collection(&db, "camp1", &c.id)
            .await
            .unwrap();
        let cols = get_campaign_collections(&db, "camp1").await.unwrap();
        assert!(cols.is_empty());
    }

    #[tokio::test]
    async fn get_campaign_collections_returns_only_subscribed() {
        let db = setup().await;
        let c1 = create(&db, "D&D 5e Core", None).await.unwrap();
        let _c2 = create(&db, "Pathfinder 2e", None).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='My Campaign', system='D&D 5e', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        add_campaign_collection(&db, "camp1", &c1.id).await.unwrap();

        let cols = get_campaign_collections(&db, "camp1").await.unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].name, "D&D 5e Core");
    }

    #[tokio::test]
    async fn add_campaign_collection_is_idempotent() {
        let db = setup().await;
        let c = create(&db, "D&D 5e Core", None).await.unwrap();
        db.query(
            "CREATE campaign SET id='camp1', name='Test', system='D&D', \
             created_at=time::now(), updated_at=time::now()",
        )
        .await
        .unwrap();
        add_campaign_collection(&db, "camp1", &c.id).await.unwrap();
        add_campaign_collection(&db, "camp1", &c.id).await.unwrap(); // second call must succeed
        let cols = get_campaign_collections(&db, "camp1").await.unwrap();
        assert_eq!(cols.len(), 1); // must not return duplicates
    }

    #[tokio::test]
    async fn get_by_id_missing_returns_err() {
        let db = setup().await;
        let result = get_by_id(&db, "does-not-exist").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn get_all_returns_ordered_by_name() {
        let db = setup().await;
        create(&db, "Zzz Collection", None).await.unwrap();
        create(&db, "Aaa Collection", None).await.unwrap();
        let all = get_all(&db).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "Aaa Collection");
        assert_eq!(all[1].name, "Zzz Collection");
    }
}
