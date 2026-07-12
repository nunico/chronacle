# Codex Tranche 5 (E1–E9) Implementation Plan — Inbound Vault Sync + Watcher

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn on the inbound direction of vault sync — GM edits in the vault flow back into the database, deletions soft-delete, divergent edits freeze into `.conflict.md` sidecars the GM resolves in the vault — with a `notify`-based filesystem watcher making it near-live.

**Architecture:** Follows `docs/superpowers/specs/2026-07-12-vault-inbound-sync-design.md`. `reconcile()` remains the single materialization path: the deferred `Apply | Conflict | SoftDelete` arm at `crates/chronacle-vault/src/reconcile.rs:114` becomes real. The watcher (`NotifyWatcher` in `chronacle-providers`) is purely a debounced trigger that filters our own writes via `PendingWrites` and requests a reconcile. Inbound DB writes go through new `VaultRecordStore` methods implemented in `chronacle-domain` (targeted updates + wikilink resync); re-embedding of applied entities happens at the app layer from `ReconcileReport.applied_refs` (the established "return changed refs; act at the command layer" pattern, `d952f13`).

**Tech Stack:** Rust (SurrealDB embedded, tokio, `notify` 8, `mockall`), Svelte 5 runes + TypeScript, Vitest, playwright-bdd.

## Global Constraints

- Every branch: `git checkout --no-track -b <branch> <base>` — never track main. Stacked chain: `feat/e1-fresh-baseline` from `main`, then `e2` ← `e1`, `e3` ← `e2`, `e4` ← `e3`, `e5` ← `e4`, `e6` ← `e5`, `e7` ← `e6`, `e8` ← `e7`, `e9` ← `e8`. After an upstream PR merges, rebase the stack (`git rebase --onto main <old-parent> <branch>`), force-push with `--force-with-lease`, retarget the next PR's base.
- **First push of a stacked branch needs an explicit refspec:** `git push -u origin <branch>:refs/heads/<branch>` — `push.default=upstream` otherwise corrupts the parent PR's head.
- **No new dependencies anywhere in this tranche.** `notify = "8"` is already in `crates/chronacle-providers/Cargo.toml:24`, approved and deny-clean. Do NOT add `notify-debouncer-mini`/`-full` — the debounce is hand-rolled with tokio. The content hash stays `std::hash::DefaultHasher`.
- Commit subjects ≤ 72 chars, imperative, conventional prefixes; never `--no-verify`.
- Clippy warnings are errors; public items in library crates need `///` docs; Svelte 5 runes only.
- **Filesystem access never appears in engine logic.** `chronacle-vault` must not depend on `tokio::fs`, `std::fs`, or `notify`. `NotifyWatcher` lives in `chronacle-providers`. A `use std::fs` under `crates/chronacle-vault/` is a review rejection.
- **Never clobber GM text.** Inbound writes touch ONLY `summary`/`notes` (entities), `notes` (sessions, rule entries). The fenced compiler block and frontmatter are reverted by re-export, never applied. Orphan-sweep deletes a file only when its content hash still equals the base.
- **`vault_deleted` is queried `!= true`, never `= false`** (pre-migration rows have no value at all).
- **Migrations are DEFINE-only and re-run every boot.** `DEFINE … OVERWRITE`; never `REMOVE`.
- **IDs are bare** in the vault layer (`"n1"`, not `"npc:n1"`); `VaultRef{table,id}`, `to_thing()` = `"table:id"`.
- Frontend `invoke()` argument keys are camelCase; struct args need `#[serde(rename_all = "camelCase")]`.
- Each PR ends green on the full CI gate: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && cargo deny check && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend`. **`cargo deny check` every time.**

## Out of scope (deliberately)

In-app conflict *resolution* UI (list + banner only), inbound frontmatter fields (`name`, session metadata), undelete/trash UI, path-scoped merge bases, id-less file creation (a vault file with no frontmatter id never creates a record), non-fs backend change feeds.

## Shared interfaces introduced by this tranche (single source of truth)

```rust
// crates/chronacle-core/src/vault.rs (E2)

/// GM-owned fields parsed from a vault file body; the only things applied inbound.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GmParts {
    pub summary: Option<String>,
    pub notes: Option<String>,
}

/// One persisted vault_sync_state row, as reconcile consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedRow {
    pub vref: VaultRef,
    pub key: VaultKey,
    /// None when the row was created by set_conflict and never had a base.
    pub synced_hash: Option<u64>,
    pub conflict: bool,
}

// trait VaultRecordStore — five new methods (all E1/E2):
async fn clear_all_synced(&self) -> Result<(), VaultRecordError>;                    // E1
async fn list_synced(&self) -> Result<Vec<SyncedRow>, VaultRecordError>;             // E2
async fn apply_gm_parts(&self, vref: &VaultRef, parts: &GmParts)
    -> Result<(), VaultRecordError>;                                                 // E2
async fn soft_delete(&self, vref: &VaultRef) -> Result<(), VaultRecordError>;        // E2
async fn set_conflict(&self, vref: &VaultRef, key: &str, in_conflict: bool)
    -> Result<(), VaultRecordError>;                                                 // E2
```

```rust
// crates/chronacle-vault (E1, E3, E4, E5)

// E1: PendingWrites moves INTO the service; export_refs/drain_loop lose the param.
impl VaultSyncService {
    pub fn new(store: Arc<dyn VaultStore>, records: Arc<dyn VaultRecordStore>,
               pending: Arc<PendingWrites>) -> Self;
    pub fn sweep_pending(&self);                        // E1, drain calls this
    pub async fn clear_all_bases(&self) -> Result<(), VaultError>;   // E1
    pub async fn export_refs(&self, refs: &HashSet<VaultRef>) -> Result<(), VaultError>; // E1 (param dropped)
    pub async fn is_own_write(&self, key: &str) -> bool;             // E6
    pub async fn conflicts(&self) -> Result<Vec<VaultConflict>, VaultError>; // E5
}
pub async fn drain_loop(rx: UnboundedReceiver<VaultRef>, svc: Arc<VaultSyncService>); // E1

// E3: report reshape (deferred_* fields DELETED)
pub struct ReconcileReport {
    pub exported: usize, pub unchanged: usize, pub adopted: usize,
    pub applied: usize, pub conflicts: usize, pub resolved: usize,
    pub soft_deleted: usize, pub swept: usize, pub invalid: usize, pub failed: usize,
    /// Refs whose GM parts were applied this pass; the app layer re-embeds these.
    pub applied_refs: Vec<VaultRef>,
}

// E4: crates/chronacle-vault/src/keys.rs
/// "…/seraphina.md" -> "…/seraphina.conflict.md" (already unmanaged per is_managed).
pub fn sidecar_key(key: &str) -> VaultKey;

// E5:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultConflict {
    pub vref: VaultRef, pub name: String, pub key: VaultKey, pub sidecar_key: VaultKey,
}
```

```rust
// apps/desktop/src-tauri/src/lib.rs (E6): AppState.vault becomes
pub struct VaultRuntime {
    pub svc: Arc<chronacle_vault::reconcile::VaultSyncService>,
    pub pending: Arc<chronacle_vault::outbound::PendingWrites>,
    pub watcher_task: Option<tauri::async_runtime::JoinHandle<()>>,
}
pub vault: tokio::sync::RwLock<Option<VaultRuntime>>,
```

New Tauri commands: `soft_delete_entity(id, kind)` (E5), `list_vault_conflicts()` (E5); `create_entity` gains `collection_id: Option<String>` and `campaign_id` becomes `Option<String>` (E5).

---

### Task 1 (E1): PendingWrites into the service + fresh baseline on vault-path switch

Fixes L2. `PendingWrites` becomes a constructor dependency of `VaultSyncService` (reconcile and the watcher must share it); `set_vault_path` clears all bases when the path *changes* and persists the setting only after a successful reconcile.

**Files:**
- Modify: `crates/chronacle-core/src/vault.rs` (add `clear_all_synced` to trait)
- Modify: `crates/chronacle-domain/src/vault_record_store.rs`
- Modify: `crates/chronacle-vault/src/reconcile.rs`, `crates/chronacle-vault/src/outbound.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src-tauri/src/commands/vault_commands.rs`

**Interfaces:**
- Produces: `VaultSyncService::new(store, records, pending)`, `sweep_pending()`, `clear_all_bases()`, `export_refs(&refs)` (no pending param), `drain_loop(rx, svc)`, `VaultRecordStore::clear_all_synced()`.
- Consumes: existing `PendingWrites` (`outbound.rs`), `settings_service::{get_all, upsert}`.

- [ ] **Step 1: Write the failing trait/impl test for `clear_all_synced`**

In `crates/chronacle-domain/src/vault_record_store.rs` tests:

```rust
#[tokio::test]
async fn clear_all_synced_wipes_every_sync_state_row() {
    let db = db().await;
    seed_campaign_npc(&db).await;
    let store = SurrealVaultRecordStore::new(db);
    let vref = VaultRef { table: "npc".into(), id: "n1".into() };
    store.set_synced_hash(&vref, "campaigns/c/entities/npc/a.md", 42).await.expect("set");

    store.clear_all_synced().await.expect("clear all");
    assert_eq!(store.get_synced_hash(&vref).await.expect("get"), None);
}
```

- [ ] **Step 2: Run it — expect FAIL** (`clear_all_synced` not on the trait):
`cargo test -p chronacle-domain clear_all_synced -- --nocapture`

- [ ] **Step 3: Add the trait method + Surreal impl**

`crates/chronacle-core/src/vault.rs`, inside `trait VaultRecordStore`:

```rust
    /// Wipe every persisted merge base (all `vault_sync_state` rows).
    /// Used when the vault path changes: the new directory gets a fresh baseline.
    async fn clear_all_synced(&self) -> Result<(), VaultRecordError>;
```

`crates/chronacle-domain/src/vault_record_store.rs`, in the impl:

```rust
    async fn clear_all_synced(&self) -> Result<(), VaultRecordError> {
        self.db
            .query("DELETE vault_sync_state")
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }
```

- [ ] **Step 4: Move `PendingWrites` into `VaultSyncService`**

`crates/chronacle-vault/src/reconcile.rs`:

```rust
pub struct VaultSyncService {
    store: Arc<dyn VaultStore>,
    records: Arc<dyn VaultRecordStore>,
    pending: Arc<crate::outbound::PendingWrites>,
}

impl VaultSyncService {
    /// Construct the engine over a storage backend, a record backend, and the
    /// shared write-loop guard (also consulted by the watcher, E6).
    pub fn new(
        store: Arc<dyn VaultStore>,
        records: Arc<dyn VaultRecordStore>,
        pending: Arc<crate::outbound::PendingWrites>,
    ) -> Self {
        Self { store, records, pending }
    }

    /// Expire stale write guards. The drain loop calls this after each batch.
    pub fn sweep_pending(&self) {
        self.pending.sweep();
    }

    /// Wipe every persisted merge base — fresh baseline for a new vault dir.
    pub async fn clear_all_bases(&self) -> Result<(), VaultError> {
        Ok(self.records.clear_all_synced().await?)
    }
}
```

Change `export_one_using(&self, vref, index)` to drop the `pending: Option<&PendingWrites>` parameter and always `self.pending.arm(&key, db);` before the write. `export_refs(&self, refs: &HashSet<VaultRef>)` drops its `pending` parameter. **Reconcile's `Export` arm also arms now** — insert before `self.store.write(&key, &rendered)` in `reconcile()`:

```rust
                SyncAction::Export => {
                    self.pending.arm(&key, db);
                    match self.store.write(&key, &rendered).await {
```

`crates/chronacle-vault/src/outbound.rs` — `drain_loop` drops its `pending` param:

```rust
pub async fn drain_loop(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<VaultRef>,
    svc: Arc<crate::reconcile::VaultSyncService>,
) {
    while let Some(first) = rx.recv().await {
        let mut batch = HashSet::new();
        batch.insert(first);
        while let Ok(next) = rx.try_recv() {
            batch.insert(next);
        }
        if let Err(e) = svc.export_refs(&batch).await {
            eprintln!("vault: drain batch failed to scan the vault index: {e}");
        }
        svc.sweep_pending();
    }
}
```

Fix every `VaultSyncService::new(...)` construction in tests: pass `Arc::new(PendingWrites::default())` as the third argument. The `export_refs_writes_to_the_existing_renamed_key` test drops its local `pending` binding.

- [ ] **Step 5: Rewire the composition root**

`apps/desktop/src-tauri/src/lib.rs` — `build_vault_service` gains the guard and returns both:

```rust
/// Construct the vault sync engine and its shared write guard.
fn build_vault_service(
    db: surrealdb::Surreal<surrealdb::engine::any::Any>,
    root: &str,
) -> (
    Arc<chronacle_vault::reconcile::VaultSyncService>,
    Arc<chronacle_vault::outbound::PendingWrites>,
) {
    let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
    let svc = Arc::new(chronacle_vault::reconcile::VaultSyncService::new(
        Arc::new(chronacle_providers::vault_store::LocalFsVaultStore::new(root)),
        Arc::new(chronacle_domain::vault_record_store::SurrealVaultRecordStore::new(db)),
        Arc::clone(&pending),
    ));
    (svc, pending)
}

pub(crate) fn spawn_outbound(
    svc: Arc<chronacle_vault::reconcile::VaultSyncService>,
) -> Arc<dyn chronacle_core::VaultOutbound> {
    let (producer, rx) = chronacle_vault::outbound::QueueOutbound::new();
    tauri::async_runtime::spawn(chronacle_vault::outbound::drain_loop(rx, svc));
    Arc::new(producer)
}
```

Adjust the startup wiring (`let (svc, _pending) = build_vault_service(...)` — `AppState.vault` stays `Option<Arc<VaultSyncService>>` until E6, when `VaultRuntime` lands and carries `pending`).

- [ ] **Step 6: Write the failing command-flow test for fresh baseline**

`apps/desktop/src-tauri/tests/vault_path_switch.rs` (new integration test; use the existing integration-test conventions — `mem://` DB, `run_migrations`, `tempfile::TempDir`):

```rust
//! L2: switching vault folders must never read as mass deletion.

use std::sync::Arc;
use chronacle_core::{VaultRecordStore, VaultRef};
use chronacle_domain::vault_record_store::SurrealVaultRecordStore;
use chronacle_providers::vault_store::LocalFsVaultStore;
use chronacle_vault::outbound::PendingWrites;
use chronacle_vault::reconcile::VaultSyncService;

async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://").await.expect("mem");
    db.use_ns("t").use_db("t").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db.query(
        "CREATE campaign:c1 SET name = 'SoV', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'Seraphina', notes = 'N.', \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    ).await.expect("seed").check().expect("seed ok");
    db
}

fn svc_for(db: &surrealdb::Surreal<surrealdb::engine::any::Any>, root: &std::path::Path)
    -> Arc<VaultSyncService>
{
    Arc::new(VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(root.to_str().unwrap())),
        Arc::new(SurrealVaultRecordStore::new(db.clone())),
        Arc::new(PendingWrites::default()),
    ))
}

#[tokio::test]
async fn switching_to_a_fresh_dir_after_clearing_bases_exports_cleanly() {
    let db = db().await;
    let dir_a = tempfile::TempDir::new().unwrap();
    let dir_b = tempfile::TempDir::new().unwrap();

    // First vault: export establishes a base.
    let a = svc_for(&db, dir_a.path());
    let r = a.reconcile().await.expect("reconcile a");
    assert_eq!(r.exported, 1);

    // Switch: fresh baseline, then reconcile against the empty dir B.
    let b = svc_for(&db, dir_b.path());
    b.clear_all_bases().await.expect("clear");
    let r = b.reconcile().await.expect("reconcile b");
    assert_eq!(r.exported, 1, "a fresh dir is a first export, not a deletion");

    // The record's file exists in B; nothing was flagged deleted.
    let store = SurrealVaultRecordStore::new(db.clone());
    let vref = VaultRef { table: "npc".into(), id: "n1".into() };
    assert!(store.get_synced_hash(&vref).await.expect("get").is_some());
}
```

- [ ] **Step 7: Run it** — `cargo test -p chronacle-desktop --test vault_path_switch` (adjust `-p` to the actual `src-tauri` package name in `apps/desktop/src-tauri/Cargo.toml`). Expected: PASS once Steps 3–5 compile.

- [ ] **Step 8: Reorder `set_vault_path`**

`apps/desktop/src-tauri/src/commands/vault_commands.rs`:

```rust
#[tauri::command]
pub async fn set_vault_path(
    state: State<'_, Arc<AppState>>,
    vault_path: Option<String>,
) -> Result<(), String> {
    match vault_path {
        Some(path) if !path.is_empty() => {
            let previous = settings_service::get_all(&state.db)
                .await?
                .into_iter()
                .find(|s| s.key == "vault_sync_path")
                .map(|s| s.value)
                .filter(|v| !v.is_empty());

            let (svc, _pending) = build_vault_service(state.db.clone(), &path);
            // Fresh baseline: a different directory must never inherit the old
            // dir's bases, or every record reads as SoftDelete (L2).
            if previous.as_deref() != Some(path.as_str()) {
                svc.clear_all_bases().await.map_err(|e| e.to_string())?;
            }
            svc.reconcile().await.map_err(|e| e.to_string())?;
            // Persist only after the reconcile succeeded; on failure the old
            // path and old bases remain in force.
            settings_service::upsert(&state.db, "vault_sync_path", &path).await?;
            let new_outbound = spawn_outbound(Arc::clone(&svc));
            *state.vault.write().await = Some(svc);
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
```

- [ ] **Step 9: Full workspace check** — `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace`. Expected: green (mock `MockVaultRecordStore` regenerates `clear_all_synced` automatically via `automock`; tests that construct the service updated in Step 4).

- [ ] **Step 10: Commit** — `git add -A && git commit -m "feat(vault): fresh baseline on path switch; guard into service"`

---

### Task 2 (E2): Schema + record-store inbound surface

The persistence layer inbound needs: the `conflict` flag, `vault_deleted` on `rule_entry`, and the four remaining `VaultRecordStore` methods with their Surreal implementations.

**Files:**
- Modify: `crates/chronacle-db/src/schema/003_vault_sync.surql`
- Modify: `crates/chronacle-core/src/vault.rs` (add `GmParts`, `SyncedRow`, trait methods; re-export from lib.rs like the existing vault types)
- Modify: `crates/chronacle-domain/src/vault_record_store.rs`
- Test: same files (`#[cfg(test)]` + the existing mem-db test harness in `vault_record_store.rs`)

**Interfaces:**
- Produces: `GmParts`, `SyncedRow` (shapes in the Shared interfaces block), `list_synced()`, `apply_gm_parts()`, `soft_delete()`, `set_conflict()`.
- Consumes: `chronacle_extraction::wikilink::parse_and_sync_wikilinks` (already a dependency of `chronacle-domain`).

- [ ] **Step 1: Schema additions** (DEFINE-only, idempotent):

Append to `crates/chronacle-db/src/schema/003_vault_sync.surql`:

```sql
-- ── Tranche 5: inbound ────────────────────────────────────────────────────────
-- `synced_hash` gains DEFAULT '' so a conflict-only row (set_conflict before any
-- base exists) can be created; '' parses to no base (None) on read.
DEFINE FIELD OVERWRITE synced_hash ON TABLE vault_sync_state TYPE string DEFAULT '';
-- Frozen-conflict marker: while true AND the sidecar exists, reconcile neither
-- applies nor exports this record.
DEFINE FIELD OVERWRITE conflict ON TABLE vault_sync_state TYPE bool DEFAULT false;
-- Rule entries become soft-deletable like the other nine tables.
DEFINE FIELD OVERWRITE vault_deleted ON TABLE rule_entry TYPE bool DEFAULT false;
```

- [ ] **Step 2: Core types + trait methods**

`crates/chronacle-core/src/vault.rs` — add `GmParts` and `SyncedRow` exactly as in the Shared interfaces block (both `#[derive(Debug, Clone, PartialEq, Eq)]`, `GmParts` also `Default`), and the four trait methods with docs:

```rust
    /// Every persisted sync-state row. One query per reconcile pass; also
    /// powers the orphan sweep (rows whose record no longer syncs).
    async fn list_synced(&self) -> Result<Vec<SyncedRow>, VaultRecordError>;
    /// Apply GM-owned fields inbound. Entities: summary + notes (+ wikilink
    /// resync, codex_stale). Sessions and rule entries: notes only.
    async fn apply_gm_parts(&self, vref: &VaultRef, parts: &GmParts)
        -> Result<(), VaultRecordError>;
    /// Set `vault_deleted = true`. The record disappears from `list_all`.
    async fn soft_delete(&self, vref: &VaultRef) -> Result<(), VaultRecordError>;
    /// Mark or clear the frozen-conflict flag for a record's row (UPSERT).
    async fn set_conflict(&self, vref: &VaultRef, key: &str, in_conflict: bool)
        -> Result<(), VaultRecordError>;
```

Update the `list_all` rule-entry query in `vault_record_store.rs` to add `WHERE vault_deleted != true`.

- [ ] **Step 3: Write the failing round-trip tests** (in `vault_record_store.rs` tests, reusing `db()` / `seed_campaign_npc`):

```rust
#[tokio::test]
async fn apply_gm_parts_updates_only_summary_and_notes() {
    let db = db().await;
    seed_campaign_npc(&db).await;
    let store = SurrealVaultRecordStore::new(db.clone());
    let vref = VaultRef { table: "npc".into(), id: "n1".into() };

    store.apply_gm_parts(&vref, &chronacle_core::GmParts {
        summary: Some("New summary.".into()),
        notes: Some("Edited in Obsidian. [[Iron Tower]]".into()),
    }).await.expect("apply");

    let rec = store.load(&vref).await.expect("load").expect("exists");
    let chronacle_core::VaultRecord::Entity(e) = rec else { panic!("entity") };
    assert_eq!(e.summary.as_deref(), Some("New summary."));
    assert_eq!(e.notes.as_deref(), Some("Edited in Obsidian. [[Iron Tower]]"));
    assert_eq!(e.name, "Seraphina Aldric", "name is never applied inbound");
    assert_eq!(e.codex_article.as_deref(), Some("Compiled."), "article untouched");
}

#[tokio::test]
async fn soft_delete_removes_the_record_from_list_all() {
    let db = db().await;
    seed_campaign_npc(&db).await;
    let store = SurrealVaultRecordStore::new(db);
    let vref = VaultRef { table: "npc".into(), id: "n1".into() };
    store.soft_delete(&vref).await.expect("soft delete");
    assert!(store.list_all().await.expect("list").is_empty());
}

#[tokio::test]
async fn set_conflict_round_trips_through_list_synced_without_a_base() {
    let db = db().await;
    seed_campaign_npc(&db).await;
    let store = SurrealVaultRecordStore::new(db);
    let vref = VaultRef { table: "npc".into(), id: "n1".into() };

    store.set_conflict(&vref, "campaigns/c/entities/npc/a.md", true).await.expect("set");
    let rows = store.list_synced().await.expect("list");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].conflict);
    assert_eq!(rows[0].synced_hash, None, "conflict-only row has no base");
    assert_eq!(rows[0].key, "campaigns/c/entities/npc/a.md");

    store.set_conflict(&vref, "campaigns/c/entities/npc/a.md", false).await.expect("clear");
    assert!(!store.list_synced().await.expect("list")[0].conflict);
}
```

- [ ] **Step 4: Run — expect FAIL** (`cargo test -p chronacle-domain vault_record_store`), then implement:

```rust
    async fn list_synced(&self) -> Result<Vec<SyncedRow>, VaultRecordError> {
        #[derive(Debug, Deserialize)]
        struct Row {
            record: String,
            key: String,
            synced_hash: String,
            #[serde(default)]
            conflict: bool,
        }
        let mut response = self
            .db
            .query("SELECT record, key, synced_hash, conflict FROM vault_sync_state")
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        let rows: Vec<Row> = response.take(0).map_err(backend_err)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let vref = VaultRef::parse(&r.record)?;
                Some(SyncedRow {
                    vref,
                    key: r.key,
                    synced_hash: r.synced_hash.parse::<u64>().ok(),
                    conflict: r.conflict,
                })
            })
            .collect())
    }

    async fn apply_gm_parts(
        &self,
        vref: &VaultRef,
        parts: &GmParts,
    ) -> Result<(), VaultRecordError> {
        // `Option::None` binds as NONE, which SCHEMAFULL `… | NULL` rejects —
        // bind explicit NULL (same convention as entity_service::update).
        fn opt(o: &Option<String>) -> surrealdb::sql::Value {
            o.clone().map_or(surrealdb::sql::Value::Null, Into::into)
        }
        match vref.table.as_str() {
            "session" => {
                self.db
                    .query(
                        "UPDATE type::thing('session', $id) SET \
                             notes = $notes, updated_at = time::now()",
                    )
                    .bind(("id", vref.id.clone()))
                    .bind(("notes", parts.notes.clone().unwrap_or_default()))
                    .await
                    .map_err(backend_err)?
                    .check()
                    .map_err(backend_err)?;
            }
            "rule_entry" => {
                self.db
                    .query(
                        "UPDATE type::thing('rule_entry', $id) SET notes = $notes",
                    )
                    .bind(("id", vref.id.clone()))
                    .bind(("notes", opt(&parts.notes)))
                    .await
                    .map_err(backend_err)?
                    .check()
                    .map_err(backend_err)?;
            }
            table if ENTITY_TABLES.contains(&table) => {
                self.db
                    .query(
                        "UPDATE type::thing($table, $id) SET \
                             summary = $summary, notes = $notes, \
                             codex_stale = true, updated_at = time::now()",
                    )
                    .bind(("table", table.to_owned()))
                    .bind(("id", vref.id.clone()))
                    .bind(("summary", opt(&parts.summary)))
                    .bind(("notes", opt(&parts.notes)))
                    .await
                    .map_err(backend_err)?
                    .check()
                    .map_err(backend_err)?;
                // Keep wikilinks (relates_to edges) consistent with the new notes,
                // exactly as an in-app edit would (entity_service::update does this).
                if let Some(notes) = &parts.notes {
                    if let Ok(Some(VaultRecord::Entity(e))) = self.load(vref).await {
                        use chronacle_extraction::wikilink::{
                            parse_and_sync_wikilinks, WikilinkScope,
                        };
                        let scope = match &e.scope {
                            VaultScope::Campaign { id, .. } => {
                                WikilinkScope::Campaign { campaign_id: id }
                            }
                            VaultScope::Collection { id, .. } => {
                                WikilinkScope::Collection { collection_id: id }
                            }
                        };
                        let _ = parse_and_sync_wikilinks(
                            &self.db, table, &vref.id, notes, scope,
                        )
                        .await;
                    }
                }
            }
            other => {
                return Err(VaultRecordError::Backend(format!(
                    "apply_gm_parts: unsupported table {other}"
                )))
            }
        }
        Ok(())
    }

    async fn soft_delete(&self, vref: &VaultRef) -> Result<(), VaultRecordError> {
        self.db
            .query("UPDATE type::thing($table, $id) SET vault_deleted = true")
            .bind(("table", vref.table.clone()))
            .bind(("id", vref.id.clone()))
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }

    async fn set_conflict(
        &self,
        vref: &VaultRef,
        key: &str,
        in_conflict: bool,
    ) -> Result<(), VaultRecordError> {
        let record = vref.to_thing();
        self.db
            .query(
                "UPSERT type::thing('vault_sync_state', $record) \
                 SET record = $record, key = $key, conflict = $flag",
            )
            .bind(("record", record))
            .bind(("key", key.to_owned()))
            .bind(("flag", in_conflict))
            .await
            .map_err(backend_err)?
            .check()
            .map_err(backend_err)?;
        Ok(())
    }
```

Note: `WikilinkScope`'s exact field types must match its definition in `chronacle-extraction` — check `crates/chronacle-extraction/src/wikilink/` and adapt (it may take `&str`). If `parse_and_sync_wikilinks` is not `pub`, make it `pub` (it is already called cross-module).

- [ ] **Step 5: Run tests** — `cargo test -p chronacle-domain && cargo test -p chronacle-core`. Expected: PASS. Also add a soft-delete test for `rule_entry` (mirrors `soft_delete_removes_the_record_from_list_all` with the rule-entry seed from `list_all_returns_rule_entries_with_collection_scope`).

- [ ] **Step 6: Commit** — `git commit -am "feat(vault): record-store inbound surface + conflict flag schema"`

---

### Task 3 (E3): Reconcile materializes Apply + SoftDelete + orphan sweep

The core of the tranche. The deferred arm becomes real for `Apply` and `SoftDelete`; `Conflict` is E4 (this task leaves a `conflicts += 1` count with no side effects, explicitly temporary). Report reshaped.

**Files:**
- Modify: `crates/chronacle-vault/src/reconcile.rs`
- Modify: `apps/desktop/src-tauri/src/commands/vault_commands.rs` (DTO follows the report)
- Test: `crates/chronacle-vault/src/reconcile.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `list_synced()`, `apply_gm_parts()`, `soft_delete()` (E2); `frontmatter::parse`, `markdown::split_body`, `render_record`, `content_hash`.
- Produces: the new `ReconcileReport` (Shared interfaces block); private `apply_inbound()` used by E4's resolution path.

- [ ] **Step 1: Reshape `ReconcileReport`** exactly as in the Shared interfaces block (delete `deferred_apply`/`deferred_conflict`/`deferred_delete`; add `applied`, `conflicts`, `resolved`, `soft_deleted`, `swept`, `invalid`, `applied_refs: Vec<VaultRef>`). It keeps `#[derive(Debug, Default, PartialEq, Eq)]`. Update `ReconcileReportDto` in `vault_commands.rs` to mirror it (all `usize` fields; omit `applied_refs` from the DTO — the frontend doesn't need refs).

- [ ] **Step 2: Write the failing tests** (adapt the existing deferral tests — they assert the OLD behavior and must flip):

```rust
    /// db == base, file differs => Apply: GM parts land in the DB, the canonical
    /// render is re-exported (fence/frontmatter edits reverted), base updated.
    #[tokio::test]
    async fn reconcile_applies_an_inbound_edit_and_reexports_canonical() {
        let old_rendered = crate::render::render_record(&npc(Some("A.")));
        let base = crate::render::content_hash(&old_rendered);
        // The GM's file: valid frontmatter, edited notes, and a tampered fence.
        let gm_file = format!(
            "---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\
             \n{}\nGM EDIT INSIDE FENCE\n{}\n\n## Notes\n\nEdited notes.\n",
            crate::markdown::FENCE_START, crate::markdown::FENCE_END
        );

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(gm_file.clone()));
        // The re-export writes the canonical render (fence reverted).
        store
            .expect_write()
            .withf(|k, content| k == KEY && !content.contains("GM EDIT INSIDE FENCE"))
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef { table: "npc".into(), id: "n1".into() },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: false,
            }])
        });
        records
            .expect_apply_gm_parts()
            .withf(|_, parts| parts.notes.as_deref() == Some("Edited notes."))
            .times(1)
            .returning(|_, _| Ok(()));
        // After apply, load() returns the updated record for re-render.
        let updated = {
            let VaultRecord::Entity(mut e) = npc(Some("A.")) else { unreachable!() };
            e.notes = Some("Edited notes.".into());
            VaultRecord::Entity(e)
        };
        records.expect_load().returning(move |_| Ok(Some(updated.clone())));
        records.expect_set_synced_hash().times(1).returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store), Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.applied, 1);
        assert_eq!(report.applied_refs.len(), 1);
        assert_eq!(report.exported, 0);
    }

    /// A managed file with unparsable frontmatter is counted invalid and never
    /// applied or overwritten.
    #[tokio::test]
    async fn reconcile_counts_an_unparsable_file_as_invalid_and_leaves_it() {
        let base = crate::render::content_hash(&crate::render::render_record(&npc(Some("A."))));
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(|_| Ok("no frontmatter at all".to_string()));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef { table: "npc".into(), id: "n1".into() },
                key: KEY.into(),
                synced_hash: Some(base),
                conflict: false,
            }])
        });
        records.expect_apply_gm_parts().never();
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(
            Arc::new(store), Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.invalid, 1);
        assert_eq!(report.applied, 0);
    }

    /// base set, file gone => SoftDelete: vault_deleted set, base cleared,
    /// file never resurrected.
    #[tokio::test]
    async fn reconcile_soft_deletes_a_record_whose_file_is_gone() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(|| {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef { table: "npc".into(), id: "n1".into() },
                key: KEY.into(),
                synced_hash: Some(123),
                conflict: false,
            }])
        });
        records.expect_soft_delete().times(1).returning(|_| Ok(()));
        records.expect_clear_synced_hash().times(1).returning(|_| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store), Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.soft_deleted, 1);
    }

    /// A sync-state row whose record no longer exists (in-app soft delete):
    /// the file is deleted only while it still matches the base.
    #[tokio::test]
    async fn orphan_sweep_deletes_an_unmodified_file_and_spares_an_edited_one() {
        let rendered = crate::render::render_record(&npc(Some("A.")));
        let matching = crate::render::content_hash(&rendered);

        // Case 1: file matches the base -> deleted.
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(rendered.clone()));
        store.expect_delete().withf(|k| k == KEY).times(1).returning(|_| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![])); // record is gone
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef { table: "npc".into(), id: "n1".into() },
                key: KEY.into(),
                synced_hash: Some(matching),
                conflict: false,
            }])
        });
        records.expect_clear_synced_hash().times(1).returning(|_| Ok(()));

        let svc = VaultSyncService::new(
            Arc::new(store), Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.swept, 1);

        // Case 2: the GM edited the file after the record died -> file survives.
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(|_| Ok("GM kept writing here".to_string()));
        store.expect_delete().never();
        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef { table: "npc".into(), id: "n1".into() },
                key: KEY.into(),
                synced_hash: Some(matching),
                conflict: false,
            }])
        });
        records.expect_clear_synced_hash().times(1).returning(|_| Ok(()));
        let svc = VaultSyncService::new(
            Arc::new(store), Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.swept, 1, "row cleared either way");
    }
```

Delete the two now-obsolete deferral tests (`reconcile_defers_apply_and_conflict_without_writing`, `reconcile_defers_soft_delete_and_does_not_resurrect_the_file`); their invariants are superseded by the tests above.

- [ ] **Step 3: Run — expect FAIL**: `cargo test -p chronacle-vault reconcile`

- [ ] **Step 4: Implement**

In `reconcile()`: fetch sync state once, up front (replaces the per-record `get_synced_hash` call):

```rust
        let synced: HashMap<VaultRef, chronacle_core::SyncedRow> = self
            .records
            .list_synced()
            .await?
            .into_iter()
            .map(|row| (row.vref.clone(), row))
            .collect();
```

Per record, `let state = synced.get(vref);`, `let base = state.and_then(|s| s.synced_hash);`. Keep the file *content* (not just its hash) when a file exists — the Apply path parses it:

```rust
            let file_content = match &existing_key {
                Some(k) => Some(self.store.read(k).await?),
                None => None,
            };
            let file = file_content.as_deref().map(content_hash);
```

New match arms (Conflict is a bare count until E4 — leave a `// E4 materializes this` comment):

```rust
                SyncAction::Apply => {
                    match self.apply_inbound(vref, &key, file_content.as_deref().unwrap_or("")).await {
                        Ok(true) => {
                            report.applied += 1;
                            report.applied_refs.push(vref.clone());
                        }
                        Ok(false) => report.invalid += 1,
                        Err(e) => {
                            eprintln!("vault: inbound apply of {key} failed: {e}");
                            report.failed += 1;
                        }
                    }
                }
                SyncAction::Conflict => report.conflicts += 1, // E4 materializes this
                SyncAction::SoftDelete => {
                    self.records.soft_delete(vref).await?;
                    self.records.clear_synced_hash(vref).await?;
                    report.soft_deleted += 1;
                }
```

The private apply helper (returns `Ok(false)` on a file whose frontmatter cannot identify it — the `invalid` bucket):

```rust
    /// Apply the GM-owned parts of `file_content` to the DB, then re-export the
    /// canonical render over `key` and set the base. Returns Ok(false) when the
    /// file has no parsable frontmatter (never applied, never overwritten).
    async fn apply_inbound(
        &self,
        vref: &VaultRef,
        key: &str,
        file_content: &str,
    ) -> Result<bool, VaultError> {
        let Ok((_fm, body)) = crate::frontmatter::parse(file_content) else {
            return Ok(false);
        };
        let parts = crate::markdown::split_body(&body);
        let gm = chronacle_core::GmParts { summary: parts.summary, notes: parts.notes };
        self.records.apply_gm_parts(vref, &gm).await?;

        // Re-export canonical: fence and frontmatter edits are reverted here,
        // and the record settles to NoOp on the next pass.
        let Some(record) = self.records.load(vref).await? else {
            return Ok(true); // deleted mid-flight; the next pass sweeps it
        };
        let rendered = render_record(&record);
        let hash = content_hash(&rendered);
        self.pending.arm(key, hash);
        self.store.write(key, &rendered).await?;
        self.records.set_synced_hash(vref, key, hash).await?;
        Ok(true)
    }
```

The orphan sweep, after the record loop (record refs collected into a `HashSet<&VaultRef>` while iterating):

```rust
        // Orphan sweep: rows whose record no longer syncs (in-app soft/hard
        // delete). Delete the file only while it still matches the base —
        // never clobber prose the GM kept editing after the record died.
        let record_refs: std::collections::HashSet<&VaultRef> =
            records.iter().map(vref_of).collect();
        for row in synced.values() {
            if record_refs.contains(&row.vref) {
                continue;
            }
            match self.store.read(&row.key).await {
                Ok(content) => {
                    if row.synced_hash == Some(content_hash(&content)) {
                        if let Err(e) = self.store.delete(&row.key).await {
                            eprintln!("vault: orphan delete of {} failed: {e}", row.key);
                            report.failed += 1;
                            continue;
                        }
                    }
                }
                Err(chronacle_core::VaultStoreError::NotFound(_)) => {}
                Err(e) => {
                    eprintln!("vault: orphan read of {} failed: {e}", row.key);
                    report.failed += 1;
                    continue;
                }
            }
            self.records.clear_synced_hash(&row.vref).await?;
            report.swept += 1;
        }
```

- [ ] **Step 5: Run** — `cargo test -p chronacle-vault`. Expected: PASS, including all pre-existing export tests (they gain `expect_list_synced().returning(|| Ok(vec![]))` where they previously stubbed `get_synced_hash` — convert each; `get_synced_hash` remains on the trait for `export_one`).

- [ ] **Step 6: Integration round-trip** — add to `apps/desktop/src-tauri/tests/vault_path_switch.rs` (rename the file to `vault_inbound.rs` if cleaner, keeping the E1 test):

```rust
#[tokio::test]
async fn gm_edit_round_trips_through_reconcile_into_the_db() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("first export");

    // Find the exported file and append GM notes outside the fence.
    let path = dir.path()
        .join("campaigns/sov/entities/npc").read_dir().unwrap()
        .next().unwrap().unwrap().path();
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\n\nInbound edit from Obsidian.\n")).unwrap();

    let report = svc.reconcile().await.expect("inbound pass");
    assert_eq!(report.applied, 1);

    #[derive(serde::Deserialize)]
    struct Row { notes: Option<String> }
    let mut resp = db.query("SELECT notes FROM npc:n1").await.unwrap().check().unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0].notes.as_deref().unwrap_or("").contains("Inbound edit from Obsidian."));

    // Third pass: everything converged.
    let report = svc.reconcile().await.expect("settle");
    assert_eq!(report.unchanged, 1);
}
```

(The campaign folder slug depends on the seeded campaign name — seed `name = 'SoV'` gives `campaigns/sov/`; keep seed and assertion in agreement.)

- [ ] **Step 7: Run the integration test**, then the workspace: `cargo test --workspace`. Expected: PASS.

- [ ] **Step 8: Commit** — `git commit -am "feat(vault): materialize inbound apply, soft-delete, orphan sweep"`

---

### Task 4 (E4): Conflict lifecycle (sidecar + freeze + deletion-as-resolution)

**Files:**
- Modify: `crates/chronacle-vault/src/keys.rs` (`sidecar_key`), `crates/chronacle-vault/src/reconcile.rs`
- Test: same files

**Interfaces:**
- Produces: `keys::sidecar_key(key: &str) -> VaultKey`; the conflict handling inside `reconcile()`.
- Consumes: `set_conflict()` (E2), `apply_inbound()` (E3), `PendingWrites` (E1).

The lifecycle (from the spec, normative):

| Observed | Action |
| --- | --- |
| `Conflict`, flag unset | write sidecar (armed), `set_conflict(true)`, `report.conflicts += 1`; no apply/export/base |
| `Conflict`, flag set, sidecar present | refresh sidecar iff DB render differs from sidecar content (armed write); still frozen, `report.conflicts += 1` |
| `Conflict`, flag set, sidecar **absent** | resolution: `apply_inbound` the GM file, `set_conflict(false)`, `report.resolved += 1` |
| non-`Conflict` action, flag set | delete sidecar if present, `set_conflict(false)`, then handle the action normally |

- [ ] **Step 1: `sidecar_key` + test** in `keys.rs`:

```rust
/// The compiler-owned conflict sidecar for `key`. `is_managed` already treats
/// `*.conflict.md` as unmanaged, so a sidecar can never hijack the index.
pub fn sidecar_key(key: &str) -> VaultKey {
    match key.strip_suffix(".md") {
        Some(stem) => format!("{stem}.conflict.md"),
        None => format!("{key}.conflict.md"),
    }
}

#[test]
fn sidecar_key_is_always_unmanaged() {
    let k = sidecar_key("campaigns/c/entities/npc/a.md");
    assert_eq!(k, "campaigns/c/entities/npc/a.conflict.md");
    assert!(!is_managed(&k));
}
```

- [ ] **Step 2: Write the failing lifecycle tests** in `reconcile.rs` (one per table row; the first in full):

```rust
    /// Both sides diverged from the base: the DB render lands in the sidecar,
    /// the GM's file is untouched, the record is frozen (no base update).
    #[tokio::test]
    async fn a_new_conflict_writes_the_sidecar_and_freezes_the_record() {
        let base = 111_u64; // both sides differ from it and from each other
        let db_render = crate::render::render_record(&npc(Some("A.")));

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().withf(|k| k == KEY)
            .returning(|_| Ok("---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\nGM version.\n".to_string()));
        let expected_sidecar = crate::keys::sidecar_key(KEY);
        let render_for_assert = db_render.clone();
        store.expect_write()
            .withf(move |k, content| k == expected_sidecar && content == render_for_assert)
            .times(1)
            .returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_list_synced().returning(move || {
            Ok(vec![chronacle_core::SyncedRow {
                vref: VaultRef { table: "npc".into(), id: "n1".into() },
                key: KEY.into(), synced_hash: Some(base), conflict: false,
            }])
        });
        records.expect_set_conflict()
            .withf(|_, _, flag| *flag).times(1).returning(|_, _, _| Ok(()));
        records.expect_apply_gm_parts().never();
        records.expect_set_synced_hash().never(); // frozen: no base movement

        let svc = VaultSyncService::new(
            Arc::new(store), Arc::new(records),
            Arc::new(crate::outbound::PendingWrites::default()),
        );
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.conflicts, 1);
    }
```

Plus three siblings (write them with the same mock vocabulary):
- `a_frozen_conflict_with_a_live_sidecar_stays_frozen_and_refreshes_a_stale_sidecar` — `conflict: true`, sidecar `read` returns an OLD render, expect exactly one `write(sidecar, <current render>)`, `set_conflict` never, `apply_gm_parts` never.
- `deleting_the_sidecar_resolves_the_conflict_by_applying_the_gm_file` — `conflict: true`, sidecar `read` returns `Err(VaultStoreError::NotFound(..))`, expect `apply_gm_parts` once, `write(KEY, canonical)` once, `set_synced_hash` once, `set_conflict(.., false)` once, `report.resolved == 1`.
- `an_evaporated_conflict_cleans_up_the_sidecar_and_proceeds` — `conflict: true` but `decide` now returns `NoOp` (file back to base == db): expect `delete(sidecar)` once, `set_conflict(.., false)` once, `report.unchanged == 1`.

- [ ] **Step 3: Run — expect FAIL**, then implement in `reconcile()`. Replace the E3 placeholder `SyncAction::Conflict => report.conflicts += 1` and wrap the action dispatch:

```rust
            let action = decide(base, db, file);
            let was_frozen = state.is_some_and(|s| s.conflict);
            let sidecar = crate::keys::sidecar_key(&key);

            if was_frozen && action != SyncAction::Conflict {
                // The conflict evaporated on its own (e.g. the GM reverted the
                // file). Clean up before handling the action normally.
                match self.store.delete(&sidecar).await {
                    Ok(()) | Err(chronacle_core::VaultStoreError::NotFound(_)) => {}
                    Err(e) => eprintln!("vault: sidecar cleanup of {sidecar} failed: {e}"),
                }
                self.records.set_conflict(vref, &key, false).await?;
            }

            match action {
                // … NoOp / AdoptBase / Export / Apply / SoftDelete arms unchanged (E3) …
                SyncAction::Conflict => {
                    let sidecar_content = match self.store.read(&sidecar).await {
                        Ok(c) => Some(c),
                        Err(chronacle_core::VaultStoreError::NotFound(_)) => None,
                        Err(e) => {
                            eprintln!("vault: sidecar read of {sidecar} failed: {e}");
                            report.failed += 1;
                            continue;
                        }
                    };
                    match (was_frozen, sidecar_content) {
                        // Deletion of the sidecar is the GM's resolution signal.
                        (true, None) => {
                            match self
                                .apply_inbound(vref, &key, file_content.as_deref().unwrap_or(""))
                                .await
                            {
                                Ok(true) => {
                                    self.records.set_conflict(vref, &key, false).await?;
                                    report.resolved += 1;
                                    report.applied_refs.push(vref.clone());
                                }
                                Ok(false) => report.invalid += 1,
                                Err(e) => {
                                    eprintln!("vault: conflict resolution of {key} failed: {e}");
                                    report.failed += 1;
                                }
                            }
                        }
                        // Frozen: keep the sidecar current with the DB render.
                        (true, Some(existing)) => {
                            if content_hash(&existing) != db {
                                self.pending.arm(&sidecar, db);
                                if let Err(e) = self.store.write(&sidecar, &rendered).await {
                                    eprintln!("vault: sidecar refresh of {sidecar} failed: {e}");
                                    report.failed += 1;
                                    continue;
                                }
                            }
                            report.conflicts += 1;
                        }
                        // New conflict: preserve the DB version, freeze.
                        (false, _) => {
                            self.pending.arm(&sidecar, db);
                            if let Err(e) = self.store.write(&sidecar, &rendered).await {
                                eprintln!("vault: sidecar write of {sidecar} failed: {e}");
                                report.failed += 1;
                                continue;
                            }
                            self.records.set_conflict(vref, &key, true).await?;
                            report.conflicts += 1;
                        }
                    }
                }
            }
```

Also extend the **orphan sweep** (E3) to delete an orphan's sidecar the same way it deletes the file (read sidecar; `NotFound` is fine; delete unconditionally — sidecars are compiler-owned).

Note the unmanaged-file conflict case: `decide(None, db, Some(other))` → `Conflict` with `state == None`; the `(false, _)` arm handles it — `set_conflict` UPSERTs a row with no base, which E2's `list_synced` returns as `synced_hash: None`, so the next pass still computes `Conflict` (frozen) until the GM deletes the sidecar, at which point apply adopts the file. Add a test asserting exactly this sequence.

- [ ] **Step 4: Run** — `cargo test -p chronacle-vault`. Expected: PASS.

- [ ] **Step 5: Integration test** in `apps/desktop/src-tauri/tests/vault_inbound.rs`:

```rust
#[tokio::test]
async fn conflict_freezes_then_sidecar_deletion_resolves_to_the_file_version() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let path = dir.path()
        .join("campaigns/sov/entities/npc").read_dir().unwrap()
        .next().unwrap().unwrap().path();

    // Diverge BOTH sides: edit the file, and edit the DB notes.
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nVault-side edit.\n")).unwrap();
    db.query("UPDATE npc:n1 SET notes = 'App-side edit.'").await.unwrap().check().unwrap();

    let report = svc.reconcile().await.expect("conflict pass");
    assert_eq!(report.conflicts, 1);
    let sidecar = path.with_file_name(
        format!("{}.conflict.md", path.file_stem().unwrap().to_str().unwrap()),
    );
    assert!(sidecar.exists(), "DB version preserved in the sidecar");
    assert!(std::fs::read_to_string(&sidecar).unwrap().contains("App-side edit."));

    // Frozen: another pass changes nothing, file untouched.
    let report = svc.reconcile().await.expect("frozen pass");
    assert_eq!(report.conflicts, 1);
    assert!(std::fs::read_to_string(&path).unwrap().contains("Vault-side edit."));

    // GM resolves by deleting the sidecar.
    std::fs::remove_file(&sidecar).unwrap();
    let report = svc.reconcile().await.expect("resolution pass");
    assert_eq!(report.resolved, 1);

    #[derive(serde::Deserialize)]
    struct Row { notes: Option<String> }
    let mut resp = db.query("SELECT notes FROM npc:n1").await.unwrap().check().unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0].notes.as_deref().unwrap().contains("Vault-side edit."));
}
```

- [ ] **Step 6: Run everything** — `cargo test --workspace`. Expected: PASS.

- [ ] **Step 7: Commit** — `git commit -am "feat(vault): conflict sidecar lifecycle, deletion resolves"`

---

### Task 5 (E5): IPC surface — soft_delete_entity, collection-scoped create, list_vault_conflicts, I1, re-embed applied refs

**Files:**
- Modify: `crates/chronacle-extraction/src/entity_service/crud/write.rs` (add `soft_delete`)
- Modify: `crates/chronacle-vault/src/reconcile.rs` (`conflicts()` + `VaultConflict`)
- Modify: `apps/desktop/src-tauri/src/commands/entity_commands.rs`, `vault_commands.rs`, `extraction_commands.rs`, `apps/desktop/src-tauri/src/lib.rs` (register commands)
- Test: service tests in `write.rs`; command-shape tests as in existing `entity_commands.rs` tests; integration in `vault_inbound.rs`

**Interfaces:**
- Produces: `entity_service::soft_delete(db, id, kind) -> Result<(), EntityError>`; commands `soft_delete_entity(id, kind)`, `list_vault_conflicts() -> Vec<VaultConflictDto>`; `create_entity(campaign_id: Option<String>, collection_id: Option<String>, kind, input)`; `VaultSyncService::conflicts() -> Vec<VaultConflict>`; app-layer helper `embed_applied_refs(state, refs)`.
- Consumes: `entity_service::{get_by_id, embed_node}`, `list_synced()`, `load()`, `sidecar_key()`.

- [ ] **Step 1: `entity_service::soft_delete`** (TDD in `write.rs` — test then impl):

```rust
/// Soft-delete: hide the entity from the app and the vault without destroying
/// it. `delete` (hard) remains for genuine destruction.
pub async fn soft_delete<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: EntityKind,
) -> Result<(), EntityError> {
    let mut response = db
        .query(
            "UPDATE type::thing($table, $id) SET \
                 vault_deleted = true, updated_at = time::now() RETURN AFTER",
        )
        .bind(("table", kind.table_name()))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    let rows: Vec<serde_json::Value> = response
        .take(0)
        .map_err(|e| EntityError::Database { message: e.to_string() })?;
    if rows.is_empty() {
        return Err(EntityError::NotFound { id: id.to_string() });
    }
    Ok(())
}
```

Test: create an npc via `create`, `soft_delete` it, assert `get_by_campaign` no longer returns it (get_by_* read paths must filter `vault_deleted != true` — if any entity read query lacks that filter, ADD it in this task; grep `crates/chronacle-extraction/src/entity_service/` for `FROM type::table` / `FROM npc` and check each).

- [ ] **Step 2: Commands** in `entity_commands.rs`:

```rust
#[tauri::command]
pub async fn create_entity(
    state: State<'_, Arc<AppState>>,
    campaign_id: Option<String>,
    collection_id: Option<String>,
    kind: String,
    input: EntityInput,
) -> Result<GraphNode, EntityError> {
    let k = parse_kind(&kind)?;
    if campaign_id.is_some() == collection_id.is_some() {
        return Err(EntityError::Validation {
            field: "scope".to_string(),
            message: "Exactly one of campaignId or collectionId is required".to_string(),
        });
    }
    let outbound = state.outbound.read().await.clone();
    let node = entity_service::create(
        &state.db, campaign_id.as_deref(), collection_id.as_deref(), k, input,
    ).await?;
    outbound.enqueue(chronacle_core::VaultRef {
        table: node.kind.clone(), id: node.id.clone(),
    });
    embed_after_save(&state, &node).await;
    Ok(node)
}

/// Soft-delete: the record disappears from the app and (via the next
/// reconcile's orphan sweep) from the vault. Hard delete remains `delete_entity`.
#[tauri::command]
pub async fn soft_delete_entity(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
) -> Result<(), EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::soft_delete(&state.db, &id, k).await?;
    // Latency: sweep the vault file now instead of waiting for the next sync.
    if let Some(svc) = state.vault.read().await.as_ref().map(Arc::clone) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = svc.reconcile().await {
                eprintln!("vault: post-soft-delete reconcile failed: {e}");
            }
        });
    }
    Ok(())
}
```

(When E6 lands, `state.vault` holds `VaultRuntime`; the `map(Arc::clone)` becomes `map(|rt| Arc::clone(&rt.svc))` — E6 owns that mechanical fix.)

- [ ] **Step 3: `VaultSyncService::conflicts()` + command** — in `reconcile.rs`:

```rust
/// A frozen conflict, shaped for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultConflict {
    pub vref: VaultRef,
    pub name: String,
    pub key: chronacle_core::VaultKey,
    pub sidecar_key: chronacle_core::VaultKey,
}

impl VaultSyncService {
    /// Every record currently frozen in conflict, with display names resolved.
    pub async fn conflicts(&self) -> Result<Vec<VaultConflict>, VaultError> {
        let mut out = Vec::new();
        for row in self.records.list_synced().await? {
            if !row.conflict {
                continue;
            }
            let name = match self.records.load(&row.vref).await? {
                Some(VaultRecord::Entity(e)) => e.name,
                Some(VaultRecord::Session(s)) => s.title,
                Some(VaultRecord::RuleEntry(r)) => r.name,
                None => row.vref.to_thing(),
            };
            out.push(VaultConflict {
                sidecar_key: crate::keys::sidecar_key(&row.key),
                vref: row.vref,
                name,
                key: row.key,
            });
        }
        Ok(out)
    }
}
```

In `vault_commands.rs`:

```rust
/// One frozen conflict, for the settings list and record-editor banners.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultConflictDto {
    pub id: String,       // bare id
    pub kind: String,     // table
    pub name: String,
    pub key: String,
    pub sidecar_key: String,
}

/// Every record currently frozen in conflict. Empty when no vault is configured.
#[tauri::command]
pub async fn list_vault_conflicts(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<VaultConflictDto>, String> {
    let guard = state.vault.read().await;
    let Some(svc) = guard.as_ref() else { return Ok(vec![]) };
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
```

Register `soft_delete_entity` and `list_vault_conflicts` in the `invoke_handler` list in `lib.rs`.

- [ ] **Step 4: Re-embed applied refs + I1**

In `vault_commands.rs`, a shared helper, and use it in `vault_sync_now`:

```rust
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
                    Err(_) => return,
                };
                if let Err(e) = chronacle_extraction::entity_service::embed_node(
                    &state.db, &provider, &node,
                ).await {
                    eprintln!("vault: re-embed of {} failed: {e}", vref.to_thing());
                }
            }
            Err(e) => eprintln!("vault: load for re-embed of {} failed: {e}", vref.to_thing()),
        }
    }
}
```

(`kind_of_table` is the inverse of `parse_kind` — add it next to `parse_kind` in `entity_commands.rs` as `pub(crate) fn kind_of_table(table: &str) -> Result<EntityKind, EntityError> { parse_kind(table) }` — table names equal the serde kind strings.) `vault_sync_now` becomes:

```rust
    let report = svc.reconcile().await.map_err(|e| e.to_string())?;
    embed_applied_refs(&state, &report.applied_refs).await;
    Ok(report.into())
```

**I1** — in `extraction_commands.rs::extract_all_from_campaign`, at the end of the spawned extraction task (immediately after the final completion event is emitted, inside the same `async move` block that owns the cloned state handle):

```rust
            // I1: bulk extraction bypasses the outbound queue (NoopOutbound in
            // persist_batch); one trailing reconcile brings the vault current.
            if let Some(svc) = state_for_task.vault.read().await.as_ref().map(Arc::clone) {
                if let Err(e) = svc.reconcile().await {
                    eprintln!("vault: post-extraction reconcile failed: {e}");
                }
            }
```

(Adapt the state-handle variable name to what the function already clones for the task.)

- [ ] **Step 5: Run the gate** — `cargo test --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings`. Expected: PASS. Frontend callers of `create_entity` keep passing `campaignId` (an `Option<String>` accepts it); no frontend change needed until E7.

- [ ] **Step 6: Commit** — `git commit -am "feat(vault): soft-delete + conflict-list IPC, post-extract sync"`

---

### Task 6 (E6): NotifyWatcher + app wiring (VaultRuntime)

**Files:**
- Create: `crates/chronacle-providers/src/vault_watcher.rs` (+ `pub mod vault_watcher;` in `lib.rs`)
- Modify: `crates/chronacle-vault/src/reconcile.rs` (`is_own_write`)
- Modify: `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src-tauri/src/commands/vault_commands.rs`
- Test: `#[cfg(test)]` in `vault_watcher.rs` with `tempfile::TempDir` (real fs — providers may touch the filesystem; the engine may not)

**Interfaces:**
- Produces: `NotifyWatcher::new(root: impl Into<PathBuf>) -> Self` implementing `VaultWatcher`; `VaultSyncService::is_own_write(&self, key)`; `VaultRuntime` (Shared interfaces block); `spawn_watcher(state: Arc<AppState>, watcher: NotifyWatcher, svc, ...) -> JoinHandle<()>`.
- Consumes: `VaultEvent` (`chronacle-core`), `PendingWrites` via the service, `embed_applied_refs` (E5).

- [ ] **Step 1: `is_own_write`** on `VaultSyncService` (+ mockall unit test: armed hash → true; different content → false; missing file → false):

```rust
    /// Whether the current content of `key` is a write this process made
    /// (consults the shared `PendingWrites` guard). The watcher drops such
    /// events instead of triggering a reconcile.
    pub async fn is_own_write(&self, key: &str) -> bool {
        match self.store.read(key).await {
            Ok(content) => self.pending.matches(key, content_hash(&content)),
            Err(_) => false,
        }
    }
```

- [ ] **Step 2: `NotifyWatcher`** in `crates/chronacle-providers/src/vault_watcher.rs`:

```rust
//! `VaultWatcher` over the `notify` crate (ADR-008 / tranche 5).
//!
//! Dumb by design: maps fs events to vault keys, debounces bursts, and emits
//! `VaultEvent`s. It does NOT decide anything — self-write filtering happens in
//! the consumer via `VaultSyncService::is_own_write`, and every materialization
//! happens in `reconcile()`. A dropped event degrades to "handled on the next
//! reconcile", never to wrong data.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use chronacle_core::{VaultEvent, VaultWatcher};
use notify::{RecursiveMode, Watcher};

/// Filesystem watcher for a local vault root.
pub struct NotifyWatcher {
    root: PathBuf,
    debounce: Duration,
}

impl NotifyWatcher {
    /// Default quiet window between an fs burst and the flush.
    pub const DEBOUNCE: Duration = Duration::from_secs(2);

    /// Watch `root` recursively with the default debounce.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), debounce: Self::DEBOUNCE }
    }

    /// Test seam: a shorter debounce keeps the integration tests fast.
    pub fn with_debounce(root: impl Into<PathBuf>, debounce: Duration) -> Self {
        Self { root: root.into(), debounce }
    }

    /// Map an OS path inside the vault to a POSIX-style key. Non-`.md` paths
    /// and paths outside the root return `None`.
    fn key_of(root: &Path, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(root).ok()?;
        let key = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        key.ends_with(".md").then_some(key)
    }
}

#[async_trait]
impl VaultWatcher for NotifyWatcher {
    async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<VaultEvent> {
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let (raw_tx, mut raw_rx) =
            tokio::sync::mpsc::unbounded_channel::<Result<notify::Event, notify::Error>>();
        let root = self.root.clone();
        let debounce = self.debounce;

        tokio::spawn(async move {
            // The watcher must stay alive for the task's lifetime; notify's
            // callback runs on its own thread, and unbounded_send is sync-safe.
            let mut watcher = match notify::recommended_watcher(move |res| {
                let _ = raw_tx.send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("vault: watcher init failed: {e}");
                    let _ = tx.send(VaultEvent::Rescan).await;
                    return;
                }
            };
            if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
                eprintln!("vault: watch of {} failed: {e}", root.display());
                let _ = tx.send(VaultEvent::Rescan).await;
                return;
            }

            let mut pending: Vec<VaultEvent> = Vec::new();
            loop {
                // Wait for the first event of a burst…
                let Some(first) = raw_rx.recv().await else { break };
                collect(&root, first, &mut pending);
                // …then absorb the burst until a quiet window elapses.
                loop {
                    match tokio::time::timeout(debounce, raw_rx.recv()).await {
                        Ok(Some(ev)) => collect(&root, ev, &mut pending),
                        Ok(None) => return, // channel closed
                        Err(_elapsed) => break,
                    }
                }
                pending.sort_unstable_by(event_order);
                pending.dedup();
                for ev in pending.drain(..) {
                    if tx.send(ev).await.is_err() {
                        return; // consumer dropped; stop watching
                    }
                }
            }
        });
        rx
    }
}

/// Fold one raw notify result into the pending batch.
fn collect(root: &Path, res: Result<notify::Event, notify::Error>, out: &mut Vec<VaultEvent>) {
    let event = match res {
        Ok(e) => e,
        Err(e) => {
            eprintln!("vault: watcher error: {e}");
            out.push(VaultEvent::Rescan);
            return;
        }
    };
    use notify::EventKind;
    for path in &event.paths {
        let Some(key) = NotifyWatcher::key_of(root, path) else { continue };
        match event.kind {
            EventKind::Remove(_) => out.push(VaultEvent::Remove(key)),
            EventKind::Create(_) | EventKind::Modify(_) => out.push(VaultEvent::Upsert(key)),
            EventKind::Any | EventKind::Other => out.push(VaultEvent::Rescan),
            EventKind::Access(_) => {}
        }
    }
}

/// Stable ordering so `dedup` collapses repeats within a batch.
fn event_order(a: &VaultEvent, b: &VaultEvent) -> std::cmp::Ordering {
    fn rank(e: &VaultEvent) -> (u8, &str) {
        match e {
            VaultEvent::Rescan => (0, ""),
            VaultEvent::Upsert(k) => (1, k),
            VaultEvent::Remove(k) => (2, k),
        }
    }
    rank(a).cmp(&rank(b))
}
```

(A rename arrives as `EventKind::Modify(ModifyKind::Name(..))` with one or two paths — both paths map through `key_of`, producing an Upsert for each; the stale one resolves at reconcile via the index. That is correct-by-reconcile; no special casing.)

- [ ] **Step 3: Watcher integration tests** (same file, `#[cfg(test)]`, real `TempDir`, `with_debounce(.., Duration::from_millis(100))`):

```rust
    #[tokio::test]
    async fn a_created_md_file_produces_an_upsert_with_a_posix_key() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(100));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await; // watcher warm-up

        let sub = dir.path().join("campaigns/c/entities/npc");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.md"), "hello").unwrap();

        let ev = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await.expect("event within 5s").expect("open channel");
        assert_eq!(ev, VaultEvent::Upsert("campaigns/c/entities/npc/a.md".into()));
    }

    #[tokio::test]
    async fn a_non_md_file_produces_no_event() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(100));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        std::fs::write(dir.path().join("workspace.json"), "{}").unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(700), rx.recv()).await.is_err(),
            "no event for non-md files"
        );
    }

    #[tokio::test]
    async fn a_burst_of_writes_coalesces_into_one_batch() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = NotifyWatcher::with_debounce(dir.path(), Duration::from_millis(200));
        let mut rx = w.subscribe().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        for _ in 0..5 {
            std::fs::write(dir.path().join("a.md"), "x").unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let ev = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await.expect("event").expect("open");
        assert_eq!(ev, VaultEvent::Upsert("a.md".into()));
        // The dedup collapsed the burst; nothing else arrives promptly.
        assert!(tokio::time::timeout(Duration::from_millis(500), rx.recv()).await.is_err());
    }
```

Mark these `#[cfg_attr(any(target_os = "linux", target_os = "macos"), tokio::test)]`-style only if CI flakes demand it — start with plain `#[tokio::test]`; fs-event timing needs the generous 5s timeouts shown.

- [ ] **Step 4: `VaultRuntime` + consumer task in `src-tauri`**

`apps/desktop/src-tauri/src/lib.rs`:

```rust
/// Everything a live vault configuration owns. Replaced wholesale on
/// `set_vault_path`; the watcher task is aborted when dropped out.
pub struct VaultRuntime {
    pub svc: Arc<chronacle_vault::reconcile::VaultSyncService>,
    pub pending: Arc<chronacle_vault::outbound::PendingWrites>,
    pub watcher_task: Option<tauri::async_runtime::JoinHandle<()>>,
}

/// Consume watcher events: drop our own writes, trigger one reconcile per
/// surviving batch (single in-flight by construction — this loop is the only
/// caller and awaits the reconcile), re-embed applied entities.
pub(crate) fn spawn_watcher(
    state: Arc<AppState>,
    svc: Arc<chronacle_vault::reconcile::VaultSyncService>,
    root: String,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let watcher = chronacle_providers::vault_watcher::NotifyWatcher::new(&root);
        let mut rx = chronacle_core::VaultWatcher::subscribe(&watcher).await;
        while let Some(first) = rx.recv().await {
            let mut batch = vec![first];
            while let Ok(next) = rx.try_recv() {
                batch.push(next);
            }
            let mut relevant = false;
            for ev in &batch {
                match ev {
                    chronacle_core::VaultEvent::Upsert(key) => {
                        if !svc.is_own_write(key).await {
                            relevant = true;
                        }
                    }
                    chronacle_core::VaultEvent::Remove(_)
                    | chronacle_core::VaultEvent::Rescan => relevant = true,
                }
            }
            if !relevant {
                continue;
            }
            match svc.reconcile().await {
                Ok(report) => {
                    crate::commands::vault_commands::embed_applied_refs(
                        &state, &report.applied_refs,
                    ).await;
                }
                Err(e) => eprintln!("vault: watcher-triggered reconcile failed: {e}"),
            }
        }
    })
}
```

Change `AppState.vault` to `tokio::sync::RwLock<Option<VaultRuntime>>`. Update every reader: `vault_sync_now` / `list_vault_conflicts` / `soft_delete_entity` / I1 use `guard.as_ref().map(|rt| Arc::clone(&rt.svc))`. Startup builds the runtime (watcher spawned **after** `AppState` exists — do it inside `.setup()`, where `state.clone()` is available, storing the handle back into `state.vault.write().await`). `set_vault_path` aborts the old watcher (`if let Some(rt) = guard.take() { if let Some(t) = rt.watcher_task { t.abort(); } }`), then installs the new `VaultRuntime` with a fresh `spawn_watcher(Arc::clone(&*state), Arc::clone(&svc), path.clone())`.

- [ ] **Step 5: End-to-end integration test** (`vault_inbound.rs`) — watcher → reconcile without calling reconcile by hand:

```rust
#[tokio::test]
async fn a_vault_edit_flows_into_the_db_via_the_watcher() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");

    let watcher = chronacle_providers::vault_watcher::NotifyWatcher::with_debounce(
        dir.path(), std::time::Duration::from_millis(100),
    );
    let mut rx = chronacle_core::VaultWatcher::subscribe(&watcher).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let path = dir.path()
        .join("campaigns/sov/entities/npc").read_dir().unwrap()
        .next().unwrap().unwrap().path();
    let content = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{content}\nWatcher-driven edit.\n")).unwrap();

    // Mimic the consumer loop: event -> not our write -> reconcile.
    let ev = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await.expect("event").expect("open");
    let chronacle_core::VaultEvent::Upsert(key) = ev else { panic!("expected upsert") };
    assert!(!svc.is_own_write(&key).await, "a GM edit is not our write");
    let report = svc.reconcile().await.expect("reconcile");
    assert_eq!(report.applied, 1);
}

#[tokio::test]
async fn our_own_export_is_recognised_by_the_guard() {
    let db = db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let svc = svc_for(&db, dir.path());
    svc.reconcile().await.expect("export");
    // Every file reconcile just wrote must match the armed guard.
    let key = "campaigns/sov/entities/npc/seraphina.md";
    assert!(svc.is_own_write(key).await, "export must arm the guard (E1)");
}
```

(Adjust the literal key to the seeded name's slug; keep seed and assertion in agreement.)

- [ ] **Step 6: Full gate** — all Rust checks + `pnpm -C apps/desktop typecheck` (the DTO changed in E3; regenerate/adjust `apps/desktop/src/lib` invoke wrapper types for the report if they exist — grep `deferred_apply` in `apps/desktop/src/`). Expected: PASS.

- [ ] **Step 7: Commit** — `git commit -am "feat(vault): notify watcher, runtime wiring, self-write filter"`

---

### Task 7 (E7): Frontend — conflict list, record banner, hints, soft-delete wiring

**Files:**
- Modify: `apps/desktop/src/components/VaultSyncSettings.svelte` (+ `.test.ts`)
- Modify: `apps/desktop/src/components/EntityForm.svelte` (+ `.test.ts`) — banner
- Modify: `apps/desktop/src/components/EntityManager.svelte` (+ `.test.ts`) — delete → `soft_delete_entity`
- Modify: the invoke-wrapper module under `apps/desktop/src/lib/` (add `listVaultConflicts`, `softDeleteEntity`; follow its existing per-command pattern)

**Interfaces:**
- Consumes: `list_vault_conflicts` → `Array<{ id: string; kind: string; name: string; key: string; sidecarKey: string }>`; `soft_delete_entity({ id, kind })`; the reshaped reconcile report (`applied`, `conflicts`, `resolved`, `softDeleted`, `swept`, `invalid` — check the DTO serialization casing in `vault_commands.rs`: the Dto derives plain `Serialize`, snake_case on the wire, matching the existing frontend's handling of `ReconcileReportDto`; keep whatever casing the existing component already consumes).
- Produces: UI only.

All components: Svelte 5 runes, Prettier/ESLint conventions, and copy exactly as written below (copy is part of the spec).

- [ ] **Step 1: Vitest first** — `VaultSyncSettings.test.ts` gains (msw/mocked `invoke` per the file's existing pattern):

```ts
it('lists each conflicted record with its resolution hint', async () => {
  mockInvoke('list_vault_conflicts', [
    {
      id: 'n1',
      kind: 'npc',
      name: 'Seraphina Aldric',
      key: 'campaigns/sov/entities/npc/seraphina-aldric.md',
      sidecarKey: 'campaigns/sov/entities/npc/seraphina-aldric.conflict.md',
    },
  ]);
  render(VaultSyncSettings);
  expect(await screen.findByText('Seraphina Aldric')).toBeInTheDocument();
  expect(screen.getByText(/seraphina-aldric\.conflict\.md/)).toBeInTheDocument();
  expect(
    screen.getByText(/delete the \.conflict\.md file/i),
  ).toBeInTheDocument();
});

it('shows no conflict section when there are none', async () => {
  mockInvoke('list_vault_conflicts', []);
  render(VaultSyncSettings);
  expect(screen.queryByText(/conflict/i)).not.toBeInTheDocument();
});
```

- [ ] **Step 2: `VaultSyncSettings.svelte`** — add a conflicts block (shape it to the component's existing markup/classes):
  - `let conflicts = $state<VaultConflict[]>([])`, loaded on mount and re-loaded after every "Sync now" completes and after `set_vault_path`.
  - A badge next to the section heading: `Conflicts ({conflicts.length})`, rendered only when `conflicts.length > 0`.
  - One row per conflict: **name** (bold), kind, the vault file path, the sidecar path, and the hint line: `Merge the two files in your vault, then delete the .conflict.md file — Chronacle applies your version on the next sync.` (one hint line under the list, not per row, is fine if the list is long — pick one and keep the test in agreement).
  - Next to the vault-path input, the explainer: `Changing the folder re-exports everything; nothing is deleted.`
  - In the section help text: `Text inside the marked compiled block is overwritten by Chronacle.`
  - Surface the new report counts in the existing sync-result line (applied / conflicts / resolved / soft-deleted / invalid alongside exported/unchanged).

- [ ] **Step 3: `EntityForm.svelte` banner** — Vitest first:

```ts
it('shows a conflict banner when the open entity is conflicted', async () => {
  mockInvoke('list_vault_conflicts', [
    { id: 'n1', kind: 'npc', name: 'Seraphina', key: 'k.md', sidecarKey: 'k.conflict.md' },
  ]);
  render(EntityForm, { props: propsForEntity({ id: 'n1', kind: 'npc' }) });
  expect(
    await screen.findByText(/unsynced vault edits in conflict/i),
  ).toBeInTheDocument();
});
```

Implementation: on mount (`$effect` keyed by the entity id), call `listVaultConflicts()` and set `let conflict = $state<VaultConflict | null>(null)` when the open record's `id`+`kind` matches. Banner (non-blocking, above the form): `This record has unsynced vault edits in conflict — resolve in your vault ({conflict.sidecarKey}).` When no vault is configured the command returns `[]`, so the banner naturally never shows.

- [ ] **Step 4: Delete becomes soft** — in `EntityManager.svelte`, change the delete action to `softDeleteEntity({ id, kind })`; update its test to assert the new command name. Keep the confirm dialog copy, but change it to: `Remove this entity? It disappears from Chronacle and your vault. (Files you edited by hand in the vault are kept.)`

- [ ] **Step 5: Run** — `pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run`. Expected: PASS.

- [ ] **Step 6: Commit** — `git commit -am "feat(ui): vault conflict list, record banner, soft delete"`

---

### Task 8 (E8): Acceptance scenarios + D-series minors

**Files:**
- Create: `apps/desktop/tests/e2e/features/vault-inbound.feature`
- Create: `apps/desktop/tests/e2e/backend/steps/vault-inbound.steps.ts`
- Modify: `crates/chronacle-vault/src/render.rs` (seam test), `crates/chronacle-core/src/vault.rs` + `crates/chronacle-providers/src/vault_store.rs` (`io::ErrorKind`), `apps/desktop/src/components/VaultSyncSettings.test.ts` (error path)

**Interfaces:** consumes everything above; produces tests only (plus the `VaultStoreError::Io` shape change below).

- [ ] **Step 1: `.feature` file** — `apps/desktop/tests/e2e/features/vault-inbound.feature`:

```gherkin
Feature: Inbound vault sync
  The GM edits campaign files in their Markdown vault; changes flow back into
  Chronacle. Divergent edits become .conflict.md sidecars resolved in the vault.

  Scenario: A vault edit updates the record
    Given a synced vault with an entity "Seraphina Aldric"
    When the GM edits the notes of "Seraphina Aldric" in the vault
    And a sync runs
    Then the entity "Seraphina Aldric" has the edited notes in Chronacle

  Scenario: An edit inside the compiled block is reverted
    Given a synced vault with an entity "Seraphina Aldric"
    When the GM edits inside the compiled block of "Seraphina Aldric"
    And a sync runs
    Then the vault file of "Seraphina Aldric" shows the compiled text again

  Scenario: Divergent edits produce a conflict sidecar
    Given a synced vault with an entity "Seraphina Aldric"
    When both Chronacle and the vault file of "Seraphina Aldric" are edited differently
    And a sync runs
    Then a conflict sidecar exists for "Seraphina Aldric"
    And the vault sync settings list "Seraphina Aldric" as a conflict

  Scenario: Deleting the sidecar resolves the conflict with the vault version
    Given an entity "Seraphina Aldric" frozen in conflict
    When the GM deletes the conflict sidecar
    And a sync runs
    Then the entity "Seraphina Aldric" has the vault version in Chronacle
    And no conflict is listed for "Seraphina Aldric"

  Scenario: Deleting a vault file soft-deletes the record
    Given a synced vault with an entity "Seraphina Aldric"
    When the GM deletes the vault file of "Seraphina Aldric"
    And a sync runs
    Then "Seraphina Aldric" is no longer visible in Chronacle

  Scenario: Switching vault folders deletes nothing
    Given a synced vault with an entity "Seraphina Aldric"
    When the vault path is changed to a new empty folder
    Then "Seraphina Aldric" is still visible in Chronacle
    And the new folder contains a file for "Seraphina Aldric"
```

- [ ] **Step 2: Step definitions** — `apps/desktop/tests/e2e/backend/steps/vault-inbound.steps.ts` following the existing playwright-bdd steps' mocked-IPC pattern (look at the D-series vault steps in the same directory for the fixture/mocking helpers; the backend suite runs against mocked `invoke`, so steps assert the UI's command calls and rendered state, mirroring E7's Vitest at the scenario level). Backend-only invariants that mocked IPC cannot express (the fence revert, the actual DB write) are already covered by the Rust integration tests in `vault_inbound.rs` named after the scenarios — add a `# backend: covered by apps/desktop/src-tauri/tests/vault_inbound.rs` comment above those scenarios, per the ADR-011 convention used in tranche 4.

- [ ] **Step 3: Minor — exact-output seam test** in `render.rs`:

```rust
    /// Locks the exact byte layout of the frontmatter/body seam. A drive-by
    /// whitespace change here would re-hash every synced file as "changed"
    /// and force a spurious full re-export.
    #[test]
    fn rendered_record_layout_is_stable_at_the_seam() {
        use pretty_assertions::assert_eq;
        let record = VaultRecord::Entity(EntityRecord {
            vref: VaultRef { table: "npc".into(), id: "n1".into() },
            name: "Seraphina".into(),
            summary: Some("S.".into()),
            notes: Some("N.".into()),
            codex_article: Some("C.".into()),
            scope: VaultScope::Campaign { id: "campaign:c1".into(), name: "SoV".into() },
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
        });
        let rendered = render_record(&record);
        // Pin the FULL literal: run the test once, paste the actual output
        // here verbatim, and review that it is what the grammar promises
        // (frontmatter fences, one blank line, Summary, fenced article, Notes).
        let expected = "<paste the reviewed render_record output here>";
        assert_eq!(rendered, expected);
    }
```

(The literal is generated, reviewed, then pinned — the value of the test is that any later drive-by change to the seam has to touch this literal consciously.)

- [ ] **Step 4: Minor — preserve `io::ErrorKind`.** In `chronacle-core`:

```rust
    #[error("I/O error ({kind:?}): {message}")]
    Io { kind: std::io::ErrorKind, message: String },
```

Update `LocalFsVaultStore` construction sites (`VaultStoreError::Io(e.to_string())` → `VaultStoreError::Io { kind: e.kind(), message: e.to_string() }`) and any test matching on `Io(..)`. `chronacle-vault` sites that *fabricate* an Io error (tests) use `ErrorKind::Other`.

- [ ] **Step 5: Minor — `VaultSyncSettings` error path** Vitest: mock `vault_sync_now` to reject with `"disk full"`, assert the component renders the error state (and does not clear the previous report). Follow the component's existing error rendering.

- [ ] **Step 6: Full gate** including `pnpm -C apps/desktop run e2e:backend`. Expected: PASS.

- [ ] **Step 7: Commit** — `git commit -am "test(vault): inbound acceptance scenarios + D-series minors"`

---

### Task 9 (E9): GM-facing user guide — "Your Vault"

**Files:**
- Modify: `docs/user-guide.md` (new chapter)

Dispatch the **`user-guide-writer`** subagent for the prose. It must cover, for non-technical readers, in this order:

1. **What vault sync is** — your campaign as ordinary Markdown files in a folder you choose; open it in Obsidian or any editor; changes flow both ways; Chronacle keeps them in step automatically (within a couple of seconds) and on every "Sync now".
2. **What's yours vs Chronacle's in a file** — a worked example file (an NPC) with callouts: the metadata block at the top and the marked "compiled" section belong to Chronacle and get rewritten; **Summary** and **Notes** (and anything else you write) are yours and sync back.
3. **Conflicts** — why they happen (the same record edited in both places between syncs), what a `.conflict.md` file is (Chronacle's version, saved next to yours so nothing is lost), and the exact walkthrough: open both files side by side → merge what you want into *your* file → delete the `.conflict.md` file → Chronacle applies your file on the next sync. Mention the conflict list in Settings → Vault Sync and the banner on the record.
4. **Deleting** — deleting a vault file hides the record in Chronacle (it is not destroyed); deleting a record in Chronacle removes its vault file unless you had edited it by hand since.
5. **Switching folders** — pointing Chronacle at a new folder re-exports everything and never deletes anything.

- [ ] **Step 1:** Write the chapter (subagent), keeping terminology identical to the UI copy shipped in E7 (`.conflict.md`, "Sync now", "compiled block").
- [ ] **Step 2:** `pnpm -C apps/desktop exec prettier --check ../../docs/user-guide.md` (or the repo's root prettier invocation) — the pre-commit hook formats Markdown; keep it clean.
- [ ] **Step 3: Commit** — `git commit -am "docs: GM guide for vault sync and conflict resolution"`

---

## Execution notes

- **PR mapping:** one task = one PR, stacked in order E1→E9. E3+E4 may merge into a single PR if the reviewer prefers seeing conflict handling with its report reshape; default is separate.
- **Verify the loop end-to-end before calling the tranche done:** run the real app (`pnpm -C apps/desktop tauri dev`), point it at a scratch vault, edit a file in another editor, and watch the DB update; then force a conflict and resolve it by deleting the sidecar. The UI E2E Docker image (`docker build -f apps/desktop/tests/e2e/ui/Dockerfile -t chronacle-e2e .`) is available for the tauri-driver path.
- **Update `docs/architecture.md`** if it documents the vault sync flow (add the inbound direction + conflict lifecycle to the ADR-008 section) — fold into E9.
- **Handover:** after E9, write `docs/superpowers/tranche-6-handover.md` if any items are carried (candidate: undelete/trash UI, in-app conflict resolution UI, id-less file creation).
