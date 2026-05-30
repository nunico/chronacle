/// Settings service — CRUD operations for application settings.
///
/// Settings are stored as key-value pairs in the `setting` SurrealDB table.
/// Keys match the `setting` keys defined in ADR-002 (e.g. `llm_provider`,
/// `llm_model`, `embedding_backend`, `active_campaign_id`).
use serde::{Deserialize, Serialize};
use surrealdb::Connection;

/// A single setting key-value pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

/// Retrieve all settings from the database.
pub async fn get_all<C>(db: &surrealdb::Surreal<C>) -> Result<Vec<Setting>, String>
where
    C: Connection,
{
    let mut response = db
        .query("SELECT * FROM setting")
        .await
        .map_err(|e| format!("Failed to query settings: {e}"))?;

    #[derive(Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
        value: String,
    }

    let rows: Vec<Row> = response
        .take(0)
        .map_err(|e| format!("Failed to parse settings: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|r| Setting {
            key: r.id.id.to_string(),
            value: r.value,
        })
        .collect())
}

/// Upsert a single setting.
pub async fn upsert<C>(db: &surrealdb::Surreal<C>, key: &str, value: &str) -> Result<(), String>
where
    C: Connection,
{
    let safe_key = key.replace('`', "``");
    let sql = format!("UPSERT setting:`{safe_key}` SET value = $value");

    db.query(sql)
        .bind(("value", value.to_owned()))
        .await
        .map_err(|e| format!("Failed to upsert setting '{key}': {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_upsert_and_get() {
        let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .unwrap();
        db.use_ns("test").use_db("test").await.unwrap();

        db.query("DEFINE TABLE setting SCHEMAFULL; DEFINE FIELD value ON setting TYPE string;")
            .await
            .unwrap();

        upsert(&db, "test_key", "test_value").await.unwrap();
        let settings = get_all(&db).await.unwrap();

        assert!(settings.iter().any(|s| s.key == "test_key" && s.value == "test_value"));
    }
}
