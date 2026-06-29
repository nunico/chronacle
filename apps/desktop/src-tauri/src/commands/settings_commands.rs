//! Settings commands — read/write the key-value `setting` table.

use std::sync::Arc;

use crate::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Deserialize)]
pub(crate) struct SettingRow {
    pub(crate) id: surrealdb::sql::Thing,
    pub(crate) value: String,
}

/// Returns a map of all stored settings key-value pairs.
#[tauri::command]
pub async fn get_settings(
    state: State<'_, Arc<AppState>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let rows = get_all_settings(&state.db).await?;
    let map = rows
        .into_iter()
        .map(|r| (r.id.id.to_raw(), r.value))
        .collect();
    Ok(map)
}

/// Helper: query all settings from the DB.
pub(crate) async fn get_all_settings(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
) -> Result<Vec<SettingRow>, String> {
    let mut response = db
        .query("SELECT * FROM setting")
        .await
        .map_err(|e| format!("Database query failed: {e}"))?;

    let rows: Vec<SettingRow> = response
        .take(0)
        .map_err(|e| format!("Failed to parse settings: {e}"))?;

    Ok(rows)
}

/// Read all settings into a flat `HashMap`, for command handlers that need to
/// inspect several keys at once.
pub(crate) async fn settings_map(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
) -> Result<std::collections::HashMap<String, String>, String> {
    Ok(get_all_settings(db)
        .await?
        .into_iter()
        .map(|r| (r.id.id.to_raw(), r.value))
        .collect())
}

/// Upserts a single setting by key.
#[tauri::command]
pub async fn update_setting(
    state: State<'_, Arc<AppState>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let safe_key = key.replace('`', "``");
    let sql = format!("UPSERT setting:`{safe_key}` SET value = $value");

    state
        .db
        .query(sql)
        .bind(("value", value.to_owned()))
        .await
        .map_err(|e| format!("Failed to update setting: {e}"))?;

    Ok(())
}
