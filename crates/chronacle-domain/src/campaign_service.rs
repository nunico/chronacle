use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use surrealdb::Connection;
use surrealdb::Surreal;

use crate::collection_service;

/// What to do with a campaign's auto-created owned collection when the campaign
/// itself is deleted.
///
/// A campaign that was created via [`create`] always has exactly one owned
/// collection (see the LLM Wiki layer design notes in
/// `docs/superpowers/specs/2026-07-02-compiled-world-model-a1-design.md`).
/// Deleting the campaign forces a decision:
///
/// * [`OnOwnedCollection::Delete`] — cascade: the owned collection is torn
///   down along with every source, chunk, entity, and `relates_to` edge that
///   lives inside it. Source blob files on disk are *not* touched (the caller
///   is responsible for that if they want it).
/// * [`OnOwnedCollection::ConvertToRegular`] — the owned collection is kept
///   but demoted to a regular (shareable) collection: its `owner_campaign`
///   field is cleared, and any `relates_to` edges whose *both* endpoints are
///   inside the collection are dropped and logged as `lint_finding` rows
///   with `kind = "orphaned_edge"`.
///
/// Legacy campaigns that pre-date the owned-collection auto-create still
/// exist in some databases; for them both variants degrade to "just delete
/// the campaign row" (there is nothing else to touch).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnOwnedCollection {
    /// Cascade-delete the owned collection and everything inside it.
    Delete,
    /// Keep the collection; demote it to a regular one and orphan intra edges.
    ConvertToRegular,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignRecord {
    pub id: Thing,
    pub name: String,
    pub system: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Campaign {
    pub id: String,
    pub name: String,
    pub system: String,
}

impl From<CampaignRecord> for Campaign {
    fn from(r: CampaignRecord) -> Self {
        Self {
            id: r.id.id.to_raw(),
            name: r.name,
            system: r.system,
        }
    }
}

/// Get all campaigns, ordered by name.
pub async fn get_all<C: Connection>(db: &Surreal<C>) -> Result<Vec<Campaign>, String> {
    let mut response = db
        .query("SELECT * FROM campaign ORDER BY name ASC")
        .await
        .map_err(|e| format!("Failed to query campaigns: {e}"))?;
    let records: Vec<CampaignRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse campaigns: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Create a new campaign, along with an auto-owned collection subscribed to
/// the campaign.
///
/// The owned collection:
/// * has the same `name` as the campaign,
/// * has `owner_campaign` set to the new campaign's id,
/// * is subscribed via a `subscribes_to` edge from the campaign.
///
/// Consumers that need the owned collection's id can look it up via
/// [`collection_service::owned_by`].
pub async fn create<C: Connection>(
    db: &Surreal<C>,
    name: &str,
    system: &str,
) -> Result<Campaign, String> {
    let campaign_id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let collection_id = uuid::Uuid::new_v4().to_string().replace('-', "");

    // Run campaign + collection + subscribes_to as a single batched query.
    // SurrealDB embedded does not give us transactional guarantees across
    // separate `.query()` calls; batching keeps the operation self-contained.
    // If any statement fails, none of the following ones execute.
    let mut response = db
        .query(
            "CREATE campaign SET
                id = $campaign_id,
                name = $name,
                system = $system,
                created_at = time::now(),
                updated_at = time::now();
             CREATE collection SET
                id = $collection_id,
                name = $name,
                description = NULL,
                owner_campaign = type::thing('campaign', $campaign_id),
                created_at = time::now(),
                updated_at = time::now();
             RELATE type::thing('campaign', $campaign_id)
                    ->subscribes_to
                    ->type::thing('collection', $collection_id)
                    SET created_at = time::now();",
        )
        .bind(("campaign_id", campaign_id.clone()))
        .bind(("collection_id", collection_id))
        .bind(("name", name.to_owned()))
        .bind(("system", system.to_owned()))
        .await
        .map_err(|e| format!("Failed to create campaign: {e}"))?;

    // First statement (the campaign itself) is the one we return to the caller.
    let created: Vec<CampaignRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created campaign: {e}"))?;

    // Surface later-statement errors too — a failed collection CREATE would
    // otherwise silently leave a dangling campaign. `.check()` promotes
    // per-statement errors into the returned Result.
    response
        .check()
        .map_err(|e| format!("Failed to create owned collection or subscription: {e}"))?;

    created
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Failed to create campaign: no record returned".to_string())
}

/// Get a single campaign by id.
pub async fn get_by_id<C: Connection>(db: &Surreal<C>, id: &str) -> Result<Campaign, String> {
    let mut response = db
        .query("SELECT * FROM type::thing('campaign', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to query campaign: {e}"))?;
    let records: Vec<CampaignRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse campaign: {e}"))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| format!("Campaign '{id}' not found"))
}

/// Update a campaign's name and/or system.
pub async fn update<C: Connection>(
    db: &Surreal<C>,
    id: &str,
    name: &str,
    system: &str,
) -> Result<Campaign, String> {
    let mut response = db
        .query(
            "UPDATE type::thing('campaign', $id) SET
                name = $name,
                system = $system,
                updated_at = time::now()",
        )
        .bind(("id", id.to_owned()))
        .bind(("name", name.to_owned()))
        .bind(("system", system.to_owned()))
        .await
        .map_err(|e| format!("Failed to update campaign: {e}"))?;
    let records: Vec<CampaignRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse updated campaign: {e}"))?;
    records
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| format!("Campaign '{id}' not found after update"))
}

/// Delete a campaign, choosing what happens to its owned collection.
///
/// See [`OnOwnedCollection`] for semantics of each mode. Both modes also
/// drop the `subscribes_to` edge from the campaign to its owned collection
/// (if any), and delete the campaign record itself.
///
/// Regular collections the campaign is subscribed to are *never* affected —
/// their `subscribes_to` edges get cleaned up but the collections stay.
pub async fn delete<C: Connection>(
    db: &Surreal<C>,
    id: &str,
    on_owned_collection: OnOwnedCollection,
) -> Result<(), String> {
    let owned = collection_service::owned_by(db, id).await?;

    if let Some(owned) = owned {
        match on_owned_collection {
            OnOwnedCollection::Delete => {
                collection_service::hard_delete_with_content(db, &owned.id).await?;
            }
            OnOwnedCollection::ConvertToRegular => {
                orphan_intra_edges_and_log(db, id, &owned.id).await?;
                db.query(
                    "UPDATE type::thing('collection', $cid) SET
                        owner_campaign = NONE,
                        updated_at = time::now()",
                )
                .bind(("cid", owned.id.clone()))
                .await
                .map_err(|e| format!("Failed to demote owned collection: {e}"))?
                .check()
                .map_err(|e| format!("Failed to demote owned collection: {e}"))?;
            }
        }
    }

    // Drop all subscribes_to edges from this campaign.
    // - In cascade mode the owned collection is already gone, so this only
    //   sweeps subscriptions to regular collections.
    // - In convert mode this also drops the demoted collection's own
    //   subscription (the campaign is going away).
    db.query(
        "DELETE subscribes_to
         WHERE in = type::thing('campaign', $id)",
    )
    .bind(("id", id.to_owned()))
    .await
    .map_err(|e| format!("Failed to clean up subscriptions: {e}"))?
    .check()
    .map_err(|e| format!("Failed to clean up subscriptions: {e}"))?;

    db.query("DELETE type::thing('campaign', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to delete campaign: {e}"))?
        .check()
        .map_err(|e| format!("Failed to delete campaign: {e}"))?;

    Ok(())
}

/// For every `relates_to` edge whose *both* endpoints are entities inside the
/// given collection, record a `lint_finding` (kind = `orphaned_edge`) and
/// delete the edge. Edges that have only one endpoint inside are preserved —
/// they now legitimately cross into what has become a regular collection.
///
/// Implementation: two `INSERT INTO lint_finding (SELECT ...)` + `DELETE` set
/// operations against a snapshotted `$entities` variable. This avoids the
/// SurrealDB `FOR` construct entirely (not exercised elsewhere in the
/// codebase) at the cost of one extra scan of `relates_to`.
async fn orphan_intra_edges_and_log<C: Connection>(
    db: &Surreal<C>,
    campaign_id: &str,
    collection_id: &str,
) -> Result<(), String> {
    db.query(
        "LET $col = type::thing('collection', $cid);
         LET $cam = type::thing('campaign',   $campaign_id);
         LET $entities = (SELECT VALUE out FROM in_collection WHERE in = $col);
         INSERT INTO lint_finding (SELECT
            'orphaned_edge' AS kind,
            {
                campaign_id:   $cam,
                collection_id: $col,
                edge_id:       id,
                from:          in,
                to:            out,
                rel_type:      rel_type
            } AS payload
         FROM relates_to
         WHERE in IN $entities AND out IN $entities);
         DELETE relates_to
         WHERE in IN $entities AND out IN $entities;",
    )
    .bind(("cid", collection_id.to_owned()))
    .bind(("campaign_id", campaign_id.to_owned()))
    .await
    .map_err(|e| format!("Failed to log/drop orphan edges: {e}"))?
    .check()
    .map_err(|e| format!("Failed to log/drop orphan edges: {e}"))?;
    Ok(())
}
