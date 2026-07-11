//! Vault sync commands — configure the vault root and run a reconcile.

use std::sync::Arc;

use crate::{build_vault_service, services::settings_service, spawn_outbound, AppState};
use chronacle_vault::reconcile::ReconcileReport;
use serde::Serialize;
use tauri::State;

/// Wire shape of `ReconcileReport` (snake_case matches the Rust struct).
#[derive(Serialize)]
pub struct ReconcileReportDto {
    pub exported: usize,
    pub unchanged: usize,
    pub adopted: usize,
    pub deferred_apply: usize,
    pub deferred_conflict: usize,
    pub deferred_delete: usize,
    pub failed: usize,
}

impl From<ReconcileReport> for ReconcileReportDto {
    fn from(r: ReconcileReport) -> Self {
        Self {
            exported: r.exported,
            unchanged: r.unchanged,
            adopted: r.adopted,
            deferred_apply: r.deferred_apply,
            deferred_conflict: r.deferred_conflict,
            deferred_delete: r.deferred_delete,
            failed: r.failed,
        }
    }
}

/// The configured vault root, or `None` when vault sync is off.
#[tauri::command]
pub async fn get_vault_path(state: State<'_, Arc<AppState>>) -> Result<Option<String>, String> {
    let settings = settings_service::get_all(&state.db).await?;
    Ok(settings
        .into_iter()
        .find(|s| s.key == "vault_sync_path")
        .map(|s| s.value)
        .filter(|v| !v.is_empty()))
}

/// Set or clear the vault root. Setting a path constructs the engine and runs a
/// full reconcile immediately; clearing it drops the engine.
#[tauri::command]
pub async fn set_vault_path(
    state: State<'_, Arc<AppState>>,
    vault_path: Option<String>,
) -> Result<(), String> {
    match vault_path {
        Some(path) if !path.is_empty() => {
            settings_service::upsert(&state.db, "vault_sync_path", &path).await?;
            let svc = build_vault_service(state.db.clone(), &path);
            svc.reconcile().await.map_err(|e| e.to_string())?;
            // Rebuild the queue and respawn the drain before publishing either
            // handle, so no producer can enqueue onto a channel with no drain.
            let new_outbound = spawn_outbound(Arc::clone(&svc));
            *state.vault.write().await = Some(svc);
            // Dropping the old producer here closes its channel; the old
            // drain loop drains whatever was already queued, then ends.
            *state.outbound.write().await = new_outbound;
        }
        _ => {
            settings_service::upsert(&state.db, "vault_sync_path", "").await?;
            *state.vault.write().await = None;
            *state.outbound.write().await = Arc::new(chronacle_core::NoopOutbound);
        }
    }
    Ok(())
}

/// Run a full reconcile now. Errors when no vault is configured.
#[tauri::command]
pub async fn vault_sync_now(state: State<'_, Arc<AppState>>) -> Result<ReconcileReportDto, String> {
    let guard = state.vault.read().await;
    let svc = guard.as_ref().ok_or("No vault path configured")?;
    svc.reconcile()
        .await
        .map(Into::into)
        .map_err(|e| e.to_string())
}
