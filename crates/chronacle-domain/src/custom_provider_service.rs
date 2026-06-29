use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;
use surrealdb::Connection;
use surrealdb::Surreal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderRecord {
    pub id: Thing,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
}

impl From<CustomProviderRecord> for CustomProvider {
    fn from(r: CustomProviderRecord) -> Self {
        Self {
            id: r.id.id.to_raw(),
            name: r.name,
            provider_type: r.provider_type,
            base_url: r.base_url,
            api_key: r.api_key,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderModelRecord {
    pub id: Thing,
    pub provider: Thing,
    pub model_id: String,
    pub display_name: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomProviderModel {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
}

impl From<CustomProviderModelRecord> for CustomProviderModel {
    fn from(r: CustomProviderModelRecord) -> Self {
        Self {
            id: r.id.id.to_raw(),
            provider_id: r.provider.id.to_raw(),
            model_id: r.model_id,
            display_name: r.display_name,
        }
    }
}

/// Get a single custom provider by its record id.
pub async fn get_by_id<C: Connection>(db: &Surreal<C>, id: &str) -> Result<CustomProvider, String> {
    let mut response = db
        .query("SELECT * FROM type::thing('custom_provider', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to query custom provider: {e}"))?;
    let record: Option<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse custom provider: {e}"))?;
    record
        .map(Into::into)
        .ok_or_else(|| "Custom provider not found".to_string())
}

/// Get all custom providers, ordered by name.
pub async fn get_all<C: Connection>(db: &Surreal<C>) -> Result<Vec<CustomProvider>, String> {
    let mut response = db
        .query("SELECT * FROM custom_provider ORDER BY name ASC")
        .await
        .map_err(|e| format!("Failed to query custom providers: {e}"))?;
    let records: Vec<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse custom providers: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Create a new custom provider with a UUID.
pub async fn create<C: Connection>(
    db: &Surreal<C>,
    name: &str,
    provider_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<CustomProvider, String> {
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let mut response = db
        .query(
            "CREATE custom_provider SET
            id = $id,
            name = $name,
            provider_type = $provider_type,
            base_url = $base_url,
            api_key = $api_key,
            updated_at = time::now()",
        )
        .bind(("id", id.clone()))
        .bind(("name", name.to_owned()))
        .bind(("provider_type", provider_type.to_owned()))
        .bind(("base_url", base_url.to_owned()))
        .bind(("api_key", api_key.to_owned()))
        .await
        .map_err(|e| format!("Failed to create custom provider: {e}"))?;
    let created: Vec<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created provider: {e}"))?;
    created
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Failed to create custom provider: no record returned".to_string())
}

/// Update an existing custom provider. Empty string fields are left unchanged.
pub async fn update<C: Connection>(
    db: &Surreal<C>,
    id: &str,
    name: &str,
    provider_type: &str,
    base_url: &str,
    api_key: &str,
) -> Result<CustomProvider, String> {
    // Build set clauses and bind params only for non-empty fields
    let mut set_parts: Vec<&str> = Vec::new();
    let mut binds: Vec<(&str, String)> = Vec::new();

    if !name.is_empty() {
        set_parts.push("name = $name");
        binds.push(("name", name.to_owned()));
    }
    if !provider_type.is_empty() {
        set_parts.push("provider_type = $provider_type");
        binds.push(("provider_type", provider_type.to_owned()));
    }
    if !base_url.is_empty() {
        set_parts.push("base_url = $base_url");
        binds.push(("base_url", base_url.to_owned()));
    }
    if !api_key.is_empty() {
        set_parts.push("api_key = $api_key");
        binds.push(("api_key", api_key.to_owned()));
    }
    set_parts.push("updated_at = time::now()");

    let sql = format!(
        "UPDATE type::thing('custom_provider', $id) SET {}",
        set_parts.join(", ")
    );

    let mut q = db.query(sql).bind(("id", id.to_owned()));
    for (k, v) in binds {
        q = q.bind((k, v));
    }

    let mut response = q
        .await
        .map_err(|e| format!("Failed to update custom provider: {e}"))?;
    let updated: Vec<CustomProviderRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse updated provider: {e}"))?;
    updated
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Custom provider not found for update".to_string())
}

/// Delete a custom provider and its associated models (SurrealDB does NOT
/// cascade-delete automatically, so we must delete models first).
pub async fn delete<C: Connection>(db: &Surreal<C>, id: &str) -> Result<(), String> {
    // Manually cascade-delete associated models first
    db.query("DELETE custom_provider_model WHERE provider = type::thing('custom_provider', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to delete provider models: {e}"))?;
    // Then delete the provider
    db.query("DELETE type::thing('custom_provider', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to delete custom provider: {e}"))?;
    Ok(())
}

/// Get all models for a custom provider, ordered by display_name.
pub async fn get_models<C: Connection>(
    db: &Surreal<C>,
    provider_id: &str,
) -> Result<Vec<CustomProviderModel>, String> {
    let safe_id = provider_id.replace('`', "``");
    let mut response = db
        .query(
            "SELECT * FROM custom_provider_model
         WHERE provider = type::thing('custom_provider', $id)
         ORDER BY display_name ASC",
        )
        .bind(("id", safe_id))
        .await
        .map_err(|e| format!("Failed to query provider models: {e}"))?;
    let records: Vec<CustomProviderModelRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse provider models: {e}"))?;
    Ok(records.into_iter().map(Into::into).collect())
}

/// Add a model to a custom provider.
pub async fn add_model<C: Connection>(
    db: &Surreal<C>,
    provider_id: &str,
    model_id: &str,
    display_name: &str,
) -> Result<CustomProviderModel, String> {
    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let mut response = db
        .query(
            "CREATE custom_provider_model SET
            id = $id,
            provider = type::thing('custom_provider', $provider_id),
            model_id = $model_id,
            display_name = $display_name",
        )
        .bind(("id", id.clone()))
        .bind(("provider_id", provider_id.to_owned()))
        .bind(("model_id", model_id.to_owned()))
        .bind(("display_name", display_name.to_owned()))
        .await
        .map_err(|e| format!("Failed to add model: {e}"))?;
    let created: Vec<CustomProviderModelRecord> = response
        .take(0)
        .map_err(|e| format!("Failed to parse created model: {e}"))?;
    created
        .into_iter()
        .next()
        .map(Into::into)
        .ok_or_else(|| "Failed to add model: no record returned".to_string())
}

/// Remove a model from a custom provider.
pub async fn remove_model<C: Connection>(db: &Surreal<C>, id: &str) -> Result<(), String> {
    db.query("DELETE type::thing('custom_provider_model', $id)")
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to delete model: {e}"))?;
    Ok(())
}
