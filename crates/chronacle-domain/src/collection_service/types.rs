use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

/// Raw record returned from SurrealDB for the `collection` table.
#[derive(Debug, Clone, Deserialize)]
pub struct CollectionRecord {
    pub id: Thing,
    pub name: String,
    pub description: Option<String>,
    /// Optional back-reference to the campaign that owns this collection.
    ///
    /// * `None` — regular (shareable) collection. The default for every
    ///   collection created before the LLM Wiki layer, and for any collection
    ///   the user creates explicitly outside a campaign context.
    /// * `Some(campaign_id)` — campaign-bound: created by
    ///   `campaign_service::create` and permanently associated with that
    ///   campaign until the campaign is deleted (or converted to regular
    ///   via `OnOwnedCollection::ConvertToRegular`).
    #[serde(default)]
    pub owner_campaign: Option<Thing>,
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
