use serde::{Deserialize, Serialize};
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
