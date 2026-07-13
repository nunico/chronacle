//! Vault sync commands — configure the vault root and run a reconcile.

use std::sync::Arc;

use crate::{
    build_vault_service, services::settings_service, spawn_outbound, spawn_watcher, AppState,
    VaultRuntime,
};
use chronacle_vault::reconcile::ReconcileReport;
use serde::Serialize;
use tauri::State;

/// Wire shape of `ReconcileReport` (snake_case matches the Rust struct).
/// `applied_refs` is omitted — the frontend has no use for the raw refs.
#[derive(Serialize)]
pub struct ReconcileReportDto {
    pub exported: usize,
    pub unchanged: usize,
    pub adopted: usize,
    pub applied: usize,
    pub conflicts: usize,
    pub resolved: usize,
    pub soft_deleted: usize,
    pub swept: usize,
    pub invalid: usize,
    pub failed: usize,
}

impl From<ReconcileReport> for ReconcileReportDto {
    fn from(r: ReconcileReport) -> Self {
        Self {
            exported: r.exported,
            unchanged: r.unchanged,
            adopted: r.adopted,
            applied: r.applied,
            conflicts: r.conflicts,
            resolved: r.resolved,
            soft_deleted: r.soft_deleted,
            swept: r.swept,
            invalid: r.invalid,
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

/// Clear-if-changed → reconcile → persist.
///
/// A different vault directory must never inherit the old dir's merge bases,
/// or every record reads as a deletion against the new (empty) folder (L2);
/// re-submitting the SAME path must leave the bases alone. The
/// `vault_sync_path` setting is written only after `reconcile` succeeds, so a
/// failed switch leaves the previous path and its bases in force.
///
/// Takes a bare database handle and an already-constructed service, so it is
/// reachable from a test with only a `mem://` SurrealDB and no Tauri `State`.
/// `pub` (not `#[tauri::command]`) so integration tests can drive it without a
/// Tauri `State` — the state-swapping stays in `set_vault_path`.
pub async fn configure_vault_path(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    svc: &chronacle_vault::reconcile::VaultSyncService,
    path: &str,
) -> Result<(), String> {
    let previous = settings_service::get_all(db)
        .await?
        .into_iter()
        .find(|s| s.key == "vault_sync_path")
        .map(|s| s.value)
        .filter(|v| !v.is_empty());

    if previous.as_deref() != Some(path) {
        svc.clear_all_bases().await.map_err(|e| e.to_string())?;
    }
    svc.reconcile().await.map_err(|e| e.to_string())?;
    // Persist only after the reconcile succeeded; on failure the old path
    // and old bases remain in force.
    settings_service::upsert(db, "vault_sync_path", path).await?;
    Ok(())
}

/// Set or clear the vault root. Setting a path constructs the engine and runs a
/// full reconcile immediately; clearing it drops the engine.
#[tauri::command]
pub async fn set_vault_path(
    state: State<'_, Arc<AppState>>,
    vault_path: Option<String>,
) -> Result<(), String> {
    let state_ref = state.inner().clone();
    match vault_path {
        Some(path) if !path.is_empty() => {
            let (svc, pending) = build_vault_service(state.db.clone(), &path);
            configure_vault_path(&state.db, &svc, &path).await?;
            // Rebuild the queue and respawn the drain before publishing either
            // handle, so no producer can enqueue onto a channel with no drain.
            let new_outbound = spawn_outbound(Arc::clone(&svc));
            let watcher_task = spawn_watcher(Arc::clone(&state_ref), Arc::clone(&svc), path);
            {
                let mut guard = state.vault.write().await;
                // Abort the old watcher before installing the new runtime — a
                // vault-path switch must never leave two watchers racing to
                // reconcile against different roots.
                if let Some(old) = guard.take() {
                    if let Some(t) = old.watcher_task {
                        t.abort();
                    }
                }
                *guard = Some(VaultRuntime {
                    svc,
                    pending,
                    watcher_task: Some(watcher_task),
                });
            }
            // Dropping the old producer here closes its channel; the old
            // drain loop drains whatever was already queued, then ends.
            *state.outbound.write().await = new_outbound;
        }
        _ => {
            settings_service::upsert(&state.db, "vault_sync_path", "").await?;
            {
                let mut guard = state.vault.write().await;
                if let Some(old) = guard.take() {
                    if let Some(t) = old.watcher_task {
                        t.abort();
                    }
                }
            }
            *state.outbound.write().await = Arc::new(chronacle_core::NoopOutbound);
        }
    }
    Ok(())
}

/// Run a full reconcile now. Errors when no vault is configured.
#[tauri::command]
pub async fn vault_sync_now(state: State<'_, Arc<AppState>>) -> Result<ReconcileReportDto, String> {
    let svc = state
        .vault
        .read()
        .await
        .as_ref()
        .map(|rt| Arc::clone(&rt.svc))
        .ok_or("No vault path configured")?;
    let report = svc.reconcile().await.map_err(|e| e.to_string())?;
    embed_applied_refs(&state, &report.applied_refs).await;
    Ok(report.into())
}

/// One frozen conflict, for the settings list and record-editor banners.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultConflictDto {
    pub id: String,   // bare id
    pub kind: String, // table
    pub name: String,
    pub key: String,
    pub sidecar_key: String,
}

/// Every record currently frozen in conflict. Empty when no vault is configured.
#[tauri::command]
pub async fn list_vault_conflicts(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<VaultConflictDto>, String> {
    let Some(svc) = state
        .vault
        .read()
        .await
        .as_ref()
        .map(|rt| Arc::clone(&rt.svc))
    else {
        return Ok(vec![]);
    };
    Ok(svc
        .conflicts()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|c| VaultConflictDto {
            id: c.vref.id,
            kind: c.vref.table,
            name: c.name,
            key: c.key,
            sidecar_key: c.sidecar_key,
        })
        .collect())
}

/// Re-embed entities whose GM parts just changed inbound. Best-effort — an
/// embedding failure only means stale semantic search until the next edit.
pub(crate) async fn embed_applied_refs(state: &AppState, refs: &[chronacle_core::VaultRef]) {
    for vref in refs {
        let Ok(kind) = crate::commands::entity_commands::kind_of_table(&vref.table) else {
            continue; // sessions / rule entries are not entity-embedded
        };
        match chronacle_extraction::entity_service::get_by_id(&state.db, &vref.id, kind).await {
            Ok(node) => {
                let provider = match state.embedding_provider.read() {
                    Ok(p) => p.clone(),
                    Err(_) => {
                        eprintln!(
                            "vault: re-embed of {} skipped: embedding_provider lock poisoned",
                            vref.to_thing()
                        );
                        continue;
                    }
                };
                if let Err(e) =
                    chronacle_extraction::entity_service::embed_node(&state.db, &provider, &node)
                        .await
                {
                    eprintln!("vault: re-embed of {} failed: {e}", vref.to_thing());
                }
            }
            Err(e) => eprintln!(
                "vault: load for re-embed of {} failed: {e}",
                vref.to_thing()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: the new command functions are referenced so the compiler
    /// verifies their signatures, imports, and return types.
    #[test]
    fn list_vault_conflicts_and_vault_sync_now_commands_compile() {
        let _ = list_vault_conflicts as fn(_) -> _;
        let _ = vault_sync_now as fn(_) -> _;
    }

    /// `VaultConflictDto` serializes with camelCase keys, matching the
    /// frontend `invoke()` wire contract.
    #[test]
    fn vault_conflict_dto_serializes_camel_case() {
        let dto = VaultConflictDto {
            id: "n1".to_string(),
            kind: "npc".to_string(),
            name: "Seraphina".to_string(),
            key: "campaigns/sov/entities/npc/seraphina.md".to_string(),
            sidecar_key: "campaigns/sov/entities/npc/seraphina.conflict.md".to_string(),
        };
        let v = serde_json::to_value(&dto).unwrap();
        assert_eq!(v["id"], "n1");
        assert_eq!(v["kind"], "npc");
        assert_eq!(
            v["sidecarKey"],
            "campaigns/sov/entities/npc/seraphina.conflict.md"
        );
    }
}
