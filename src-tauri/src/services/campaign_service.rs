use serde::{Deserialize, Serialize};
use surrealdb::engine::local::Db;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

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
            id: r.id.id.to_string(),
            name: r.name,
            system: r.system,
        }
    }
}

/// Get all campaigns, ordered by name.
pub async fn get_all(db: &Surreal<Db>) -> Result<Vec<Campaign>, String> {
    let mut response = db
        .query("SELECT * FROM campaign ORDER BY name ASC")
        .await
        .map_err(|e| format!("Failed to query campaigns: {e}"))?;
    let records: Vec<CampaignRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse campaigns: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Create a new campaign.
pub async fn create(db: &Surreal<Db>, name: &str, system: &str) -> Result<Campaign, String> {
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let mut response = db
        .query(
            "CREATE campaign SET
            id = $id,
            name = $name,
            system = $system,
            created_at = time::now(),
            updated_at = time::now()",
        )
        .bind(("id", id.clone()))
        .bind(("name", name.to_owned()))
        .bind(("system", system.to_owned()))
        .await
        .map_err(|e| format!("Failed to create campaign: {e}"))?;
    let created: Vec<CampaignRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created campaign: {e}"))?;
    created
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Failed to create campaign: no record returned".to_string())
}

/// Get a single campaign by id.
pub async fn get_by_id(db: &Surreal<Db>, id: &str) -> Result<Campaign, String> {
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
pub async fn update(
    db: &Surreal<Db>,
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

/// Delete a campaign by id.
pub async fn delete(db: &Surreal<Db>, id: &str) -> Result<(), String> {
    db.query("DELETE type::thing('campaign', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to delete campaign: {e}"))?;
    Ok(())
}
