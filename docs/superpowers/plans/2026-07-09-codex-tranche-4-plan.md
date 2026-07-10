# Codex Tranche 4 (D0, D1a, D1b, D2a, D2b, D3a, D3b, D4a, D4b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export the compiled codex — entity articles, rule entries, GM notes, and sessions — as an Obsidian-compatible Markdown vault, kept current by a content-hash reconcile and a non-blocking outbound queue.

**Architecture:** Follows `docs/superpowers/specs/2026-07-09-codex-vault-sync-design.md`. A new backend-agnostic engine crate `chronacle-vault` owns markdown rendering, key mapping, and the three-way sync decision. It talks to four ports declared in `chronacle-core`: `VaultStore` (keyed blob I/O), `VaultWatcher` (change events, unused this tranche), `VaultOutbound` (one-method enqueue), and `VaultRecordStore` (record access). `chronacle-providers` supplies `LocalFsVaultStore`; `chronacle-domain` supplies `SurrealVaultRecordStore`; `apps/desktop` is the composition root. **This tranche implements the export direction only** — the `apply`, `conflict`, and `soft_delete` branches of the decision table are computed and logged but not acted on. Inbound is tranche 5.

**Tech Stack:** Rust (SurrealDB embedded, tokio, `yaml_serde`, `mockall`), Svelte 5 runes + TypeScript, Vitest, playwright-bdd.

## Global Constraints

- Every branch: `git checkout --no-track -b <branch> <base>` — never track main. Stacked chain: `chore/d0-vault-crate` from `main`, then `d1a` ← `d0`, `d1b` ← `d1a`, `d2a` ← `d1b`, `d2b` ← `d2a`, `d3a` ← `d2b`, `d3b` ← `d3a`, `d4a` ← `d3b`, `d4b` ← `d4a`. After an upstream PR merges, rebase the stack (`git rebase --onto main <old-parent> <branch>`), force-push with `--force-with-lease`, and retarget the next PR's base to `main`.
- **First push of a stacked branch needs an explicit refspec:** `git push -u origin <branch>:refs/heads/<branch>`. `push.default=upstream` otherwise pushes onto the parent PR's head and silently corrupts it.
- **Two new crates are added in D0 and nowhere else:** `yaml_serde = "0.10"` (in `chronacle-vault`) and `notify = "8"` (in `chronacle-providers`). Both are verified green against `cargo deny check`. No other new Cargo or npm dependency anywhere in this tranche. In particular **do not add a hashing crate** — the content hash is `std::hash::DefaultHasher`, a loop/merge guard, not a security primitive.
- Commit subjects ≤ 72 chars, imperative, conventional prefixes; never `--no-verify`.
- Clippy warnings are errors (`cargo clippy --workspace --all-targets --all-features -- -D warnings`); public items in library crates need `///` docs; Svelte 5 runes only (`$state`, `$derived`, `$props`) — no `export let` / `$:`.
- BDD (ADR-011): UI-reachable scenarios ship as `.feature` files (D3b, D4b); backend-only scenarios ship as Rust tests named to mirror the Gherkin (D2b, D3a, D4a), per `apps/desktop/tests/e2e/features/README.md`.
- **Filesystem access never appears in engine logic.** `chronacle-vault` must not depend on `tokio::fs`, `std::fs`, or `notify`. If an engine module needs to touch a file, it goes through `Arc<dyn VaultStore>`. A `use std::fs` anywhere under `crates/chronacle-vault/` is a review rejection.
- **Sync is content-hash based, never timestamp based.** `codex_service::compile.rs:220-224` updates `codex_article` without touching `updated_at`, so any comparison of `mtime` against `updated_at` is wrong by construction. `mtime` may be used only as a read-skipping optimisation, never as a decision input.
- **`vault_deleted` is queried as `!= true`, never `= false`.** `DEFINE FIELD … DEFAULT false` does not backfill rows written before the migration; those rows have no value at all and a `= false` filter silently omits them. Same class of bug as the `count()` + `GROUP ALL` case already in this repo's history.
- **FLEXIBLE object binding:** never bind a `serde_json::Value` when _writing_ to a FLEXIBLE `object` / `array<object>` field — nested keys are lost. Write via plain `#[derive(Serialize)]` structs or inline SurrealQL object literals.
- **Migrations are DEFINE-only and re-run on every boot.** `run_migrations` (`crates/chronacle-db/src/schema/mod.rs`) globs `src/schema/*.surql`, sorts by filename, and executes each. Use `DEFINE … OVERWRITE`; never `REMOVE` (a `REMOVE TABLE` once wiped every `relates_to` edge on restart).
- Embedding only via `Arc<dyn EmbeddingProvider>`; `embed_model` identity is preserved on any re-index.
- Frontend `invoke()` argument keys are camelCase (`vaultPath`); Tauri maps them to snake_case Rust parameters. Struct arguments need `#[serde(rename_all = "camelCase")]`.
- Each PR ends green on the full CI gate: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && cargo deny check && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend`. `cargo deny check` is time-dependent and easy to forget — run it every time.

## Out of scope (deliberately, → tranche 5)

Inbound apply, the `notify` watcher loop, id-less file creation, relocation and `vault_type_mismatch`, soft-delete and the `!= true` read-path filters across extraction/retrieval/domain, `.conflict.<ts>.md` materialisation, the conflict card and restore-or-confirm UI, and the GM-facing user guide. `VaultWatcher` is **declared** in D0 and left unimplemented; `LocalFsVaultStore` ships without a watcher and the service runs reconcile-only.

## Shared interfaces introduced by this tranche (single source of truth)

```rust
// crates/chronacle-core/src/vault.rs                                       (D0)

/// A record's stable identity: table name + raw id, e.g. ("npc", "abc123").
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VaultRef { pub table: String, pub id: String }
impl VaultRef {
    pub fn parse(thing: &str) -> Option<VaultRef>;   // "npc:abc123" -> VaultRef
    pub fn to_thing(&self) -> String;                // -> "npc:abc123"
}

/// A vault key: a `/`-separated, POSIX-style path relative to the vault root.
/// Never an OS path. `LocalFsVaultStore` is the only thing that joins it to one.
pub type VaultKey = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultMetadata { pub mtime: std::time::SystemTime }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultEvent { Upsert(VaultKey), Remove(VaultKey), Rescan }

#[derive(Debug, thiserror::Error)]
pub enum VaultStoreError {
    #[error("I/O error: {0}")] Io(String),
    #[error("Not found: {0}")] NotFound(VaultKey),
    #[error("Invalid key: {0}")] InvalidKey(VaultKey),
}

#[async_trait::async_trait]
pub trait VaultStore: Send + Sync {
    async fn read(&self, key: &str) -> Result<String, VaultStoreError>;
    async fn write(&self, key: &str, content: &str) -> Result<(), VaultStoreError>;
    async fn delete(&self, key: &str) -> Result<(), VaultStoreError>;
    /// Recursive. Returns keys (not OS paths) under `prefix`, `.md` files only.
    async fn list(&self, prefix: &str) -> Result<Vec<VaultKey>, VaultStoreError>;
    async fn metadata(&self, key: &str) -> Result<VaultMetadata, VaultStoreError>;
}

#[async_trait::async_trait]
pub trait VaultWatcher: Send + Sync {
    /// Tranche 5. Declared here so the service signature is stable.
    async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<VaultEvent>;
}

/// Fire-and-forget. Producers depend on this and nothing else vault-shaped.
pub trait VaultOutbound: Send + Sync {
    fn enqueue(&self, target: VaultRef);
}

/// A no-op used wherever vault sync is disabled. Keeps producers `Option`-free.
pub struct NoopOutbound;
impl VaultOutbound for NoopOutbound { fn enqueue(&self, _: VaultRef) {} }

/// The three record shapes the vault mirrors. One enum, not five method families.
#[derive(Debug, Clone, PartialEq)]
pub enum VaultRecord { Entity(EntityRecord), Session(SessionRecord), RuleEntry(RuleEntryRecord) }

#[derive(Debug, Clone, PartialEq)]
pub struct EntityRecord {
    pub vref: VaultRef,            // table == entity kind, e.g. "npc"
    pub name: String,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub codex_article: Option<String>,
    pub scope: VaultScope,
    pub created_at: String,        // RFC3339
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub vref: VaultRef,
    pub session_number: i64,
    pub title: String,
    pub date_played: String,
    pub notes: String,
    pub campaign: VaultScope,      // always VaultScope::Campaign
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleEntryRecord {
    pub vref: VaultRef,
    pub name: String,
    pub category: String,
    pub body: String,
    pub notes: Option<String>,
    pub page_refs: Vec<RulePageRef>,
    pub collection: VaultScope,    // always VaultScope::Collection
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RulePageRef { pub source_name: String, pub page_start: i64, pub page_end: i64 }

/// The owning scope of a record, carrying both id and display name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VaultScope {
    Campaign { id: String, name: String },
    Collection { id: String, name: String },
}

#[derive(Debug, thiserror::Error)]
pub enum VaultRecordError {
    #[error("record store error: {0}")] Backend(String),
    #[error("not found: {0}")] NotFound(String),
}

#[async_trait::async_trait]
pub trait VaultRecordStore: Send + Sync {
    /// Every syncable record, excluding soft-deleted ones (`vault_deleted != true`).
    async fn list_all(&self) -> Result<Vec<VaultRecord>, VaultRecordError>;
    async fn load(&self, vref: &VaultRef) -> Result<Option<VaultRecord>, VaultRecordError>;
    /// Persisted merge base. `None` when the record has never synced.
    async fn get_synced_hash(&self, vref: &VaultRef) -> Result<Option<u64>, VaultRecordError>;
    async fn set_synced_hash(&self, vref: &VaultRef, key: &str, hash: u64)
        -> Result<(), VaultRecordError>;
    async fn clear_synced_hash(&self, vref: &VaultRef) -> Result<(), VaultRecordError>;
}
```

```rust
// crates/chronacle-vault/src/lib.rs                                        (D0–D4)
pub mod frontmatter;   // D1a
pub mod markdown;      // D1a
pub mod keys;          // D1b
pub mod render;        // D3a
pub mod decide;        // D3a
pub mod reconcile;     // D3a
pub mod outbound;      // D4a

// crates/chronacle-vault/src/frontmatter.rs                                (D1a)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Frontmatter { /* ordered map, all scalars emitted quoted */ }
pub fn render(fm: &Frontmatter) -> String;                  // includes `---` fences
pub fn parse(file: &str) -> Result<(Frontmatter, String), FrontmatterError>;  // (fm, body)
#[derive(Debug, thiserror::Error)] pub enum FrontmatterError { /* Missing, Yaml(String) */ }

// crates/chronacle-vault/src/markdown.rs                                   (D1a)
pub const FENCE_START: &str = "<!-- chronacle:codex-article start -- compiled; edits are not applied -->";
pub const FENCE_END:   &str = "<!-- chronacle:codex-article end -->";
pub const SUMMARY_HEADING: &str = "## Summary";
/// Lossless split. Everything outside the fence and outside a leading
/// `## Summary` becomes `notes`, verbatim — including unknown headings.
pub struct BodyParts { pub summary: Option<String>, pub fenced: Option<String>, pub notes: Option<String> }
pub fn split_body(body: &str) -> BodyParts;
pub fn render_body(parts: &BodyParts) -> String;
/// Trim + CRLF→LF. Every comparison in the engine runs on normalized text.
pub fn normalize(s: &str) -> String;

// crates/chronacle-vault/src/keys.rs                                       (D1b)
pub const ENTITY_TYPES: [&str; 8];                       // the eight per-type tables
pub fn slug(name: &str) -> String;                       // lowercase, Unicode-aware, `-` separated; never empty or Windows-reserved
pub fn scope_folder(scope: &VaultScope) -> String;       // "campaigns/<slug>" | "collections/<slug>"
pub fn scope_folder_disambiguated(scope: &VaultScope, collides: bool) -> String;
pub fn key_for(record: &VaultRecord, collides: bool) -> VaultKey;
pub fn is_managed(key: &str) -> bool;                    // true only for the four shapes: campaigns/<slug>/entities/<type>/<file>.md, campaigns/<slug>/sessions/<file>.md, collections/<slug>/entities/<type>/<file>.md, collections/<slug>/rules/<file>.md
pub fn entity_type_of(key: &str) -> Option<&str>;        // the segment after `entities/`
/// id → key map built by scanning the vault. Identity is the frontmatter `id`.
pub struct VaultIndex { /* .. */ }
impl VaultIndex {
    pub async fn scan(store: &dyn VaultStore) -> Result<VaultIndex, VaultError>;
    pub fn key_of(&self, vref: &VaultRef) -> Option<&VaultKey>;
    pub fn contains(&self, vref: &VaultRef) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

// crates/chronacle-vault/src/render.rs                                     (D3a)
pub fn render_record(record: &VaultRecord) -> String;    // full file: frontmatter + body
pub fn content_hash(s: &str) -> u64;                     // DefaultHasher over normalize(s)

// crates/chronacle-vault/src/decide.rs                                     (D3a)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction { NoOp, AdoptBase, Export, Apply, Conflict, SoftDelete }
/// Pure. `file == None` means the key is absent from the vault.
pub fn decide(base: Option<u64>, db: u64, file: Option<u64>) -> SyncAction;

// crates/chronacle-vault/src/reconcile.rs                                  (D3a)
pub struct VaultSyncService { /* store, records, root-relative */ }
impl VaultSyncService {
    pub fn new(store: Arc<dyn VaultStore>, records: Arc<dyn VaultRecordStore>) -> Self;
    /// Export direction only this tranche. Apply/Conflict/SoftDelete log + no-op.
    pub async fn reconcile(&self) -> Result<ReconcileReport, VaultError>;
    pub async fn export_one(&self, vref: &VaultRef) -> Result<(), VaultError>;
}
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    pub exported: usize, pub unchanged: usize, pub adopted: usize,
    pub deferred_apply: usize, pub deferred_conflict: usize, pub deferred_delete: usize,
    pub failed: usize,   // a failing key never aborts the run, and never claims a base
}

// crates/chronacle-vault/src/outbound.rs                                   (D4a)
pub struct QueueOutbound { /* mpsc::UnboundedSender<VaultRef> */ }
impl QueueOutbound { pub fn new() -> (QueueOutbound, tokio::sync::mpsc::UnboundedReceiver<VaultRef>); }
impl VaultOutbound for QueueOutbound { fn enqueue(&self, target: VaultRef); }
pub struct PendingWrites { /* HashMap<VaultKey, (u64 /*hash*/, Instant)> */ }
impl PendingWrites {
    pub const TTL: std::time::Duration = std::time::Duration::from_secs(30);
    pub fn arm(&self, key: &str, hash: u64);
    pub fn arm_at(&self, key: &str, hash: u64, at: std::time::Instant);  // TTL test seam
    /// Content-based, not consume-on-first-match: one write emits several events.
    pub fn matches(&self, key: &str, hash: u64) -> bool;
    pub fn sweep(&self);
}
/// Testable core: `drain_loop` is the thin wrapper that calls `export_one`.
pub async fn drain_loop_with<F>(rx: tokio::sync::mpsc::UnboundedReceiver<VaultRef>, export: F)
where F: Fn(VaultRef) -> Result<(), crate::VaultError> + Send + 'static;
/// Drains the queue, coalescing repeats, and exports each ref exactly once.
pub async fn drain_loop(
    rx: tokio::sync::mpsc::UnboundedReceiver<VaultRef>,
    svc: Arc<VaultSyncService>,
    pending: Arc<PendingWrites>,
);
```

```rust
// crates/chronacle-providers/src/vault_store.rs                            (D2a)
pub struct LocalFsVaultStore { root: std::path::PathBuf }
impl LocalFsVaultStore { pub fn new(root: impl Into<std::path::PathBuf>) -> Self; }
#[async_trait] impl VaultStore for LocalFsVaultStore { /* tokio::fs */ }

// crates/chronacle-domain/src/vault_record_store.rs                        (D2b)
pub struct SurrealVaultRecordStore { db: Surreal<Any> }
impl SurrealVaultRecordStore { pub fn new(db: Surreal<Any>) -> Self; }
#[async_trait] impl VaultRecordStore for SurrealVaultRecordStore { /* SurrealQL */ }
```

```ts
// apps/desktop/src/lib/commands.ts additions                               (D3b)
export interface ReconcileReport {
  exported: number;
  unchanged: number;
  adopted: number;
  deferred_apply: number;
  deferred_conflict: number;
  deferred_delete: number;
}
export function getVaultPath(): Promise<string | null>;
export function setVaultPath(vaultPath: string | null): Promise<void>;
export function vaultSyncNow(): Promise<ReconcileReport>;
```

---

### Task 1: `chronacle-core` vault ports + `chronacle-vault` crate skeleton

**Files:**

- Create: `crates/chronacle-core/src/vault.rs`
- Modify: `crates/chronacle-core/src/lib.rs` (register module, re-export)
- Modify: `crates/chronacle-core/Cargo.toml` (add `mockall` dev-dep for `MockVaultStore`)
- Create: `crates/chronacle-vault/Cargo.toml`
- Create: `crates/chronacle-vault/src/lib.rs`

**Interfaces:**

- Consumes: nothing (this is the base of the stack).
- Produces: every type in the "Shared interfaces" `chronacle-core/src/vault.rs` block above — `VaultRef`, `VaultKey`, `VaultMetadata`, `VaultEvent`, `VaultStoreError`, `VaultStore`, `VaultWatcher`, `VaultOutbound`, `NoopOutbound`, `VaultRecord` + its three variants, `VaultScope`, `RulePageRef`, `VaultRecordError`, `VaultRecordStore`.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b chore/d0-vault-crate main
```

- [ ] **Step 2: Write the failing test for `VaultRef` round-tripping**

`VaultRef` is the only type in this task with behaviour, so it is the only one that gets a test. Put this at the bottom of the new `crates/chronacle-core/src/vault.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_ref_parses_a_thing_string() {
        let r = VaultRef::parse("npc:abc123").expect("parse");
        assert_eq!(r.table, "npc");
        assert_eq!(r.id, "abc123");
    }

    #[test]
    fn vault_ref_round_trips_through_to_thing() {
        let r = VaultRef { table: "rule_entry".into(), id: "xyz".into() };
        assert_eq!(r.to_thing(), "rule_entry:xyz");
        assert_eq!(VaultRef::parse(&r.to_thing()), Some(r));
    }

    #[test]
    fn vault_ref_rejects_a_string_without_a_colon() {
        assert_eq!(VaultRef::parse("npc"), None);
    }

    #[test]
    fn vault_ref_keeps_colons_inside_the_id() {
        // SurrealDB ids may themselves contain a colon; split once, on the first.
        let r = VaultRef::parse("npc:a:b").expect("parse");
        assert_eq!(r.table, "npc");
        assert_eq!(r.id, "a:b");
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p chronacle-core vault_ref`
Expected: FAIL — `cannot find type VaultRef in this scope` (the module does not exist yet).

- [ ] **Step 4: Write `crates/chronacle-core/src/vault.rs`**

Copy the full `chronacle-core/src/vault.rs` block from "Shared interfaces" above verbatim, adding `///` docs to every public item. The two implementations with real bodies:

```rust
impl VaultRef {
    /// Parse a SurrealDB thing string (`"npc:abc123"`) into a `VaultRef`.
    ///
    /// Splits on the *first* colon — SurrealDB record ids may contain colons.
    pub fn parse(thing: &str) -> Option<VaultRef> {
        let (table, id) = thing.split_once(':')?;
        if table.is_empty() || id.is_empty() {
            return None;
        }
        Some(VaultRef { table: table.to_owned(), id: id.to_owned() })
    }

    /// Render back to a SurrealDB thing string.
    pub fn to_thing(&self) -> String {
        format!("{}:{}", self.table, self.id)
    }
}
```

Declare `VaultStore`, `VaultWatcher`, and `VaultRecordStore` with `#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]` above `#[async_trait::async_trait]` so downstream crates get `MockVaultStore` / `MockVaultRecordStore`. Add to `crates/chronacle-core/Cargo.toml`:

```toml
[features]
mocks = ["dep:mockall"]

[dependencies]
mockall = { workspace = true, optional = true }
```

- [ ] **Step 5: Register the module**

In `crates/chronacle-core/src/lib.rs`, after the existing `pub mod vector_store;`:

```rust
pub mod vault;

pub use vault::{
    EntityRecord, NoopOutbound, RuleEntryRecord, RulePageRef, SessionRecord, VaultEvent, VaultKey,
    VaultMetadata, VaultOutbound, VaultRecord, VaultRecordError, VaultRecordStore, VaultRef,
    VaultScope, VaultStore, VaultStoreError, VaultWatcher,
};
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p chronacle-core vault_ref`
Expected: PASS — 4 tests.

- [ ] **Step 7: Create the `chronacle-vault` crate skeleton**

`crates/chronacle-vault/Cargo.toml`. Note what is **absent**: no `tokio::fs`, no `notify`, no `surrealdb`, no `chronacle-domain`, no `chronacle-extraction`.

```toml
[package]
name = "chronacle-vault"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_vault"

[dependencies]
chronacle-core = { path = "../chronacle-core" }
serde.workspace = true
async-trait.workspace = true
thiserror.workspace = true
tokio.workspace = true
yaml_serde = "0.10"

[dev-dependencies]
chronacle-core = { path = "../chronacle-core", features = ["mocks"] }
mockall.workspace = true
pretty_assertions.workspace = true
tokio.workspace = true
```

`crates/chronacle-vault/src/lib.rs`:

```rust
//! Backend-agnostic Markdown vault sync engine (ADR-008).
//!
//! Owns markdown rendering, key mapping, and the three-way sync decision.
//! Reaches storage and records only through the `chronacle-core` ports, so a
//! future S3 / WebDAV backend needs no change here. **This crate must never
//! depend on `std::fs`, `tokio::fs`, or `notify`.**

/// Errors surfaced by the vault engine.
#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("store error: {0}")]
    Store(#[from] chronacle_core::VaultStoreError),
    #[error("record error: {0}")]
    Record(#[from] chronacle_core::VaultRecordError),
    #[error("frontmatter error: {0}")]
    Frontmatter(String),
}
```

- [ ] **Step 8: Verify `notify` and `yaml_serde` clear the dependency gate**

Add `notify = "8"` to `crates/chronacle-providers/Cargo.toml` under `[dependencies]` (it is unused until D2a/tranche 5, so also add `#![allow(unused_crate_dependencies)]`-free usage by deferring the dep to D2a if clippy objects — see Step 9).

Run: `cargo deny check advisories licenses bans`
Expected: `advisories ok, bans ok, licenses ok`

If this fails, **stop and escalate** — do not swap to `serde_yml`, which carries RUSTSEC-2025-0068 (unsound, unpatched) and would fail the `unsound = "workspace"` gate.

- [ ] **Step 9: Build the workspace**

Run: `cargo build --workspace && cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: PASS. `crates/*` is already globbed into `workspace.members`, so no root `Cargo.toml` edit is needed.

If clippy flags `notify` as an unused dependency in `chronacle-providers`, **move the `notify` line to Task 7 (D2a)** rather than silencing the lint.

- [ ] **Step 10: Commit**

```bash
git add crates/chronacle-core crates/chronacle-vault crates/chronacle-providers/Cargo.toml Cargo.lock
git commit -m "feat(vault): core ports + chronacle-vault crate skeleton"
```

---

### Task 2: ADR-008 amendment, architecture tables, AGENTS.md — and the D0 PR

**Files:**

- Modify: `docs/architecture.md` (ADR-008 body ~line 509; "Internal Workspace Crates" table; "Crate & Tool Summary" table ~line 1090)
- Modify: `AGENTS.md` (setting-keys line)

**Interfaces:**

- Consumes: Task 1's crate structure.
- Produces: nothing code-facing. This task exists because ADR-008 currently documents a design that contradicts the code, and every later task's reviewer will read it.

- [ ] **Step 1: Amend ADR-008's Decision section**

In `docs/architecture.md`, replace ADR-008's vault-layout block, its two sync-behaviour tables, its conflict-resolution bullets, its `is_gm_only` paragraph, its Implementation bullets, and its "New migration required" block, to match `docs/superpowers/specs/2026-07-09-codex-vault-sync-design.md`. Specifically, five corrections, each of which should be stated as an explicit amendment note so the history is legible:

1. There is no `entity` table — eight per-type tables. `vault_deleted` is ×9 (`npc`, `location`, `faction`, `creature`, `item`, `event`, `player_character`, `misc`, `session`).
2. The tree is rooted at `campaigns/` **and** `collections/`, mirroring the exclusive `in_campaign` / `in_collection` ownership edges.
3. `is_gm_only` is **not** in the frontmatter and `vault_include_gm_only` is **not** a setting. Both move to Phase 3 alongside AI-detected passage-level GM-secret flags.
4. File I/O goes through the `VaultStore` port, not `tokio::fs` directly. Add `VaultWatcher`, `VaultOutbound`, `VaultRecordStore`.
5. Sync is content-hash based. Delete the "last-write-wins on `updated_at` vs. file mtime" and "delta under 5 seconds" rules; replace with the three-way merge table and `vault_sync_state`.

Leave ADR-008's **Status** as `Proposed`. It moves to `Accepted` in D7 (tranche 5), once inbound exists.

- [ ] **Step 2: Add the `chronacle-vault` row to "Internal Workspace Crates"**

```markdown
| `chronacle-vault` | Markdown vault sync engine (ADR-008): rendering, key mapping, three-way reconcile. Backend-agnostic — reaches storage and records only via `chronacle-core` ports. |
```

- [ ] **Step 3: Fix the "Crate & Tool Summary" YAML row**

Replace the existing row:

```markdown
| YAML frontmatter (vault sync) | `serde_yaml` |
```

with:

```markdown
| YAML frontmatter (vault sync) | `yaml_serde` (the YAML org's maintained successor; `serde_yaml` is archived and `serde_yml` carries RUSTSEC-2025-0068) |
```

Leave the `notify` row unchanged — it is already approved.

- [ ] **Step 4: Fix the AGENTS.md setting-keys line**

`AGENTS.md` currently lists `vault_include_gm_only` among the `setting` keys. That key is not shipping. Remove it, and leave `vault_sync_path`. Note `CLAUDE.md` is a symlink to `AGENTS.md` — edit `AGENTS.md` only.

- [ ] **Step 5: Verify formatting**

Run: `pnpm -C apps/desktop exec prettier --check ../../docs/architecture.md ../../AGENTS.md`
Expected: `All matched files use Prettier code style!` (run `--write` first if not).

- [ ] **Step 6: Run the full CI gate**

```bash
cargo fmt --all --check && \
cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && \
cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
```

Expected: all green.

- [ ] **Step 7: Commit and open the D0 PR**

```bash
git add docs/architecture.md AGENTS.md
git commit -m "docs(adr): amend ADR-008 for as-built model + ports"
git push -u origin chore/d0-vault-crate:refs/heads/chore/d0-vault-crate
gh pr create --base main --title "chore(vault): D0 — vault crate, core ports, ADR-008 amendment" --body "$(cat <<'EOF'
## What
Adds `chronacle-vault` (engine crate, no filesystem deps) and the four vault
ports in `chronacle-core`: `VaultStore`, `VaultWatcher`, `VaultOutbound`,
`VaultRecordStore`. Amends ADR-008, which was written against a data model
that no longer exists.

## Why
ADR-008 assumed one `entity` table with an `entity_type` field; the schema has
eight per-type tables. It also specified `tokio::fs` directly, `is_gm_only`
frontmatter (a reverted flag), and timestamp-driven conflict resolution that
cannot work because `compile.rs` never bumps `updated_at`.

See `docs/superpowers/specs/2026-07-09-codex-vault-sync-design.md`.

## Testing
`cargo test -p chronacle-core vault_ref` (4 tests). Full CI gate green,
including `cargo deny check` with the two new crates (`yaml_serde` 0.10.4,
`notify` 8.2.0) — `advisories ok, bans ok, licenses ok`.

No behaviour change: nothing constructs these ports yet.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 3: Frontmatter render + parse

**Files:**

- Create: `crates/chronacle-vault/src/frontmatter.rs`
- Modify: `crates/chronacle-vault/src/lib.rs` (register module)

**Interfaces:**

- Consumes: `VaultError` (Task 1).
- Produces: `Frontmatter`, `frontmatter::render(&Frontmatter) -> String`, `frontmatter::parse(&str) -> Result<(Frontmatter, String), FrontmatterError>`, `FrontmatterError`.

**Why quoting is unconditional:** an entity named `Vex: The Unbound` emits `name: Vex: The Unbound`, which is invalid YAML. One named `[Redacted]` parses as a list. Obsidian additionally treats `aliases` and `title` as meaningful, and a `[[wikilink]]` in a frontmatter value must be quoted to survive.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d1a-frontmatter chore/d0-vault-crate
```

- [ ] **Step 2: Write the failing tests**

At the bottom of the new `crates/chronacle-vault/src/frontmatter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn entity_fm() -> Frontmatter {
        Frontmatter {
            id: "npc:abc123".into(),
            name: Some("Seraphina Aldric".into()),
            title: Some("Seraphina Aldric".into()),
            aliases: vec!["Seraphina Aldric".into()],
            kind: Some("npc".into()),
            campaign: Some("Shadows of Valdris".into()),
            collection: None,
            category: None,
            session_number: None,
            date_played: None,
            page_refs: vec![],
            created_at: "2026-05-28T14:00:00Z".into(),
            updated_at: "2026-07-09T18:32:00Z".into(),
        }
    }

    #[test]
    fn render_emits_fenced_yaml_with_id_first() {
        let out = render(&entity_fm());
        assert!(out.starts_with("---\n"), "must open with a YAML fence");
        assert!(out.ends_with("---\n"), "must close with a YAML fence");
        let first_key = out.lines().nth(1).unwrap();
        assert!(first_key.starts_with("id:"), "id must be first, got {first_key:?}");
    }

    #[test]
    fn render_quotes_every_string_scalar() {
        let out = render(&entity_fm());
        assert!(out.contains(r#"id: "npc:abc123""#));
        assert!(out.contains(r#"name: "Seraphina Aldric""#));
        assert!(out.contains(r#"type: "npc""#));
    }

    #[test]
    fn render_quotes_a_name_containing_a_colon() {
        let mut fm = entity_fm();
        fm.name = Some("Vex: The Unbound".into());
        let out = render(&fm);
        assert!(out.contains(r#"name: "Vex: The Unbound""#), "got:\n{out}");
        // and it must survive a round-trip
        let (back, _) = parse(&format!("{out}\nbody")).expect("reparse");
        assert_eq!(back.name.as_deref(), Some("Vex: The Unbound"));
    }

    #[test]
    fn render_quotes_a_name_that_looks_like_a_wikilink() {
        let mut fm = entity_fm();
        fm.name = Some("[[Iron Tower]]".into());
        let out = render(&fm);
        let (back, _) = parse(&format!("{out}\nbody")).expect("reparse");
        assert_eq!(back.name.as_deref(), Some("[[Iron Tower]]"));
    }

    #[test]
    fn parse_splits_frontmatter_from_body() {
        let file = "---\nid: \"npc:a\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\n## Notes\n\nhi\n";
        let (fm, body) = parse(file).expect("parse");
        assert_eq!(fm.id, "npc:a");
        assert_eq!(body, "\n## Notes\n\nhi\n");
    }

    #[test]
    fn parse_rejects_a_file_with_no_frontmatter() {
        assert!(matches!(parse("## Notes\n"), Err(FrontmatterError::Missing)));
    }

    #[test]
    fn parse_rejects_frontmatter_with_no_id() {
        let file = "---\nname: \"x\"\n---\nbody\n";
        assert!(matches!(parse(file), Err(FrontmatterError::MissingId)));
    }

    #[test]
    fn render_then_parse_round_trips() {
        let fm = entity_fm();
        let file = format!("{}\nbody\n", render(&fm));
        let (back, body) = parse(&file).expect("parse");
        assert_eq!(back, fm);
        assert_eq!(body, "\nbody\n");
    }

    #[test]
    fn aliases_survive_a_round_trip_as_a_list() {
        let mut fm = entity_fm();
        fm.aliases = vec!["A".into(), "B".into()];
        let file = format!("{}\nbody\n", render(&fm));
        let (back, _) = parse(&file).expect("parse");
        assert_eq!(back.aliases, vec!["A".to_string(), "B".to_string()]);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p chronacle-vault frontmatter`
Expected: FAIL — `cannot find function render in this scope`.

- [ ] **Step 4: Implement `frontmatter.rs`**

`Frontmatter` is a fixed struct, not a free-form map: the vault's frontmatter is a closed vocabulary the engine owns. `type` is a Rust keyword, so the field is `kind` with `#[serde(rename = "type")]`.

```rust
//! YAML frontmatter for vault files.
//!
//! All string scalars are emitted **quoted, unconditionally**. An entity named
//! `Vex: The Unbound` would otherwise emit invalid YAML, and one named
//! `[[Iron Tower]]` would parse as a nested list. `aliases` and `title` are
//! Obsidian-meaningful keys, not private serialisation keys.

use chronacle_core::RulePageRef;
use serde::{Deserialize, Serialize};

/// The closed frontmatter vocabulary. Field order here is emission order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Stable record identity, e.g. `"npc:abc123"`. Never derived from the path.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Obsidian display name. Set for every record; the filename is a slug.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Makes `[[Name]]` resolve to a slug-named file. Without this every
    /// compiled wikilink renders broken in Obsidian.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_played: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_refs: Vec<RulePageRef>,
    pub created_at: String,
    pub updated_at: String,
}

/// Errors from parsing a vault file's frontmatter.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrontmatterError {
    /// The file does not open with a `---` fence.
    #[error("file has no frontmatter")]
    Missing,
    /// Frontmatter present but carries no `id` — cannot be identified.
    #[error("frontmatter has no id")]
    MissingId,
    /// The YAML between the fences did not parse.
    #[error("invalid YAML: {0}")]
    Yaml(String),
}

/// Render frontmatter, including the opening and closing `---` fences.
///
/// Trailing newline is included, so a caller appends the body directly.
pub fn render(fm: &Frontmatter) -> String {
    // yaml_serde emits unquoted scalars where YAML permits it. We re-quote
    // every string scalar by serialising through a wrapper that forces the
    // quoted style, because Obsidian and our own parser both round-trip
    // through this text and a bare `Vex: The Unbound` is not valid YAML.
    let body = yaml_serde::to_string(fm).expect("Frontmatter is always serialisable");
    let quoted = force_quote_scalars(&body);
    format!("---\n{quoted}---\n")
}

/// Split a vault file into its frontmatter and its body.
///
/// The body is returned verbatim, including its leading newline — the caller
/// (`markdown::split_body`) owns all further structure.
pub fn parse(file: &str) -> Result<(Frontmatter, String), FrontmatterError> {
    let rest = file.strip_prefix("---\n").ok_or(FrontmatterError::Missing)?;
    let end = rest.find("\n---\n").ok_or(FrontmatterError::Missing)?;
    let (yaml, body) = rest.split_at(end);
    let body = &body["\n---\n".len()..];

    let fm: Frontmatter =
        yaml_serde::from_str(yaml).map_err(|e| FrontmatterError::Yaml(e.to_string()))?;
    if fm.id.trim().is_empty() {
        return Err(FrontmatterError::MissingId);
    }
    Ok((fm, body.to_owned()))
}
```

Implement `force_quote_scalars` as a private helper: for each top-level `key: value` line whose value is a bare scalar (not `[`, not already `"`-wrapped, not a number for `session_number`), wrap the value in `"` and escape any embedded `"` and `\`. A missing `id` key must surface as `FrontmatterError::MissingId`, so make `id` a required (non-`Option`) field and map serde's missing-field error for `id` onto `MissingId` before falling through to `Yaml`.

- [ ] **Step 5: Register the module**

In `crates/chronacle-vault/src/lib.rs`:

```rust
pub mod frontmatter;
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p chronacle-vault frontmatter`
Expected: PASS — 9 tests.

- [ ] **Step 7: Commit**

```bash
git add crates/chronacle-vault/src/frontmatter.rs crates/chronacle-vault/src/lib.rs
git commit -m "feat(vault): frontmatter render/parse with forced quoting"
```

---

### Task 4: Markdown fence, lossless body grammar, normalized compare — and the D1a PR

**Files:**

- Create: `crates/chronacle-vault/src/markdown.rs`
- Modify: `crates/chronacle-vault/src/lib.rs` (register module)

**Interfaces:**

- Consumes: nothing from Task 3 (these modules are siblings).
- Produces: `FENCE_START`, `FENCE_END`, `SUMMARY_HEADING`, `BodyParts`, `markdown::split_body(&str) -> BodyParts`, `markdown::render_body(&BodyParts) -> String`, `markdown::normalize(&str) -> String`.

**Why the grammar must be lossless:** outbound renders the body **from the record**. Any GM prose the parser fails to recognise is destroyed by the next outbound write — and the next write can be triggered by a compile the GM never asked for. So everything outside the fence and outside a leading `## Summary` maps into `notes` verbatim, including unknown headings. `## Notes` is a rendering convention, not a parsing requirement.

- [ ] **Step 1: Write the failing tests**

At the bottom of the new `crates/chronacle-vault/src/markdown.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_strips_crlf_and_trims() {
        assert_eq!(normalize("a\r\nb\r\n"), "a\nb");
        assert_eq!(normalize("  a\n\n"), "a");
        assert_eq!(normalize("a"), "a");
    }

    #[test]
    fn normalize_makes_a_trailing_newline_invisible() {
        // The whole point: an editor appending "\n" must not manufacture a conflict.
        assert_eq!(normalize("body"), normalize("body\n"));
        assert_eq!(normalize("body"), normalize("body\r\n"));
    }

    #[test]
    fn split_body_extracts_summary_fence_and_notes() {
        let body = format!(
            "\n## Summary\n\nA short summary.\n\n{FENCE_START}\nCompiled text.\n{FENCE_END}\n\n## Notes\n\nGM notes.\n"
        );
        let parts = split_body(&body);
        assert_eq!(parts.summary.as_deref(), Some("A short summary."));
        assert_eq!(parts.fenced.as_deref(), Some("Compiled text."));
        assert_eq!(parts.notes.as_deref(), Some("GM notes."));
    }

    #[test]
    fn split_body_keeps_unknown_headings_in_notes() {
        let body = format!(
            "\n## Summary\n\nS.\n\n{FENCE_START}\nC.\n{FENCE_END}\n\n## Notes\n\nN.\n\n## Ideas\n\nAn idea.\n"
        );
        let parts = split_body(&body);
        let notes = parts.notes.expect("notes");
        assert!(notes.contains("N."), "got {notes:?}");
        assert!(notes.contains("## Ideas"), "unknown heading must survive: {notes:?}");
        assert!(notes.contains("An idea."), "unknown section body must survive: {notes:?}");
    }

    #[test]
    fn split_body_keeps_prose_written_above_the_first_heading() {
        let body = "\nStray prose.\n\n## Notes\n\nN.\n";
        let parts = split_body(body);
        let notes = parts.notes.expect("notes");
        assert!(notes.contains("Stray prose."), "got {notes:?}");
    }

    #[test]
    fn split_body_treats_a_deleted_notes_heading_as_notes() {
        let body = format!("\n{FENCE_START}\nC.\n{FENCE_END}\n\nJust prose, no heading.\n");
        let parts = split_body(&body);
        assert_eq!(parts.fenced.as_deref(), Some("C."));
        assert_eq!(parts.notes.as_deref(), Some("Just prose, no heading."));
    }

    #[test]
    fn split_body_handles_a_session_file_with_no_fence() {
        let parts = split_body("\nSession recap.\n");
        assert_eq!(parts.fenced, None);
        assert_eq!(parts.summary, None);
        assert_eq!(parts.notes.as_deref(), Some("Session recap."));
    }

    #[test]
    fn render_body_then_split_body_round_trips() {
        let parts = BodyParts {
            summary: Some("S.".into()),
            fenced: Some("C.".into()),
            notes: Some("N.\n\n## Ideas\n\nAn idea.".into()),
        };
        let rendered = render_body(&parts);
        let back = split_body(&rendered);
        assert_eq!(back.summary, parts.summary);
        assert_eq!(back.fenced, parts.fenced);
        assert_eq!(back.notes, parts.notes);
    }

    #[test]
    fn render_body_omits_absent_sections() {
        let parts = BodyParts { summary: None, fenced: None, notes: Some("N.".into()) };
        let out = render_body(&parts);
        assert!(!out.contains(SUMMARY_HEADING));
        assert!(!out.contains(FENCE_START));
        assert!(out.contains("N."));
    }

    #[test]
    fn an_unterminated_fence_is_treated_as_notes_not_as_article() {
        // A GM who deletes the closing marker must not have their prose
        // silently reclassified as compiler-owned.
        let body = format!("\n{FENCE_START}\nDangling.\n");
        let parts = split_body(&body);
        assert_eq!(parts.fenced, None, "no end marker => no fence");
        assert!(parts.notes.expect("notes").contains("Dangling."));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p chronacle-vault markdown`
Expected: FAIL — `cannot find function normalize in this scope`.

- [ ] **Step 3: Implement `markdown.rs`**

```rust
//! Vault file body grammar.
//!
//! The compiled article lives inside an HTML-comment fence. Everything outside
//! the fence and outside a leading `## Summary` is GM-owned `notes`, verbatim.
//! The grammar is **lossless by construction**: outbound renders from the
//! record, so any unrecognised prose would otherwise be destroyed by the next
//! compile the GM never asked for.

/// Opening marker of the compiler-owned region.
pub const FENCE_START: &str =
    "<!-- chronacle:codex-article start -- compiled; edits are not applied -->";
/// Closing marker of the compiler-owned region.
pub const FENCE_END: &str = "<!-- chronacle:codex-article end -->";
/// Heading that delimits the GM-owned `summary` field.
pub const SUMMARY_HEADING: &str = "## Summary";
/// Heading emitted above `notes`. A parsing convention only — its absence is fine.
pub const NOTES_HEADING: &str = "## Notes";

/// The three regions of a vault file body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BodyParts {
    /// GM-owned. `None` for sessions and rule entries.
    pub summary: Option<String>,
    /// Compiler-owned: `codex_article` or `rule_entry.body`. Never applied inbound.
    pub fenced: Option<String>,
    /// GM-owned. Everything else, verbatim.
    pub notes: Option<String>,
}

/// Trim and normalise line endings. **Every** comparison in the engine runs on
/// normalized text — a byte-exact compare would manufacture a conflict each
/// time an editor appends a trailing newline.
pub fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_owned()
}
```

`split_body` algorithm — implement exactly this, so the round-trip test holds:

1. Find `FENCE_START`. If found, find `FENCE_END` **after** it. If both are present, `fenced` is the normalized text strictly between them, and the fence region (markers included) is cut out of the body. If `FENCE_START` is present but `FENCE_END` is not, **do not** treat anything as fenced — leave the text in place so the GM's prose survives.
2. In the remainder, if the first non-blank line is exactly `SUMMARY_HEADING`, `summary` is the normalized text from after that line up to (but excluding) the next `## ` heading at line start, or end of input.
3. Cut that summary region out. In what remains, drop a leading `NOTES_HEADING` line if present. Everything left, normalized, is `notes` (`None` if empty).

`render_body` emits, in order, each present section separated by one blank line: `## Summary\n\n{summary}`, then `{FENCE_START}\n{fenced}\n{FENCE_END}`, then `## Notes\n\n{notes}`. Prepend a leading `\n` so the body starts on its own line after the frontmatter fence.

- [ ] **Step 4: Register the module**

In `crates/chronacle-vault/src/lib.rs`:

```rust
pub mod markdown;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p chronacle-vault`
Expected: PASS — 9 frontmatter + 10 markdown tests.

- [ ] **Step 6: Run the full CI gate**

```bash
cargo fmt --all --check && \
cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && \
cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
```

Expected: all green.

- [ ] **Step 7: Commit and open the D1a PR**

```bash
git add crates/chronacle-vault
git commit -m "feat(vault): lossless body grammar + normalized compare"
git push -u origin feat/d1a-frontmatter:refs/heads/feat/d1a-frontmatter
gh pr create --base chore/d0-vault-crate --title "feat(vault): D1a — frontmatter + body grammar" --body "$(cat <<'EOF'
## What
Frontmatter render/parse with unconditional scalar quoting, and a lossless
body grammar: fenced compiler-owned article, GM-owned `## Summary`, and
everything else verbatim into `notes`.

## Why
`aliases`/`title` are Obsidian-meaningful — without `aliases`, every compiled
`[[wikilink]]` renders broken, because `wikilink/mod.rs` resolves links against
entity `name` while files are slug-named. Unconditional quoting keeps a name
like `Vex: The Unbound` from emitting invalid YAML.

The grammar is lossless because outbound renders from the record: an
unrecognised `## Ideas` section would otherwise be destroyed by the next
compile.

## Testing
`cargo test -p chronacle-vault` — 19 unit tests, including round-trips, an
unterminated fence (must not reclassify GM prose as compiler-owned), and
trailing-newline/CRLF invariance. Full CI gate green including `cargo deny check`.

Stacked on #<D0>.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 5: Key mapping — slug, collision suffix, scope folders, managed gating

**Files:**

- Create: `crates/chronacle-vault/src/keys.rs`
- Modify: `crates/chronacle-vault/src/lib.rs` (register module)

**Interfaces:**

- Consumes: `VaultRecord`, `VaultScope`, `VaultRef` (Task 1).
- Produces: `keys::slug(&str) -> String`, `keys::scope_folder(&VaultScope) -> String`, `keys::key_for(&VaultRecord, bool) -> VaultKey`, `keys::is_managed(&str) -> bool`, `keys::entity_type_of(&VaultKey) -> Option<&str>`.

**Why scope folders also need the collision suffix:** `campaign.name` has no unique index (`001_base_schema.surql:9-13`), so two campaigns can share a name and would otherwise write into the same folder.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d1b-key-mapping feat/d1a-frontmatter
```

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{EntityRecord, RuleEntryRecord, SessionRecord, VaultRecord, VaultRef, VaultScope};
    use pretty_assertions::assert_eq;

    fn campaign() -> VaultScope {
        VaultScope::Campaign { id: "campaign:c1".into(), name: "Shadows of Valdris".into() }
    }
    fn collection() -> VaultScope {
        VaultScope::Collection { id: "collection:k1".into(), name: "D&D 5e Core".into() }
    }
    fn npc(name: &str, id: &str, scope: VaultScope) -> VaultRecord {
        VaultRecord::Entity(EntityRecord {
            vref: VaultRef { table: "npc".into(), id: id.into() },
            name: name.into(), summary: None, notes: None, codex_article: None,
            scope, created_at: "x".into(), updated_at: "y".into(),
        })
    }

    #[test]
    fn slug_lowercases_and_hyphenates() {
        assert_eq!(slug("Seraphina Aldric"), "seraphina-aldric");
        assert_eq!(slug("The Iron Tower"), "the-iron-tower");
    }

    #[test]
    fn slug_strips_punctuation_and_collapses_separators() {
        assert_eq!(slug("Vex: The Unbound!"), "vex-the-unbound");
        assert_eq!(slug("A  --  B"), "a-b");
        assert_eq!(slug("  padded  "), "padded");
    }

    #[test]
    fn slug_never_returns_empty() {
        // A name of pure punctuation must still produce a usable filename.
        assert_eq!(slug("???"), "untitled");
        assert_eq!(slug(""), "untitled");
    }

    #[test]
    fn scope_folder_roots_campaigns_and_collections_separately() {
        assert_eq!(scope_folder(&campaign()), "campaigns/shadows-of-valdris");
        assert_eq!(scope_folder(&collection()), "collections/d-d-5e-core");
    }

    #[test]
    fn scope_folder_suffixes_on_collision() {
        // Two campaigns may share a name — campaign.name has no UNIQUE index.
        let a = VaultScope::Campaign { id: "campaign:aaa".into(), name: "Guard Duty".into() };
        let b = VaultScope::Campaign { id: "campaign:bbb".into(), name: "Guard Duty".into() };
        assert_ne!(scope_folder_disambiguated(&a, true), scope_folder_disambiguated(&b, true));
        assert!(scope_folder_disambiguated(&a, true).starts_with("campaigns/guard-duty-"));
    }

    #[test]
    fn key_for_entity_nests_under_scope_and_type() {
        let k = key_for(&npc("Seraphina Aldric", "abc123", campaign()), false);
        assert_eq!(k, "campaigns/shadows-of-valdris/entities/npc/seraphina-aldric.md");
    }

    #[test]
    fn key_for_collection_owned_entity_uses_the_collections_root() {
        let k = key_for(&npc("Goblin", "g1", collection()), false);
        assert_eq!(k, "collections/d-d-5e-core/entities/npc/goblin.md");
    }

    #[test]
    fn key_for_appends_an_id_suffix_on_collision() {
        let a = key_for(&npc("Guard", "4f2a1c", campaign()), true);
        let b = key_for(&npc("Guard", "9e8d7b", campaign()), true);
        assert_ne!(a, b);
        assert!(a.ends_with("/guard-4f2a1c.md"), "got {a}");
    }

    #[test]
    fn key_for_session_uses_a_zero_padded_number() {
        let rec = VaultRecord::Session(SessionRecord {
            vref: VaultRef { table: "session".into(), id: "s1".into() },
            session_number: 1, title: "The Awakening".into(), date_played: "2026-01-01".into(),
            notes: String::new(), campaign: campaign(),
            created_at: "x".into(), updated_at: "y".into(),
        });
        assert_eq!(
            key_for(&rec, false),
            "campaigns/shadows-of-valdris/sessions/001-the-awakening.md"
        );
    }

    #[test]
    fn key_for_rule_entry_lands_under_rules() {
        let rec = VaultRecord::RuleEntry(RuleEntryRecord {
            vref: VaultRef { table: "rule_entry".into(), id: "r1".into() },
            name: "Grappling".into(), category: "procedure".into(), body: String::new(),
            notes: None, page_refs: vec![], collection: collection(),
            created_at: "x".into(), updated_at: "y".into(),
        });
        assert_eq!(key_for(&rec, false), "collections/d-d-5e-core/rules/grappling.md");
    }

    #[test]
    fn is_managed_accepts_only_the_two_roots() {
        assert!(is_managed("campaigns/x/entities/npc/a.md"));
        assert!(is_managed("collections/x/rules/a.md"));
        assert!(!is_managed("a.md"), "vault root is unmanaged");
        assert!(!is_managed(".obsidian/workspace.json"));
        assert!(!is_managed("campaigns/x/entities/npc/a.conflict.123.md"));
        assert!(!is_managed("Templates/entity.md"));
    }

    #[test]
    fn entity_type_of_reads_the_type_folder() {
        assert_eq!(entity_type_of("campaigns/x/entities/npc/a.md"), Some("npc"));
        assert_eq!(entity_type_of("collections/x/rules/a.md"), None);
        assert_eq!(entity_type_of("campaigns/x/sessions/001-a.md"), None);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p chronacle-vault keys`
Expected: FAIL — `cannot find function slug in this scope`.

- [ ] **Step 4: Implement `keys.rs`**

```rust
//! Record ↔ vault-key mapping.
//!
//! A key is a POSIX-style, `/`-separated path relative to the vault root. It is
//! **derived** from the record and never authoritative: identity is the
//! frontmatter `id`. A file renamed in Obsidian keeps its record; only the
//! *type folder* carries meaning.

use chronacle_core::{VaultKey, VaultRecord, VaultScope};

/// Recognised entity type folders. Mirrors the eight per-type tables.
pub const ENTITY_TYPES: [&str; 8] = [
    "npc", "location", "faction", "creature", "item", "event", "player_character", "misc",
];

/// Lowercase, ASCII, hyphen-separated. Never empty — falls back to `"untitled"`.
pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = true; // suppresses a leading dash
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "untitled".to_owned() } else { out }
}
```

Then:

- `fn raw_id(scope_or_ref: &str) -> &str` — strips the `table:` prefix from a thing string.
- `pub fn scope_folder(scope: &VaultScope) -> String` → `scope_folder_disambiguated(scope, false)`.
- `pub fn scope_folder_disambiguated(scope: &VaultScope, collides: bool) -> String` — `campaigns/{slug}` or `collections/{slug}`, appending `-{raw_id}` when `collides`.
- `pub fn key_for(record: &VaultRecord, collides: bool) -> VaultKey` — dispatches on the enum:
  - `Entity` → `{scope_folder}/entities/{vref.table}/{slug(name)}{suffix}.md`
  - `Session` → `{scope_folder}/sessions/{session_number:03}-{slug(title)}{suffix}.md`
  - `RuleEntry` → `{scope_folder}/rules/{slug(name)}{suffix}.md`
  - where `suffix` is `-{raw_id}` when `collides`, else empty.
- `pub fn is_managed(key: &str) -> bool` — `true` only for the four exact shapes `campaigns/<slug>/entities/<type>/<file>.md`, `campaigns/<slug>/sessions/<file>.md`, `collections/<slug>/entities/<type>/<file>.md`, and `collections/<slug>/rules/<file>.md`, where `<type>` is a member of `ENTITY_TYPES` and `<slug>`/`<file>` are non-empty, `/`-free segments; and does **not** match `*.conflict.*.md`. (The conflict exclusion matters on the watcher path in tranche 5, but the predicate lives here.)
- `pub fn entity_type_of(key: &str) -> Option<&str>` — returns the segment after `entities/` when it is in `ENTITY_TYPES`.

- [ ] **Step 5: Register and run**

Add `pub mod keys;` to `lib.rs`.
Run: `cargo test -p chronacle-vault keys`
Expected: PASS — 12 tests.

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-vault
git commit -m "feat(vault): record-to-key mapping with collision suffixes"
```

---

### Task 6: `VaultIndex` — the id → key scan — and the D1b PR

**Files:**

- Modify: `crates/chronacle-vault/src/keys.rs` (append `VaultIndex`)

**Interfaces:**

- Consumes: `VaultStore` (Task 1), `frontmatter::parse` (Task 3), `is_managed` (Task 5).
- Produces: `keys::VaultIndex`, `VaultIndex::scan(&dyn VaultStore) -> Result<VaultIndex, VaultError>`, `VaultIndex::key_of(&VaultRef) -> Option<&VaultKey>`, `VaultIndex::contains(&VaultRef) -> bool`, `VaultIndex::len()`, `VaultIndex::is_empty()`.

**Why this exists:** reconcile must never compute an expected slug and look for it. Filenames derive from `name`, which is neither unique nor stable. The index is built by reading frontmatter, so a renamed file is still found.

- [ ] **Step 1: Write the failing tests**

`MockVaultStore` comes from `chronacle-core`'s `mocks` feature (Task 1).

```rust
#[cfg(test)]
mod index_tests {
    use super::*;
    use chronacle_core::{MockVaultStore, VaultRef};
    use mockall::predicate::eq;

    fn file_with_id(id: &str) -> String {
        format!("---\nid: \"{id}\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\nbody\n")
    }

    #[tokio::test]
    async fn scan_maps_ids_to_keys_regardless_of_filename() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| {
            Ok(vec!["campaigns/c/entities/npc/renamed-by-the-gm.md".to_string()])
        });
        store
            .expect_read()
            .with(eq("campaigns/c/entities/npc/renamed-by-the-gm.md"))
            .returning(|_| Ok(file_with_id("npc:abc123")));

        let idx = VaultIndex::scan(&store).await.expect("scan");
        let vref = VaultRef { table: "npc".into(), id: "abc123".into() };
        assert_eq!(
            idx.key_of(&vref).map(String::as_str),
            Some("campaigns/c/entities/npc/renamed-by-the-gm.md"),
            "identity is the frontmatter id, not the slug"
        );
    }

    #[tokio::test]
    async fn scan_ignores_unmanaged_keys() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| {
            Ok(vec![
                "Templates/entity.md".to_string(),
                ".obsidian/workspace.json".to_string(),
                "campaigns/c/entities/npc/a.conflict.9.md".to_string(),
            ])
        });
        // read() must never be called for an unmanaged key.
        store.expect_read().never();

        let idx = VaultIndex::scan(&store).await.expect("scan");
        assert_eq!(idx.len(), 0);
    }

    #[tokio::test]
    async fn scan_skips_a_managed_file_with_no_frontmatter() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec!["campaigns/c/entities/npc/new.md".to_string()]));
        store.expect_read().returning(|_| Ok("just prose, no frontmatter\n".to_string()));

        // An id-less file is a tranche-5 create candidate, not an index entry,
        // and must not abort the scan.
        let idx = VaultIndex::scan(&store).await.expect("scan must tolerate id-less files");
        assert_eq!(idx.len(), 0);
    }

    #[tokio::test]
    async fn scan_records_the_slug_to_scope_map() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| {
            Ok(vec!["campaigns/shadows-of-valdris/entities/npc/a.md".to_string()])
        });
        store.expect_read().returning(|_| Ok(file_with_id("npc:a1")));

        let idx = VaultIndex::scan(&store).await.expect("scan");
        assert!(idx.contains(&VaultRef { table: "npc".into(), id: "a1".into() }));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p chronacle-vault index_tests`
Expected: FAIL — `cannot find type VaultIndex in this scope`.

- [ ] **Step 3: Implement `VaultIndex`**

```rust
/// `id → key` map built by scanning the vault and reading frontmatter.
///
/// Reconcile matches records to files through this map. It never computes an
/// expected slug: filenames derive from `name`, which is neither unique
/// (two NPCs called "Guard") nor stable (a rename would orphan the file).
#[derive(Debug, Default)]
pub struct VaultIndex {
    by_ref: std::collections::HashMap<chronacle_core::VaultRef, VaultKey>,
}

impl VaultIndex {
    /// Read every managed `.md` under the vault root and index it by `id`.
    ///
    /// Files with no frontmatter, or unparsable frontmatter, are skipped —
    /// they are tranche-5 create candidates, not errors.
    pub async fn scan(store: &dyn chronacle_core::VaultStore) -> Result<Self, crate::VaultError> {
        let mut by_ref = std::collections::HashMap::new();
        for key in store.list("").await? {
            if !is_managed(&key) {
                continue;
            }
            let content = store.read(&key).await?;
            let Ok((fm, _)) = crate::frontmatter::parse(&content) else {
                continue;
            };
            let Some(vref) = chronacle_core::VaultRef::parse(&fm.id) else {
                continue;
            };
            by_ref.insert(vref, key);
        }
        Ok(Self { by_ref })
    }

    /// The key currently holding this record, if any.
    pub fn key_of(&self, vref: &chronacle_core::VaultRef) -> Option<&VaultKey> {
        self.by_ref.get(vref)
    }

    /// Whether the vault holds a file for this record.
    pub fn contains(&self, vref: &chronacle_core::VaultRef) -> bool {
        self.by_ref.contains_key(vref)
    }

    /// Number of indexed records.
    pub fn len(&self) -> usize {
        self.by_ref.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.by_ref.is_empty()
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p chronacle-vault`
Expected: PASS — 19 (D1a) + 12 (keys) + 4 (index) = 35 tests.

- [ ] **Step 5: Run the full CI gate**

```bash
cargo fmt --all --check && \
cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
```

- [ ] **Step 6: Commit and open the D1b PR**

```bash
git add crates/chronacle-vault
git commit -m "feat(vault): VaultIndex id-to-key scan"
git push -u origin feat/d1b-key-mapping:refs/heads/feat/d1b-key-mapping
gh pr create --base feat/d1a-frontmatter --title "feat(vault): D1b — key mapping + id-keyed index" --body "$(cat <<'EOF'
## What
Record ↔ key mapping (slug, `id`-derived collision suffix for both files and
scope folders, managed-folder gating), plus `VaultIndex`: the `id → key` scan
that makes the frontmatter `id` the sole identity.

## Why
Filenames derive from `name`, which is neither unique (two NPCs called "Guard")
nor stable. Matching records to files by computed slug would corrupt on every
rename. Scope folders need the same suffix rule because `campaign.name` has no
UNIQUE index.

## Testing
`cargo test -p chronacle-vault` — 35 tests. `VaultIndex` is driven entirely
through `MockVaultStore`; a renamed file is still resolved by `id`, unmanaged
keys are never even read, and an id-less file is skipped rather than fataling.
Full CI gate green including `cargo deny check`.

Stacked on #<D1a>.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 7: `LocalFsVaultStore` — the D2a PR

**Files:**

- Create: `crates/chronacle-providers/src/vault_store.rs`
- Modify: `crates/chronacle-providers/src/lib.rs` (register module)
- Modify: `crates/chronacle-providers/Cargo.toml` (add `notify = "8"` here if Task 1 Step 9 deferred it; add `chronacle-core` `mocks` dev-feature if needed)

**Interfaces:**

- Consumes: `VaultStore`, `VaultStoreError`, `VaultMetadata` (Task 1).
- Produces: `LocalFsVaultStore::new(impl Into<PathBuf>) -> Self` and its `VaultStore` impl.

**This is the only place in the tranche that touches a real filesystem.** It is a thin adapter; all interesting logic lives in the engine and is tested with `MockVaultStore`.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d2a-fs-store feat/d1b-key-mapping
```

- [ ] **Step 2: Write the failing integration tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{VaultStore, VaultStoreError};
    use tempfile::TempDir;

    fn store() -> (TempDir, LocalFsVaultStore) {
        let dir = TempDir::new().expect("tempdir");
        let store = LocalFsVaultStore::new(dir.path());
        (dir, store)
    }

    #[tokio::test]
    async fn write_then_read_round_trips() {
        let (_dir, store) = store();
        store.write("campaigns/c/entities/npc/a.md", "hello").await.expect("write");
        assert_eq!(store.read("campaigns/c/entities/npc/a.md").await.expect("read"), "hello");
    }

    #[tokio::test]
    async fn write_creates_missing_parent_directories() {
        let (_dir, store) = store();
        // Nothing pre-creates `campaigns/c/entities/npc/`.
        store.write("campaigns/c/entities/npc/deep.md", "x").await.expect("write must mkdir -p");
        assert_eq!(store.read("campaigns/c/entities/npc/deep.md").await.expect("read"), "x");
    }

    #[tokio::test]
    async fn read_of_a_missing_key_is_not_found() {
        let (_dir, store) = store();
        assert!(matches!(store.read("nope.md").await, Err(VaultStoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn delete_removes_the_file_and_is_idempotent() {
        let (_dir, store) = store();
        store.write("a.md", "x").await.expect("write");
        store.delete("a.md").await.expect("delete");
        assert!(matches!(store.read("a.md").await, Err(VaultStoreError::NotFound(_))));
        store.delete("a.md").await.expect("deleting an absent key must succeed");
    }

    #[tokio::test]
    async fn list_returns_posix_keys_recursively_and_only_md() {
        let (_dir, store) = store();
        store.write("campaigns/c/entities/npc/a.md", "x").await.expect("write");
        store.write("campaigns/c/sessions/001-b.md", "x").await.expect("write");
        store.write(".obsidian/workspace.json", "{}").await.expect("write");

        let mut keys = store.list("").await.expect("list");
        keys.sort();
        assert_eq!(
            keys,
            vec![
                "campaigns/c/entities/npc/a.md".to_string(),
                "campaigns/c/sessions/001-b.md".to_string(),
            ],
            "only .md files, POSIX separators, no OS paths"
        );
    }

    #[tokio::test]
    async fn list_honours_the_prefix() {
        let (_dir, store) = store();
        store.write("campaigns/c/entities/npc/a.md", "x").await.expect("write");
        store.write("collections/k/rules/g.md", "x").await.expect("write");
        let keys = store.list("collections").await.expect("list");
        assert_eq!(keys, vec!["collections/k/rules/g.md".to_string()]);
    }

    #[tokio::test]
    async fn list_of_a_missing_prefix_is_empty_not_an_error() {
        let (_dir, store) = store();
        assert_eq!(store.list("campaigns").await.expect("list"), Vec::<String>::new());
    }

    #[tokio::test]
    async fn metadata_returns_a_monotonic_mtime() {
        let (_dir, store) = store();
        store.write("a.md", "x").await.expect("write");
        let m1 = store.metadata("a.md").await.expect("metadata");
        store.write("a.md", "y").await.expect("rewrite");
        let m2 = store.metadata("a.md").await.expect("metadata");
        assert!(m2.mtime >= m1.mtime);
    }

    #[tokio::test]
    async fn a_key_escaping_the_root_is_rejected() {
        let (_dir, store) = store();
        assert!(matches!(store.write("../escape.md", "x").await, Err(VaultStoreError::InvalidKey(_))));
        assert!(matches!(store.read("campaigns/../../etc/passwd").await, Err(VaultStoreError::InvalidKey(_))));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p chronacle-providers vault_store`
Expected: FAIL — `cannot find type LocalFsVaultStore in this scope`.

- [ ] **Step 4: Implement `LocalFsVaultStore`**

```rust
//! Filesystem-backed `VaultStore`.
//!
//! The only component in the vault stack that knows about `tokio::fs`. Keys are
//! POSIX-style and root-relative; this adapter is the sole place they become OS
//! paths. There is deliberately no `rename()` — S3 has none, and a re-key is
//! `write(new) + delete(old)`.

use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use chronacle_core::{VaultKey, VaultMetadata, VaultStore, VaultStoreError};

/// A `VaultStore` rooted at a directory on the local filesystem.
pub struct LocalFsVaultStore {
    root: PathBuf,
}

impl LocalFsVaultStore {
    /// Create a store rooted at `root`. The directory need not exist yet.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Resolve a key to an absolute path, refusing anything that escapes the root.
    fn resolve(&self, key: &str) -> Result<PathBuf, VaultStoreError> {
        let rel = Path::new(key);
        if rel.is_absolute()
            || rel.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(VaultStoreError::InvalidKey(key.to_owned()));
        }
        Ok(self.root.join(rel))
    }
}
```

Then the `#[async_trait] impl VaultStore`:

- `read` — `tokio::fs::read_to_string`; map `ErrorKind::NotFound` to `VaultStoreError::NotFound(key)`, everything else to `Io`.
- `write` — `tokio::fs::create_dir_all(parent)` then `tokio::fs::write`.
- `delete` — `tokio::fs::remove_file`; **`ErrorKind::NotFound` is `Ok(())`**, so reconcile is idempotent.
- `list` — walk `self.root.join(prefix)` recursively with an explicit stack (no new crate); skip non-`.md`; return keys built by `strip_prefix(&self.root)` and joining components with `/` so Windows `\` never leaks. A missing directory yields `Ok(vec![])`.
- `metadata` — `tokio::fs::metadata(...).modified()` into `VaultMetadata { mtime }`.

- [ ] **Step 5: Register the module**

In `crates/chronacle-providers/src/lib.rs`: `pub mod vault_store;`

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p chronacle-providers vault_store`
Expected: PASS — 9 tests.

- [ ] **Step 7: Full CI gate, commit, open the D2a PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend

git add crates/chronacle-providers Cargo.lock
git commit -m "feat(vault): LocalFsVaultStore over tokio::fs"
git push -u origin feat/d2a-fs-store:refs/heads/feat/d2a-fs-store
gh pr create --base feat/d1b-key-mapping --title "feat(vault): D2a — filesystem VaultStore" --body "$(cat <<'EOF'
## What
`LocalFsVaultStore`: the filesystem `VaultStore` adapter. Key-addressed, no
`rename()` (S3 has none; a re-key is write-then-delete), path-traversal
rejected, `delete` idempotent, `list` returns POSIX keys not OS paths.

## Why
This is the only component in the vault stack permitted to touch `tokio::fs`.
Keeping it a thin adapter is what lets the engine's reconcile and conflict logic
be tested with `MockVaultStore` against forced I/O errors — states unreachable
through a real disk.

## Testing
`cargo test -p chronacle-providers vault_store` — 9 TempDir integration tests,
including root-escape rejection and idempotent delete. Full CI gate green
including `cargo deny check`.

Stacked on #<D1b>.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 8: `003_vault_sync.surql` — `vault_deleted` ×9 + `vault_sync_state`

**Files:**

- Create: `crates/chronacle-db/src/schema/003_vault_sync.surql`
- Modify: `crates/chronacle-db/src/schema/mod.rs` (doc comment listing schema files; append migration tests)

**Interfaces:**

- Consumes: nothing.
- Produces: the `vault_deleted` field on `npc`, `location`, `faction`, `creature`, `item`, `event`, `player_character`, `misc`, `session`; the `vault_sync_state` table.

**Two traps this task exists to defuse.** First, `run_migrations` re-executes every `.surql` on **every boot**, so the file must be `DEFINE`-only — a `REMOVE TABLE` here once wiped every `relates_to` edge on restart. Second, `DEFAULT false` **does not backfill** rows written before the migration; a `WHERE vault_deleted = false` filter silently omits every pre-existing entity.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d2b-record-store feat/d2a-fs-store
```

- [ ] **Step 2: Write the failing migration tests**

Append to the `mod tests` block in `crates/chronacle-db/src/schema/mod.rs`:

```rust
#[tokio::test]
async fn vault_deleted_exists_on_all_nine_syncable_tables() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(()).await.expect("db");
    db.use_ns("test").use_db("test").await.unwrap();
    run_migrations(&db).await.expect("migrations");

    #[derive(serde::Deserialize)]
    struct TableInfo { fields: std::collections::HashMap<String, serde_json::Value> }

    for table in &[
        "npc", "location", "faction", "creature", "item", "event",
        "player_character", "misc", "session",
    ] {
        let mut resp = db.query(format!("INFO FOR TABLE {table}")).await.expect("INFO");
        let info: TableInfo = resp.take::<Option<TableInfo>>(0).expect("parse").expect("some");
        assert!(
            info.fields.contains_key("vault_deleted"),
            "vault_deleted must exist on '{table}' — there is no `entity` table to define it on"
        );
    }
}

/// A row written before `003_vault_sync.surql` carries no `vault_deleted` value
/// at all. `DEFAULT false` applies at write time, not retroactively — so a
/// `= false` filter silently omits it and `!= true` is the only safe form.
#[tokio::test]
async fn default_false_does_not_backfill_pre_migration_rows() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(()).await.expect("db");
    db.use_ns("test").use_db("test").await.unwrap();

    // Simulate a pre-migration row: define the table WITHOUT vault_deleted.
    db.query(
        "DEFINE TABLE npc SCHEMALESS; \
         CREATE npc:legacy SET name = 'Legacy', created_at = time::now(), updated_at = time::now()",
    )
    .await
    .expect("seed legacy row")
    .check()
    .expect("seed response");

    // Now run the real migrations over the live database.
    run_migrations(&db).await.expect("migrations");

    let mut wrong = db
        .query("SELECT id FROM npc WHERE vault_deleted = false")
        .await
        .expect("query = false");
    let wrong_rows: Vec<serde_json::Value> = wrong.take(0).expect("take");

    let mut right = db
        .query("SELECT id FROM npc WHERE vault_deleted != true")
        .await
        .expect("query != true");
    let right_rows: Vec<serde_json::Value> = right.take(0).expect("take");

    assert_eq!(
        right_rows.len(),
        1,
        "`!= true` must see the pre-migration row"
    );
    assert!(
        wrong_rows.len() <= right_rows.len(),
        "regression guard: if `= false` ever starts matching, the `!= true` rule can be revisited"
    );
}

#[tokio::test]
async fn vault_sync_state_table_exists_and_is_schemafull() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(()).await.expect("db");
    db.use_ns("test").use_db("test").await.unwrap();
    run_migrations(&db).await.expect("migrations");

    db.query(
        "CREATE vault_sync_state:⟨npc:a⟩ SET \
         record = 'npc:a', key = 'campaigns/c/entities/npc/a.md', \
         synced_hash = '123', synced_at = time::now()",
    )
    .await
    .expect("insert sync state")
    .check()
    .expect("insert response");

    let mut resp = db.query("SELECT id FROM vault_sync_state").await.expect("select");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take");
    assert_eq!(rows.len(), 1);
}

/// The whole file must survive a second execution — `run_migrations` runs on
/// every boot. A `REMOVE` here once wiped every `relates_to` edge on restart.
#[tokio::test]
async fn vault_sync_state_rows_survive_a_migration_rerun() {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(()).await.expect("db");
    db.use_ns("test").use_db("test").await.unwrap();
    run_migrations(&db).await.expect("first migration");

    db.query(
        "CREATE vault_sync_state:⟨npc:a⟩ SET record = 'npc:a', key = 'k', \
         synced_hash = '123', synced_at = time::now()",
    )
    .await
    .expect("seed")
    .check()
    .expect("seed response");

    run_migrations(&db).await.expect("second migration (restart simulation)");

    let mut resp = db.query("SELECT id FROM vault_sync_state").await.expect("select");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take");
    assert_eq!(rows.len(), 1, "vault_sync_state must survive a migration re-run");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p chronacle-db vault_`
Expected: FAIL — `vault_deleted must exist on 'npc'`, and the `vault_sync_state` table does not exist.

- [ ] **Step 4: Write `003_vault_sync.surql`**

`synced_hash` is stored as a **string**, not an int: it is a `u64` from `DefaultHasher`, and SurrealDB's `int` is `i64`, so values above `i64::MAX` would round-trip wrong.

```surql
-- ── Vault sync (ADR-008, D-series) ───────────────────────────────────────────
-- DEFINE-only. `run_migrations` re-executes this file on every boot; a REMOVE
-- here would wipe live data on every restart.
--
-- NOTE: `DEFAULT false` does NOT backfill rows written before this migration.
-- Every read path must filter `vault_deleted != true`, never `= false`.

DEFINE FIELD OVERWRITE vault_deleted ON TABLE npc              TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE location         TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE faction          TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE creature         TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE item             TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE event            TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE player_character TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE misc             TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE vault_deleted ON TABLE session          TYPE bool DEFAULT false;

-- ── vault_sync_state: the three-way merge base ───────────────────────────────
-- One row per synced record, id = the record's thing string. `synced_hash` is a
-- u64 from std DefaultHasher, stored as a string because SurrealDB `int` is i64.
-- SCHEMAFULL with typed fields — no FLEXIBLE object, no serde_json::Value binds.

DEFINE TABLE OVERWRITE vault_sync_state SCHEMAFULL;
DEFINE FIELD OVERWRITE record      ON TABLE vault_sync_state TYPE string;
DEFINE FIELD OVERWRITE key         ON TABLE vault_sync_state TYPE string;
DEFINE FIELD OVERWRITE synced_hash ON TABLE vault_sync_state TYPE string;
DEFINE FIELD OVERWRITE synced_at   ON TABLE vault_sync_state TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE vault_sync_state_record_idx ON TABLE vault_sync_state COLUMNS record UNIQUE;
```

Update the `run_migrations` doc comment in `mod.rs` to list `003_vault_sync.surql`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p chronacle-db`
Expected: PASS — existing tests plus 4 new.

- [ ] **Step 6: Commit**

```bash
git add crates/chronacle-db
git commit -m "feat(db): vault_deleted x9 + vault_sync_state migration"
```

---

### Task 9: `SurrealVaultRecordStore` — and the D2b PR

**Files:**

- Create: `crates/chronacle-domain/src/vault_record_store.rs`
- Modify: `crates/chronacle-domain/src/lib.rs` (register module)
- Modify: `crates/chronacle-domain/Cargo.toml` (add `chronacle-core` if absent; `async-trait`)

**Interfaces:**

- Consumes: `VaultRecordStore`, `VaultRecord`, `VaultScope`, `VaultRef`, `VaultRecordError` (Task 1); the `vault_sync_state` table (Task 8).
- Produces: `SurrealVaultRecordStore::new(Surreal<Any>) -> Self` and its `VaultRecordStore` impl.

**Where scope comes from:** an entity belongs to exactly one campaign **or** one collection — `in_campaign` and `in_collection` each carry a `UNIQUE` index on `out` (`001_base_schema.surql:211-223`). Resolve it with a graph traversal per entity table.

- [ ] **Step 1: Write the failing integration tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{VaultRecord, VaultRecordStore, VaultRef, VaultScope};

    async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
        let db = surrealdb::engine::any::connect("mem://").await.expect("mem db");
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.expect("migrations");
        db
    }

    async fn seed_campaign_npc(db: &surrealdb::Surreal<surrealdb::engine::any::Any>) {
        db.query(
            "CREATE campaign:c1 SET name = 'Shadows of Valdris', system = '5e', \
                 created_at = time::now(), updated_at = time::now(); \
             CREATE npc:n1 SET name = 'Seraphina Aldric', summary = 'Archivist', \
                 notes = 'GM notes', codex_article = 'Compiled.', \
                 created_at = time::now(), updated_at = time::now(); \
             RELATE campaign:c1->in_campaign->npc:n1;",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");
    }

    #[tokio::test]
    async fn list_all_returns_entities_with_their_campaign_scope() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);

        let records = store.list_all().await.expect("list_all");
        let entity = records
            .iter()
            .find_map(|r| match r { VaultRecord::Entity(e) => Some(e), _ => None })
            .expect("one npc");

        assert_eq!(entity.vref, VaultRef { table: "npc".into(), id: "n1".into() });
        assert_eq!(entity.name, "Seraphina Aldric");
        assert_eq!(entity.codex_article.as_deref(), Some("Compiled."));
        assert!(matches!(&entity.scope, VaultScope::Campaign { name, .. } if name == "Shadows of Valdris"));
    }

    #[tokio::test]
    async fn list_all_excludes_soft_deleted_records() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        db.query("UPDATE npc:n1 SET vault_deleted = true").await.expect("soft delete");
        let store = SurrealVaultRecordStore::new(db);

        let records = store.list_all().await.expect("list_all");
        assert!(records.is_empty(), "soft-deleted records must not sync");
    }

    /// The `!= true` rule: a row created before the migration has no
    /// `vault_deleted` value, and a `= false` filter would silently drop it.
    #[tokio::test]
    async fn list_all_includes_a_record_whose_vault_deleted_is_unset() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        db.query("UPDATE npc:n1 SET vault_deleted = NONE").await.expect("unset");
        let store = SurrealVaultRecordStore::new(db);

        let records = store.list_all().await.expect("list_all");
        assert_eq!(records.len(), 1, "an unset vault_deleted must be treated as not-deleted");
    }

    #[tokio::test]
    async fn list_all_returns_rule_entries_with_collection_scope() {
        let db = db().await;
        db.query(
            "CREATE collection:k1 SET name = 'D&D 5e Core', created_at = time::now(), updated_at = time::now(); \
             CREATE rule_entry:r1 SET collection = collection:k1, name = 'Grappling', \
                 category = 'procedure', body = 'Rules text.', compiled_at = time::now();",
        )
        .await
        .expect("seed")
        .check()
        .expect("seed response");
        let store = SurrealVaultRecordStore::new(db);

        let records = store.list_all().await.expect("list_all");
        let rule = records
            .iter()
            .find_map(|r| match r { VaultRecord::RuleEntry(x) => Some(x), _ => None })
            .expect("one rule_entry");
        assert_eq!(rule.name, "Grappling");
        assert_eq!(rule.category, "procedure");
        assert!(matches!(&rule.collection, VaultScope::Collection { name, .. } if name == "D&D 5e Core"));
    }

    #[tokio::test]
    async fn synced_hash_round_trips_through_the_store() {
        let db = db().await;
        seed_campaign_npc(&db).await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef { table: "npc".into(), id: "n1".into() };

        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), None);

        // A hash above i64::MAX must survive — it is stored as a string.
        let big: u64 = u64::MAX - 7;
        store.set_synced_hash(&vref, "campaigns/c/entities/npc/a.md", big).await.expect("set");
        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), Some(big));

        store.set_synced_hash(&vref, "campaigns/c/entities/npc/a.md", 42).await.expect("update");
        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), Some(42));

        store.clear_synced_hash(&vref).await.expect("clear");
        assert_eq!(store.get_synced_hash(&vref).await.expect("get"), None);
    }

    #[tokio::test]
    async fn load_returns_none_for_a_missing_record() {
        let db = db().await;
        let store = SurrealVaultRecordStore::new(db);
        let vref = VaultRef { table: "npc".into(), id: "nope".into() };
        assert!(store.load(&vref).await.expect("load").is_none());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p chronacle-domain vault_record_store`
Expected: FAIL — `cannot find type SurrealVaultRecordStore in this scope`.

- [ ] **Step 3: Implement `SurrealVaultRecordStore`**

```rust
//! SurrealQL implementation of the `VaultRecordStore` port.
//!
//! Lives in `chronacle-domain` (not in the engine) so `chronacle-vault` stays
//! free of SurrealDB. Delegates entity semantics to `chronacle-extraction`'s
//! `entity_service` where writes are needed — `chronacle-domain` already
//! depends on `chronacle-extraction`, so no cycle arises.

use async_trait::async_trait;
use chronacle_core::{
    EntityRecord, RuleEntryRecord, SessionRecord, VaultRecord, VaultRecordError, VaultRecordStore,
    VaultRef, VaultScope,
};
use surrealdb::{engine::any::Any, Surreal};

/// The eight per-type entity tables. There is no `entity` table.
const ENTITY_TABLES: [&str; 8] = [
    "npc", "location", "faction", "creature", "item", "event", "player_character", "misc",
];

/// `VaultRecordStore` backed by the embedded SurrealDB.
pub struct SurrealVaultRecordStore {
    db: Surreal<Any>,
}

impl SurrealVaultRecordStore {
    /// Wrap a live database handle.
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }
}
```

Implementation notes, each of which a test above pins:

- `list_all` loops `ENTITY_TABLES`, running per table:

  ```surql
  SELECT id, name, summary, notes, codex_article, created_at, updated_at,
         (SELECT VALUE in FROM in_campaign  WHERE out = $parent.id)[0]  AS campaign_id,
         (SELECT VALUE in FROM in_collection WHERE out = $parent.id)[0] AS collection_id
  FROM type::table($table)
  WHERE vault_deleted != true
  ```

  then resolves scope names with one `SELECT id, name FROM campaign` + one `SELECT id, name FROM collection`, joined in Rust. **`!= true`, never `= false`.** Entities with neither edge are skipped (they are unreachable in the UI anyway).

- Sessions: `SELECT ... FROM session WHERE vault_deleted != true`, campaign from the `campaign` record link.
- Rule entries: `SELECT ... FROM rule_entry` (no `vault_deleted` field on this table — it is not soft-deletable in this tranche), collection from the `collection` link. Deserialise `page_refs` into `Vec<RulePageRef>` with `#[serde(default)]`.
- `get_synced_hash` — `SELECT synced_hash FROM vault_sync_state WHERE record = $record`, then `str::parse::<u64>()`. A row whose `synced_hash` fails to parse is treated as `None` (self-healing: the next reconcile adopts a fresh base).
- `set_synced_hash` — `UPSERT vault_sync_state SET record = $record, key = $key, synced_hash = $hash, synced_at = time::now() WHERE record = $record`; bind `hash.to_string()`. Prefer an explicit `UPSERT vault_sync_state:⟨{record}⟩` by id so the UNIQUE index is never contended.
- `clear_synced_hash` — `DELETE vault_sync_state WHERE record = $record`.
- Every `db.query(...)` is followed by `.check()` on the response — a SurrealDB statement can fail _inside_ an otherwise-`Ok` response, and a missing `.check()` is exactly the bug the C2a review caught.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p chronacle-domain vault_record_store`
Expected: PASS — 6 tests.

- [ ] **Step 5: Full CI gate, commit, open the D2b PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend

git add crates/chronacle-db crates/chronacle-domain
git commit -m "feat(vault): SurrealVaultRecordStore + sync-state base"
git push -u origin feat/d2b-record-store:refs/heads/feat/d2b-record-store
gh pr create --base feat/d2a-fs-store --title "feat(vault): D2b — record store + migration" --body "$(cat <<'EOF'
## What
`003_vault_sync.surql`: `vault_deleted` on all nine syncable tables (there is no
`entity` table) plus the `vault_sync_state` merge-base table. And
`SurrealVaultRecordStore`, the SurrealQL side of the `VaultRecordStore` port.

## Why
`DEFAULT false` does not backfill rows written before the migration, so every
read path filters `vault_deleted != true` — a `= false` filter silently omits
every pre-existing entity. A regression test pins this.

`synced_hash` is stored as a string: it is a `u64`, and SurrealDB's `int` is
`i64`, so `u64::MAX - 7` would round-trip wrong as an int.

## Testing
`cargo test -p chronacle-db` (4 new: field-on-nine-tables, no-backfill,
sync-state exists, survives a migration re-run) and
`cargo test -p chronacle-domain vault_record_store` (6, on `mem://`).
Full CI gate green including `cargo deny check`.

Stacked on #<D2a>.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 10: `render` + `decide` — the pure core

**Files:**

- Create: `crates/chronacle-vault/src/render.rs`
- Create: `crates/chronacle-vault/src/decide.rs`
- Modify: `crates/chronacle-vault/src/lib.rs` (register modules)

**Interfaces:**

- Consumes: `frontmatter::render` (Task 3), `markdown::{render_body, normalize, BodyParts}` (Task 4), `VaultRecord` (Task 1).
- Produces: `render::render_record(&VaultRecord) -> String`, `render::content_hash(&str) -> u64`, `decide::SyncAction`, `decide::decide(Option<u64>, u64, Option<u64>) -> SyncAction`.

**`decide` is the whole sync algorithm, and it is a pure function of three hashes.** No clock, no disk, no database. Every interesting state — including the crash-recovery case — is a table row in its test.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d3a-reconcile feat/d2b-record-store
```

- [ ] **Step 2: Write the failing tests for `decide`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Hash values are opaque; only their equality matters.
    const BASE: u64 = 100;
    const CHANGED: u64 = 200;
    const OTHER: u64 = 300;

    #[test]
    fn nothing_changed_is_a_noop() {
        assert_eq!(decide(Some(BASE), BASE, Some(BASE)), SyncAction::NoOp);
    }

    #[test]
    fn db_changed_and_file_untouched_exports() {
        assert_eq!(decide(Some(BASE), CHANGED, Some(BASE)), SyncAction::Export);
    }

    #[test]
    fn file_changed_and_db_untouched_applies() {
        assert_eq!(decide(Some(BASE), BASE, Some(CHANGED)), SyncAction::Apply);
    }

    #[test]
    fn both_changed_differently_is_a_conflict() {
        assert_eq!(decide(Some(BASE), CHANGED, Some(OTHER)), SyncAction::Conflict);
    }

    /// Crash recovery: the app died between `VaultStore::write` and the
    /// `synced_hash` update. Both sides diverge from the base, but they agree
    /// with each other — that is not a conflict, it is a stale base.
    #[test]
    fn both_changed_identically_adopts_the_base_and_never_conflicts() {
        assert_eq!(decide(Some(BASE), CHANGED, Some(CHANGED)), SyncAction::AdoptBase);
    }

    #[test]
    fn no_base_and_no_file_is_a_first_export() {
        assert_eq!(decide(None, CHANGED, None), SyncAction::Export);
    }

    #[test]
    fn no_base_but_an_identical_file_adopts_the_base() {
        // e.g. the GM restored a vault backup that already matches the DB.
        assert_eq!(decide(None, CHANGED, Some(CHANGED)), SyncAction::AdoptBase);
    }

    #[test]
    fn no_base_and_a_differing_file_is_a_conflict() {
        // An unmanaged pre-existing file already claims this id. Never clobber it.
        assert_eq!(decide(None, CHANGED, Some(OTHER)), SyncAction::Conflict);
    }

    #[test]
    fn a_base_with_no_file_is_a_soft_delete() {
        // We wrote it once; it is gone now. The GM deleted it in the vault.
        assert_eq!(decide(Some(BASE), BASE, None), SyncAction::SoftDelete);
        assert_eq!(decide(Some(BASE), CHANGED, None), SyncAction::SoftDelete);
    }

    /// The decision never consults a clock. This is a compile-time property —
    /// the signature takes three hashes — but assert the shape anyway.
    #[test]
    fn decide_is_total_over_every_combination() {
        for base in [None, Some(BASE)] {
            for file in [None, Some(BASE), Some(CHANGED)] {
                for db in [BASE, CHANGED] {
                    let _ = decide(base, db, file); // must not panic
                }
            }
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p chronacle-vault decide`
Expected: FAIL — `cannot find function decide in this scope`.

- [ ] **Step 4: Implement `decide.rs`**

Note the ordering: the `db == file` short-circuit must precede the conflict branch, or crash recovery raises a conflict whose two sides are identical.

```rust
//! The three-way sync decision. Pure: a function of three content hashes.
//!
//! Timestamps are deliberately absent. `codex_service::compile.rs` updates
//! `codex_article` without touching `updated_at`, so an `mtime`-vs-`updated_at`
//! comparison would never re-export a recompiled article. And a timestamp delta
//! cannot distinguish "the file is a stale copy" from "the file has divergent
//! edits" — that needs a base.

/// What reconcile should do about one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    /// Both sides match the base. Nothing to do.
    NoOp,
    /// Both sides agree with each other but not with the base. Record the base.
    AdoptBase,
    /// The database changed; the file did not. Write the file.
    Export,
    /// The file changed; the database did not. Apply inbound. (Tranche 5.)
    Apply,
    /// Both changed, differently. Preserve both. (Tranche 5.)
    Conflict,
    /// We wrote this file once and it is gone. (Tranche 5.)
    SoftDelete,
}

/// Decide what to do, given the last-synced hash, the rendered-record hash, and
/// the file's content hash (`None` when the key is absent from the vault).
pub fn decide(base: Option<u64>, db: u64, file: Option<u64>) -> SyncAction {
    let Some(file) = file else {
        // No file. If we never wrote one, this is the first export. If we did,
        // the GM deleted it.
        return match base {
            None => SyncAction::Export,
            Some(_) => SyncAction::SoftDelete,
        };
    };

    // MUST precede the conflict branch: after a crash between write and
    // base-update, both sides differ from the base but agree with each other.
    if db == file {
        return if base == Some(db) { SyncAction::NoOp } else { SyncAction::AdoptBase };
    }

    match base {
        // An unmanaged file already claims this id and differs from us.
        None => SyncAction::Conflict,
        Some(base) => match (db == base, file == base) {
            (true, true) => unreachable!("db == file was handled above"),
            (false, true) => SyncAction::Export,
            (true, false) => SyncAction::Apply,
            (false, false) => SyncAction::Conflict,
        },
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p chronacle-vault decide`
Expected: PASS — 10 tests.

- [ ] **Step 6: Write the failing tests for `render`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{EntityRecord, SessionRecord, VaultRecord, VaultRef, VaultScope};

    fn npc() -> VaultRecord {
        VaultRecord::Entity(EntityRecord {
            vref: VaultRef { table: "npc".into(), id: "abc123".into() },
            name: "Seraphina Aldric".into(),
            summary: Some("Archivist of the Iron Tower.".into()),
            notes: Some("GM notes.".into()),
            codex_article: Some("Seraphina is the archivist of [[The Iron Tower]].".into()),
            scope: VaultScope::Campaign { id: "campaign:c1".into(), name: "Shadows of Valdris".into() },
            created_at: "2026-05-28T14:00:00Z".into(),
            updated_at: "2026-07-09T18:32:00Z".into(),
        })
    }

    #[test]
    fn render_record_emits_aliases_so_obsidian_wikilinks_resolve() {
        let out = render_record(&npc());
        assert!(out.contains(r#"aliases: ["Seraphina Aldric"]"#), "got:\n{out}");
        assert!(out.contains(r#"title: "Seraphina Aldric""#));
    }

    #[test]
    fn render_record_fences_the_compiled_article() {
        let out = render_record(&npc());
        assert!(out.contains(crate::markdown::FENCE_START));
        assert!(out.contains("[[The Iron Tower]]"));
        assert!(out.contains(crate::markdown::FENCE_END));
    }

    #[test]
    fn render_record_never_emits_is_gm_only() {
        // The manual flag was built and reverted; GM-secret is Phase 3.
        assert!(!render_record(&npc()).contains("is_gm_only"));
    }

    #[test]
    fn render_record_of_an_entity_with_no_article_omits_the_fence() {
        let VaultRecord::Entity(mut e) = npc() else { unreachable!() };
        e.codex_article = None;
        let out = render_record(&VaultRecord::Entity(e));
        assert!(!out.contains(crate::markdown::FENCE_START));
    }

    #[test]
    fn render_record_of_a_session_has_no_fence_and_carries_title() {
        let rec = VaultRecord::Session(SessionRecord {
            vref: VaultRef { table: "session".into(), id: "s1".into() },
            session_number: 1, title: "The Awakening".into(),
            date_played: "2026-01-01".into(), notes: "Recap.".into(),
            campaign: VaultScope::Campaign { id: "campaign:c1".into(), name: "SoV".into() },
            created_at: "x".into(), updated_at: "y".into(),
        });
        let out = render_record(&rec);
        assert!(!out.contains(crate::markdown::FENCE_START), "sessions have no compiled body");
        assert!(out.contains(r#"title: "The Awakening""#));
        assert!(out.contains("session_number: 1"));
        assert!(out.contains("Recap."));
    }

    #[test]
    fn content_hash_ignores_trailing_newlines_and_crlf() {
        assert_eq!(content_hash("body"), content_hash("body\n"));
        assert_eq!(content_hash("a\nb"), content_hash("a\r\nb\r\n"));
    }

    #[test]
    fn content_hash_distinguishes_different_content() {
        assert_ne!(content_hash("a"), content_hash("b"));
    }

    #[test]
    fn render_record_is_deterministic() {
        assert_eq!(render_record(&npc()), render_record(&npc()));
    }
}
```

- [ ] **Step 7: Implement `render.rs`**

```rust
//! Render a record to its full vault file, and hash file content.

use std::hash::{Hash, Hasher};

use chronacle_core::{VaultRecord, VaultScope};

use crate::frontmatter::Frontmatter;
use crate::markdown::{self, BodyParts};

/// Hash normalized content. A merge/loop guard, **not** a security primitive —
/// which is why this is `std`'s `DefaultHasher` and not a new crate dependency.
pub fn content_hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    markdown::normalize(s).hash(&mut h);
    h.finish()
}

/// Render a record to its complete `.md` file: frontmatter plus body.
pub fn render_record(record: &VaultRecord) -> String {
    let (fm, parts) = match record {
        VaultRecord::Entity(e) => (
            Frontmatter {
                id: e.vref.to_thing(),
                name: Some(e.name.clone()),
                title: Some(e.name.clone()),
                // Without this, `[[Seraphina Aldric]]` in a compiled article
                // renders broken: wikilinks resolve against `name`, files are
                // slug-named.
                aliases: vec![e.name.clone()],
                kind: Some(e.vref.table.clone()),
                campaign: scope_campaign_name(&e.scope),
                collection: scope_collection_name(&e.scope),
                category: None,
                session_number: None,
                date_played: None,
                page_refs: vec![],
                created_at: e.created_at.clone(),
                updated_at: e.updated_at.clone(),
            },
            BodyParts {
                summary: e.summary.clone(),
                fenced: e.codex_article.clone(),
                notes: e.notes.clone(),
            },
        ),
        // Sessions carry no compiled body, so there is no fence: the whole
        // body is GM-owned `notes`. `title` is both our field and Obsidian's.
        VaultRecord::Session(s) => (
            Frontmatter {
                id: s.vref.to_thing(),
                name: None,
                title: Some(s.title.clone()),
                aliases: vec![s.title.clone()],
                kind: None,
                campaign: scope_campaign_name(&s.campaign),
                collection: None,
                category: None,
                session_number: Some(s.session_number),
                date_played: Some(s.date_played.clone()),
                page_refs: vec![],
                created_at: s.created_at.clone(),
                updated_at: s.updated_at.clone(),
            },
            BodyParts { summary: None, fenced: None, notes: Some(s.notes.clone()) },
        ),
        // Rule entries mirror the entity split: `body` is compiler-owned and
        // fenced; `notes` is GM-owned. `page_refs` is read-only provenance.
        VaultRecord::RuleEntry(r) => (
            Frontmatter {
                id: r.vref.to_thing(),
                name: Some(r.name.clone()),
                title: Some(r.name.clone()),
                aliases: vec![r.name.clone()],
                kind: None,
                campaign: None,
                collection: scope_collection_name(&r.collection),
                category: Some(r.category.clone()),
                session_number: None,
                date_played: None,
                page_refs: r.page_refs.clone(),
                created_at: r.created_at.clone(),
                updated_at: r.updated_at.clone(),
            },
            BodyParts { summary: None, fenced: Some(r.body.clone()), notes: r.notes.clone() },
        ),
    };
    format!("{}{}", crate::frontmatter::render(&fm), markdown::render_body(&parts))
}

/// The campaign name, when this scope is a campaign.
fn scope_campaign_name(scope: &VaultScope) -> Option<String> {
    match scope {
        VaultScope::Campaign { name, .. } => Some(name.clone()),
        VaultScope::Collection { .. } => None,
    }
}

/// The collection name, when this scope is a collection.
fn scope_collection_name(scope: &VaultScope) -> Option<String> {
    match scope {
        VaultScope::Collection { name, .. } => Some(name.clone()),
        VaultScope::Campaign { .. } => None,
    }
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p chronacle-vault`
Expected: PASS — 35 (prior) + 10 (decide) + 8 (render) = 53 tests.

- [ ] **Step 9: Commit**

```bash
git add crates/chronacle-vault
git commit -m "feat(vault): pure three-way decide + record render"
```

---

### Task 11: `VaultSyncService::reconcile` — export direction

**Files:**

- Create: `crates/chronacle-vault/src/reconcile.rs`
- Modify: `crates/chronacle-vault/src/lib.rs` (register module)

**Interfaces:**

- Consumes: `VaultStore`, `VaultRecordStore` (Task 1); `VaultIndex`, `key_for` (Tasks 5–6); `render_record`, `content_hash` (Task 10); `decide`, `SyncAction` (Task 10).
- Produces: `VaultSyncService::new(Arc<dyn VaultStore>, Arc<dyn VaultRecordStore>) -> Self`, `VaultSyncService::reconcile() -> Result<ReconcileReport, VaultError>`, `VaultSyncService::export_one(&VaultRef) -> Result<(), VaultError>`, `ReconcileReport`.

**Export direction only.** `Apply`, `Conflict`, and `SoftDelete` are computed, counted into the report, logged at `warn`, and otherwise **not acted on**. Tranche 5 turns them on. This is what lets D3a ship a releasable one-way export.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{
        EntityRecord, MockVaultRecordStore, MockVaultStore, VaultRecord, VaultRef, VaultScope,
        VaultStoreError,
    };
    use std::sync::Arc;

    fn npc(article: Option<&str>) -> VaultRecord {
        VaultRecord::Entity(EntityRecord {
            vref: VaultRef { table: "npc".into(), id: "n1".into() },
            name: "Seraphina".into(), summary: None, notes: Some("N.".into()),
            codex_article: article.map(str::to_owned),
            scope: VaultScope::Campaign { id: "campaign:c1".into(), name: "SoV".into() },
            created_at: "x".into(), updated_at: "y".into(),
        })
    }
    const KEY: &str = "campaigns/sov/entities/npc/seraphina.md";

    #[tokio::test]
    async fn reconcile_exports_a_record_that_has_never_synced() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store.expect_write().withf(|k, _| k == KEY).times(1).returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_get_synced_hash().returning(|_| Ok(None));
        records.expect_set_synced_hash().times(1).returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.exported, 1);
    }

    #[tokio::test]
    async fn reconcile_is_a_noop_when_nothing_changed() {
        let rendered = crate::render::render_record(&npc(Some("A.")));
        let h = crate::render::content_hash(&rendered);

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(rendered.clone()));
        store.expect_write().never();          // the point of the test
        store.expect_delete().never();

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_get_synced_hash().returning(move |_| Ok(Some(h)));
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.exported, 0);
    }

    /// A recompiled article changes `codex_article` but NOT `updated_at`
    /// (`compile.rs:220-224`). A timestamp-driven reconcile would miss it.
    #[tokio::test]
    async fn reconcile_reexports_a_recompiled_article_despite_unchanged_updated_at() {
        let old = crate::render::render_record(&npc(Some("OLD.")));
        let old_hash = crate::render::content_hash(&old);

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(old.clone()));
        store.expect_write().times(1).returning(|_, _| Ok(()));

        let mut records = MockVaultRecordStore::new();
        // Same updated_at ("y"), different article.
        records.expect_list_all().returning(|| Ok(vec![npc(Some("NEW."))]));
        records.expect_get_synced_hash().returning(move |_| Ok(Some(old_hash)));
        records.expect_set_synced_hash().times(1).returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        assert_eq!(svc.reconcile().await.expect("reconcile").exported, 1);
    }

    /// Crash between write and base-update: the file already equals the DB.
    /// Adopt the base; never raise a conflict, never rewrite.
    #[tokio::test]
    async fn reconcile_adopts_a_stale_base_without_conflicting() {
        let rendered = crate::render::render_record(&npc(Some("A.")));

        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(move |_| Ok(rendered.clone()));
        store.expect_write().never();

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_get_synced_hash().returning(|_| Ok(Some(999_999))); // stale
        records.expect_set_synced_hash().times(1).returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.adopted, 1);
        assert_eq!(report.deferred_conflict, 0, "identical sides are not a conflict");
    }

    #[tokio::test]
    async fn reconcile_defers_apply_and_conflict_without_writing() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![KEY.to_string()]));
        store.expect_read().returning(|_| Ok("---\nid: \"npc:n1\"\ncreated_at: \"x\"\nupdated_at: \"y\"\n---\n\nGM edited this.\n".to_string()));
        store.expect_write().never();  // export-only tranche must not clobber
        store.expect_delete().never();

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        // db == base, file differs => Apply (deferred to tranche 5)
        records.expect_get_synced_hash().returning(|_| {
            Ok(Some(crate::render::content_hash(&crate::render::render_record(&npc(Some("A."))))))
        });
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.deferred_apply, 1);
        assert_eq!(report.exported, 0);
    }

    #[tokio::test]
    async fn reconcile_defers_soft_delete_and_does_not_resurrect_the_file() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));   // file is gone
        store.expect_write().never();                    // MUST NOT rewrite it

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_get_synced_hash().returning(|_| Ok(Some(123)));  // we wrote it once
        records.expect_set_synced_hash().never();

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("reconcile");
        assert_eq!(report.deferred_delete, 1);
        assert_eq!(report.exported, 0, "reconcile must never resurrect a deleted file");
    }

    #[tokio::test]
    async fn reconcile_suffixes_colliding_slugs() {
        let a = VaultRecord::Entity(EntityRecord {
            vref: VaultRef { table: "npc".into(), id: "aaa".into() },
            name: "Guard".into(), summary: None, notes: None, codex_article: None,
            scope: VaultScope::Campaign { id: "campaign:c1".into(), name: "SoV".into() },
            created_at: "x".into(), updated_at: "y".into(),
        });
        let b = VaultRecord::Entity(EntityRecord {
            vref: VaultRef { table: "npc".into(), id: "bbb".into() },
            name: "Guard".into(), summary: None, notes: None, codex_article: None,
            scope: VaultScope::Campaign { id: "campaign:c1".into(), name: "SoV".into() },
            created_at: "x".into(), updated_at: "y".into(),
        });

        let written = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let w = Arc::clone(&written);
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store.expect_write().returning(move |k, _| { w.lock().unwrap().push(k.to_string()); Ok(()) });

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(move || Ok(vec![a.clone(), b.clone()]));
        records.expect_get_synced_hash().returning(|_| Ok(None));
        records.expect_set_synced_hash().returning(|_, _, _| Ok(()));

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        svc.reconcile().await.expect("reconcile");

        let keys = written.lock().unwrap().clone();
        assert_eq!(keys.len(), 2);
        assert_ne!(keys[0], keys[1], "colliding names must not share a key");
    }

    #[tokio::test]
    async fn reconcile_reports_an_io_failure_without_aborting_the_run() {
        let mut store = MockVaultStore::new();
        store.expect_list().returning(|_| Ok(vec![]));
        store.expect_write().returning(|_, _| Err(VaultStoreError::Io("disk full".into())));

        let mut records = MockVaultRecordStore::new();
        records.expect_list_all().returning(|| Ok(vec![npc(Some("A."))]));
        records.expect_get_synced_hash().returning(|_| Ok(None));
        records.expect_set_synced_hash().never();  // never claim a base we did not write

        let svc = VaultSyncService::new(Arc::new(store), Arc::new(records));
        let report = svc.reconcile().await.expect("a failing key must not fail the run");
        assert_eq!(report.exported, 0);
        assert_eq!(report.failed, 1);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p chronacle-vault reconcile`
Expected: FAIL — `cannot find type VaultSyncService in this scope`.

- [ ] **Step 3: Implement `reconcile.rs`**

```rust
//! Bidirectional reconcile — export direction only in this tranche.
//!
//! Reconcile is the **correctness guarantee**; the outbound queue (D4a) is only
//! a latency optimisation. A dropped `enqueue()` degrades to "the file updates
//! on next reconcile", never to "the file is permanently wrong". That is also
//! why a backend with no change feed (S3, WebDAV) is still correct.

use std::sync::Arc;

use chronacle_core::{VaultRecord, VaultRecordStore, VaultRef, VaultStore, VaultStoreError};

use crate::decide::{decide, SyncAction};
use crate::keys::{key_for, VaultIndex};
use crate::render::{content_hash, render_record};
use crate::VaultError;

/// Outcome counts for one reconcile pass.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    /// Files written.
    pub exported: usize,
    /// Records already in sync.
    pub unchanged: usize,
    /// Stale merge bases adopted (crash recovery).
    pub adopted: usize,
    /// Inbound edits seen but not applied (tranche 5).
    pub deferred_apply: usize,
    /// Divergent edits seen but not materialised (tranche 5).
    pub deferred_conflict: usize,
    /// Vault deletions seen but not soft-deleted (tranche 5).
    pub deferred_delete: usize,
    /// Keys that failed to write. The run continues.
    pub failed: usize,
}

/// The vault sync engine.
pub struct VaultSyncService {
    store: Arc<dyn VaultStore>,
    records: Arc<dyn VaultRecordStore>,
}

impl VaultSyncService {
    /// Construct the engine over a storage backend and a record backend.
    pub fn new(store: Arc<dyn VaultStore>, records: Arc<dyn VaultRecordStore>) -> Self {
        Self { store, records }
    }
}
```

`reconcile()` body:

1. `let index = VaultIndex::scan(self.store.as_ref()).await?;`
2. `let records = self.records.list_all().await?;`
3. Build a collision set: group records by their _un-suffixed_ key; any key claimed by more than one record marks all of them `collides = true`.
4. For each record:
   - `let rendered = render_record(&record);` `let db = content_hash(&rendered);`
   - `let key = index.key_of(vref).cloned().unwrap_or_else(|| key_for(&record, collides));` — **the index wins**, so a file the GM renamed keeps its name.
   - `let file = match index.key_of(vref) { Some(k) => Some(content_hash(&self.store.read(k).await?)), None => None };`
   - `let base = self.records.get_synced_hash(vref).await?;`
   - `match decide(base, db, file)`:
     - `NoOp` → `report.unchanged += 1`
     - `AdoptBase` → `set_synced_hash(vref, &key, db)`, `report.adopted += 1`
     - `Export` → `self.store.write(&key, &rendered)`; on `Ok`, `set_synced_hash(vref, &key, db)` and `report.exported += 1`; on `Err`, `tracing::warn!` and `report.failed += 1` and **do not** set the base — `continue`.
     - `Apply` / `Conflict` / `SoftDelete` → `tracing::warn!(?vref, ?action, "vault: inbound action deferred to tranche 5")` and bump the matching `deferred_*` counter. **No write, no delete, no base update.**
5. Return the report.

`export_one(vref)` loads one record, renders it, writes it, and sets the base — the single-record path D4a's drain task calls. On a missing record it is `Ok(())` (the record was deleted between enqueue and drain).

Use `tracing` if the workspace already depends on it; otherwise `eprintln!` is acceptable and no new crate is added.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p chronacle-vault reconcile`
Expected: PASS — 8 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/chronacle-vault
git commit -m "feat(vault): reconcile export direction with deferred inbound"
```

---

### Task 12: Tauri commands `get_vault_path` / `set_vault_path` / `vault_sync_now` — and the D3a PR

**Files:**

- Create: `apps/desktop/src-tauri/src/commands/vault_commands.rs`
- Modify: `apps/desktop/src-tauri/src/commands/mod.rs` (register module)
- Modify: `apps/desktop/src-tauri/src/lib.rs` (`AppState` gains `vault: RwLock<Option<Arc<VaultSyncService>>>`; register the three commands in `invoke_handler`; build the service at startup if `vault_sync_path` is set)
- Modify: `apps/desktop/src-tauri/Cargo.toml` (add `chronacle-vault`)
- Create: `apps/desktop/src-tauri/tests/vault_reconcile_test.rs`

**Interfaces:**

- Consumes: `VaultSyncService`, `ReconcileReport` (Task 11); `LocalFsVaultStore` (Task 7); `SurrealVaultRecordStore` (Task 9); `settings_service::upsert` (existing).
- Produces: Tauri commands `get_vault_path() -> Option<String>`, `set_vault_path(vault_path: Option<String>) -> ()`, `vault_sync_now() -> ReconcileReport`.

- [ ] **Step 1: Write the failing integration test**

`apps/desktop/src-tauri/tests/vault_reconcile_test.rs` — service-layer, no Tauri runtime, per the standing pattern.

```rust
//! End-to-end reconcile over a real temp vault and a real in-memory database.

use std::sync::Arc;

use chronacle_domain::vault_record_store::SurrealVaultRecordStore;
use chronacle_providers::vault_store::LocalFsVaultStore;
use chronacle_vault::reconcile::VaultSyncService;
use tempfile::TempDir;

async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://").await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db
}

/// Gherkin: Given a campaign with entities and no vault configured, when the GM
/// sets a vault path, then a full reconcile writes one .md per record, and each
/// entity file carries `aliases` matching its name.
#[tokio::test]
async fn reconcile_writes_one_file_per_record_with_resolving_aliases() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'Shadows of Valdris', system = '5e', \
             created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'Seraphina Aldric', notes = 'GM notes', \
             codex_article = 'She guards [[The Iron Tower]].', \
             created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    )
    .await
    .expect("seed")
    .check()
    .expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );

    let report = svc.reconcile().await.expect("reconcile");
    assert_eq!(report.exported, 1);

    let path = dir.path().join("campaigns/shadows-of-valdris/entities/npc/seraphina-aldric.md");
    let content = std::fs::read_to_string(&path).expect("file must exist at the derived key");
    assert!(content.contains(r#"id: "npc:n1""#));
    assert!(content.contains(r#"aliases: ["Seraphina Aldric"]"#), "wikilinks would break without this");
    assert!(content.contains("[[The Iron Tower]]"));
    assert!(!content.contains("is_gm_only"));
}

/// Gherkin: Given a configured vault and no changes, when the GM clicks
/// "Sync now", then no file contents change.
#[tokio::test]
async fn a_second_reconcile_writes_nothing() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'C', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'A', created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    ).await.expect("seed").check().expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );

    assert_eq!(svc.reconcile().await.expect("first").exported, 1);
    let second = svc.reconcile().await.expect("second");
    assert_eq!(second.exported, 0);
    assert_eq!(second.unchanged, 1);
}

/// Gherkin: Given a record with vault_deleted = TRUE, when reconcile runs,
/// then no file is written for it.
#[tokio::test]
async fn reconcile_skips_soft_deleted_records() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'C', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE npc:n1 SET name = 'A', vault_deleted = true, created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    ).await.expect("seed").check().expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );
    assert_eq!(svc.reconcile().await.expect("reconcile").exported, 0);
    assert!(
        !dir.path().join("campaigns/c/entities/npc/a.md").exists(),
        "a soft-deleted record must never be written to the vault"
    );
}

/// Gherkin: Given a collection subscribed to two campaigns, when reconcile
/// runs, then its entities appear exactly once, under collections/<slug>/.
#[tokio::test]
async fn a_shared_collection_entity_is_written_once_under_collections() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name = 'One', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE campaign:c2 SET name = 'Two', system = '5e', created_at = time::now(), updated_at = time::now(); \
         CREATE collection:k1 SET name = 'Core', created_at = time::now(), updated_at = time::now(); \
         RELATE campaign:c1->subscribes_to->collection:k1 SET created_at = time::now(); \
         RELATE campaign:c2->subscribes_to->collection:k1 SET created_at = time::now(); \
         CREATE creature:g1 SET name = 'Goblin', created_at = time::now(), updated_at = time::now(); \
         RELATE collection:k1->in_collection->creature:g1;",
    ).await.expect("seed").check().expect("seed response");

    let dir = TempDir::new().expect("tempdir");
    let svc = VaultSyncService::new(
        Arc::new(LocalFsVaultStore::new(dir.path())),
        Arc::new(SurrealVaultRecordStore::new(db)),
    );
    assert_eq!(svc.reconcile().await.expect("reconcile").exported, 1);
    assert!(dir.path().join("collections/core/entities/creature/goblin.md").exists());
    assert!(!dir.path().join("campaigns/one/entities/creature/goblin.md").exists());
    assert!(!dir.path().join("campaigns/two/entities/creature/goblin.md").exists());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p chronacle --test vault_reconcile_test`
Expected: FAIL — unresolved import `chronacle_vault`.

- [ ] **Step 3: Add `chronacle-vault` to the app and extend `AppState`**

`apps/desktop/src-tauri/Cargo.toml`: `chronacle-vault = { path = "../../../crates/chronacle-vault" }`.

In `apps/desktop/src-tauri/src/lib.rs`, add to `AppState`:

```rust
/// Vault sync engine. `None` until `vault_sync_path` is configured.
pub vault: tokio::sync::RwLock<Option<Arc<chronacle_vault::reconcile::VaultSyncService>>>,
```

Add a helper that builds the service from a path, and call it at startup when the `vault_sync_path` setting is present:

```rust
/// Construct the vault sync engine over a filesystem root.
fn build_vault_service(
    db: surrealdb::Surreal<surrealdb::engine::any::Any>,
    root: &str,
) -> Arc<chronacle_vault::reconcile::VaultSyncService> {
    Arc::new(chronacle_vault::reconcile::VaultSyncService::new(
        Arc::new(chronacle_providers::vault_store::LocalFsVaultStore::new(root)),
        Arc::new(chronacle_domain::vault_record_store::SurrealVaultRecordStore::new(db)),
    ))
}
```

- [ ] **Step 4: Write `vault_commands.rs`**

```rust
//! Vault sync commands — configure the vault root and run a reconcile.

use std::sync::Arc;

use crate::AppState;
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

impl From<ReconcileReport> for ReconcileReportDto { /* field-for-field */ }

/// The configured vault root, or `None` when vault sync is off.
#[tauri::command]
pub async fn get_vault_path(state: State<'_, Arc<AppState>>) -> Result<Option<String>, String> {
    // read setting `vault_sync_path`
}

/// Set or clear the vault root. Setting a path constructs the engine and runs a
/// full reconcile immediately; clearing it drops the engine.
#[tauri::command]
pub async fn set_vault_path(
    state: State<'_, Arc<AppState>>,
    vault_path: Option<String>,
) -> Result<(), String> {
    // upsert setting; rebuild or drop state.vault; if Some(_), reconcile once
}

/// Run a full reconcile now. Errors when no vault is configured.
#[tauri::command]
pub async fn vault_sync_now(
    state: State<'_, Arc<AppState>>,
) -> Result<ReconcileReportDto, String> {
    let guard = state.vault.read().await;
    let svc = guard.as_ref().ok_or("No vault path configured")?;
    svc.reconcile().await.map(Into::into).map_err(|e| e.to_string())
}
```

Register all three in `invoke_handler![...]` and add `pub mod vault_commands;` to `commands/mod.rs`.

**Note the camelCase rule:** the frontend calls `invoke('set_vault_path', { vaultPath })`; Tauri maps `vaultPath` → `vault_path`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p chronacle --test vault_reconcile_test`
Expected: PASS — 4 tests.

- [ ] **Step 6: Full CI gate, commit, open the D3a PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend

git add crates/chronacle-vault apps/desktop/src-tauri Cargo.lock
git commit -m "feat(vault): reconcile commands + app wiring"
git push -u origin feat/d3a-reconcile:refs/heads/feat/d3a-reconcile
gh pr create --base feat/d2b-record-store --title "feat(vault): D3a — three-way reconcile (export)" --body "$(cat <<'EOF'
## What
The pure three-way `decide(base, db, file)` function, record rendering, the
reconcile pass, and the `get_vault_path` / `set_vault_path` / `vault_sync_now`
commands. **Export direction only** — `Apply`/`Conflict`/`SoftDelete` are
computed, counted, and logged, but never acted on.

## Why
Sync is content-hash based, not timestamp based. `compile.rs:220-224` rewrites
`codex_article` without touching `updated_at`, so an `mtime`-vs-`updated_at`
reconcile would leave every recompiled article permanently stale in the vault.
A stored `synced_hash` merge base also makes conflict detection exact rather
than a 5-second window, and makes reconcile the correctness guarantee — so a
backend with no change feed (S3, WebDAV) is still correct.

Shipping export-only means this PR is releasable on its own: a one-way Obsidian
export, with the decision table it will later depend on already under test.

## Testing
`cargo test -p chronacle-vault` — 61 unit tests, all against `MockVaultStore` /
`MockVaultRecordStore`: no clock, no disk. Includes the recompiled-article case
(unchanged `updated_at`), crash recovery (identical sides must not conflict),
soft-delete-must-not-resurrect, collision suffixing, and an I/O failure that
must not claim a merge base it never wrote.
`cargo test -p chronacle --test vault_reconcile_test` — 4 acceptance tests
mirroring the D3 Gherkin, over a real temp dir and `mem://`.
Full CI gate green including `cargo deny check`.

Stacked on #<D2b>.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 13: Settings UI — vault path picker + "Sync now" — the D3b PR

**Files:**

- Modify: `apps/desktop/src/lib/commands.ts` (invoke wrappers)
- Create: `apps/desktop/src/components/VaultSyncSettings.svelte`
- Modify: `apps/desktop/src/views/SettingsView.svelte` (mount the section)
- Create: `apps/desktop/src/components/VaultSyncSettings.test.ts`
- Create: `apps/desktop/tests/e2e/features/vault-sync.feature`
- Create: `apps/desktop/tests/e2e/backend/steps/vault-sync.steps.ts`

**Interfaces:**

- Consumes: the three Tauri commands (Task 12).
- Produces: `getVaultPath()`, `setVaultPath(vaultPath: string | null)`, `vaultSyncNow()` in `commands.ts`; the `VaultSyncSettings` component.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d3b-settings-ui feat/d3a-reconcile
```

- [ ] **Step 2: Add the invoke wrappers**

In `apps/desktop/src/lib/commands.ts`, matching the file's existing style:

```ts
export interface ReconcileReport {
  exported: number;
  unchanged: number;
  adopted: number;
  deferred_apply: number;
  deferred_conflict: number;
  deferred_delete: number;
  failed: number;
}

/** The configured vault root, or null when vault sync is off. */
export function getVaultPath(): Promise<string | null> {
  return invoke("get_vault_path");
}

/** Set or clear the vault root. Setting a path runs a full reconcile. */
export function setVaultPath(vaultPath: string | null): Promise<void> {
  return invoke("set_vault_path", { vaultPath });
}

/** Run a full reconcile now. */
export function vaultSyncNow(): Promise<ReconcileReport> {
  return invoke("vault_sync_now");
}
```

- [ ] **Step 3: Write the failing component tests**

`apps/desktop/src/components/VaultSyncSettings.test.ts`, following the existing Vitest + `@testing-library/svelte` + `msw`-mocked-invoke pattern used by the other settings tests.

```ts
import { render, screen, waitFor } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import VaultSyncSettings from "./VaultSyncSettings.svelte";
import * as commands from "../lib/commands";

describe("VaultSyncSettings", () => {
  beforeEach(() => vi.restoreAllMocks());

  it('shows "not configured" when no vault path is set', async () => {
    vi.spyOn(commands, "getVaultPath").mockResolvedValue(null);
    render(VaultSyncSettings);
    expect(await screen.findByText(/no vault configured/i)).toBeInTheDocument();
  });

  it("shows the configured path and enables Sync now", async () => {
    vi.spyOn(commands, "getVaultPath").mockResolvedValue("/Users/gm/Vault");
    render(VaultSyncSettings);
    expect(await screen.findByText("/Users/gm/Vault")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /sync now/i })).toBeEnabled();
  });

  it("disables Sync now when no vault is configured", async () => {
    vi.spyOn(commands, "getVaultPath").mockResolvedValue(null);
    render(VaultSyncSettings);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /sync now/i })).toBeDisabled(),
    );
  });

  it("reports the export count after a successful sync", async () => {
    vi.spyOn(commands, "getVaultPath").mockResolvedValue("/Users/gm/Vault");
    vi.spyOn(commands, "vaultSyncNow").mockResolvedValue({
      exported: 3,
      unchanged: 7,
      adopted: 0,
      deferred_apply: 0,
      deferred_conflict: 0,
      deferred_delete: 0,
      failed: 0,
    });
    render(VaultSyncSettings);
    await userEvent.click(
      await screen.findByRole("button", { name: /sync now/i }),
    );
    expect(await screen.findByText(/3 exported/i)).toBeInTheDocument();
    expect(await screen.findByText(/7 unchanged/i)).toBeInTheDocument();
  });

  it("surfaces a failure count rather than claiming success", async () => {
    vi.spyOn(commands, "getVaultPath").mockResolvedValue("/Users/gm/Vault");
    vi.spyOn(commands, "vaultSyncNow").mockResolvedValue({
      exported: 1,
      unchanged: 0,
      adopted: 0,
      deferred_apply: 0,
      deferred_conflict: 0,
      deferred_delete: 0,
      failed: 2,
    });
    render(VaultSyncSettings);
    await userEvent.click(
      await screen.findByRole("button", { name: /sync now/i }),
    );
    expect(await screen.findByText(/2 failed/i)).toBeInTheDocument();
  });

  it("clears the vault path when Disconnect is clicked", async () => {
    vi.spyOn(commands, "getVaultPath").mockResolvedValue("/Users/gm/Vault");
    const setPath = vi.spyOn(commands, "setVaultPath").mockResolvedValue();
    render(VaultSyncSettings);
    await userEvent.click(
      await screen.findByRole("button", { name: /disconnect/i }),
    );
    expect(setPath).toHaveBeenCalledWith(null);
  });
});
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `pnpm -C apps/desktop test:run VaultSyncSettings`
Expected: FAIL — cannot resolve `./VaultSyncSettings.svelte`.

- [ ] **Step 5: Implement `VaultSyncSettings.svelte`**

Svelte 5 runes only. Use `@tauri-apps/plugin-dialog`'s `open({ directory: true })` for the picker — already a dependency (`tauri-plugin-dialog` is in `src-tauri/Cargo.toml`). Verify the capability manifest permits `dialog:allow-open`; if not, add it deliberately to `apps/desktop/src-tauri/capabilities/` and call it out in the PR body, since capability files are security-sensitive.

```svelte
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { getVaultPath, setVaultPath, vaultSyncNow, type ReconcileReport } from '../lib/commands';

  let path = $state<string | null>(null);
  let busy = $state(false);
  let report = $state<ReconcileReport | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    getVaultPath().then((p) => (path = p));
  });

  async function choose() { /* open({ directory: true }) → setVaultPath → refresh */ }
  async function disconnect() { await setVaultPath(null); path = null; report = null; }
  async function syncNow() { /* busy = true; report = await vaultSyncNow(); busy = false */ }
</script>
```

Render: the configured path or "No vault configured"; a "Choose folder…" button; "Disconnect" when configured; "Sync now" (disabled when `path === null` or `busy`); and, after a sync, a summary line reading `{exported} exported · {unchanged} unchanged` plus `· {failed} failed` when `failed > 0`. Mount it in `SettingsView.svelte` under a "Markdown vault" heading.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `pnpm -C apps/desktop test:run VaultSyncSettings`
Expected: PASS — 6 tests.

- [ ] **Step 7: Write the acceptance feature (ADR-011)**

`apps/desktop/tests/e2e/features/vault-sync.feature`:

```gherkin
Feature: Markdown vault sync — export

  Scenario: Configuring a vault exports every record
    Given a campaign "Shadows of Valdris" with an entity "Seraphina Aldric"
    When the GM sets the vault path to a temporary directory
    Then a file exists at "campaigns/shadows-of-valdris/entities/npc/seraphina-aldric.md"
    And that file's frontmatter carries the alias "Seraphina Aldric"

  Scenario: Syncing again writes nothing
    Given a campaign with a configured vault that has been synced
    When the GM clicks "Sync now"
    Then the reconcile report shows 0 exported

  Scenario: A soft-deleted record is not exported
    Given a campaign with an entity marked vault_deleted
    When the GM clicks "Sync now"
    Then no file is written for that entity

  Scenario: A shared collection's entities are written once
    Given a collection subscribed to two campaigns with one entity
    When the GM clicks "Sync now"
    Then exactly one file exists for that entity under "collections/"
```

Bind the steps in `apps/desktop/tests/e2e/backend/steps/vault-sync.steps.ts` using `playwright-bdd`, following `tests/e2e/backend/steps/`'s existing patterns. Never edit or commit `.features-gen/`.

- [ ] **Step 8: Full CI gate, commit, open the D3b PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend

git add apps/desktop
git commit -m "feat(ui): vault sync settings + sync now"
git push -u origin feat/d3b-settings-ui:refs/heads/feat/d3b-settings-ui
gh pr create --base feat/d3a-reconcile --title "feat(vault): D3b — settings UI" --body "$(cat <<'EOF'
## What
Vault path picker, Disconnect, and "Sync now" with a reconcile summary, plus
the `.feature` acceptance scenarios (ADR-011).

## Why
Setting a vault path is the only way to turn export on. The summary surfaces
`failed` explicitly rather than reporting success when writes failed.

## Testing
`pnpm -C apps/desktop test:run VaultSyncSettings` — 6 component tests.
`pnpm -C apps/desktop run e2e:backend` — 4 Gherkin scenarios.
Full CI gate green including `cargo deny check`.

Stacked on #<D3a>.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 14: `QueueOutbound` + `PendingWrites` + drain loop — the D4a PR

**Files:**

- Create: `crates/chronacle-vault/src/outbound.rs`
- Modify: `crates/chronacle-vault/src/lib.rs` (register module)

**Interfaces:**

- Consumes: `VaultOutbound`, `VaultRef` (Task 1); `VaultSyncService::export_one` (Task 11).
- Produces: `QueueOutbound::new() -> (QueueOutbound, UnboundedReceiver<VaultRef>)`, `PendingWrites::{arm, matches, sweep, TTL}`, `drain_loop(rx, svc, pending)`.

**Guard semantics, and why they are not obvious.** One `write()` commonly emits several watcher events (`Create` + `Modify`, or `Modify(Data)` + `Modify(Metadata)`). So the guard is **not** consumed on first match. It is content-hash keyed, so a stale guard cannot mask a genuine later edit — different content, different hash. And it expires on a TTL, so a guard whose event never arrives does not live forever. Tranche 5's watcher is the consumer; D4a proves the guard before a watcher exists to trip it.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d4a-outbound-queue feat/d3b-settings-ui
```

- [ ] **Step 2: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chronacle_core::{VaultOutbound, VaultRef};

    fn vref(id: &str) -> VaultRef { VaultRef { table: "npc".into(), id: id.into() } }

    #[tokio::test]
    async fn enqueue_delivers_the_ref_to_the_receiver() {
        let (out, mut rx) = QueueOutbound::new();
        out.enqueue(vref("a"));
        assert_eq!(rx.recv().await, Some(vref("a")));
    }

    #[tokio::test]
    async fn enqueue_never_blocks_and_never_panics_after_the_receiver_drops() {
        let (out, rx) = QueueOutbound::new();
        drop(rx);
        out.enqueue(vref("a"));  // fire-and-forget: a dropped receiver is not an error
    }

    #[test]
    fn a_guard_matches_the_same_key_and_content() {
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        assert!(p.matches("k.md", 42));
    }

    #[test]
    fn a_guard_does_not_match_different_content_on_the_same_key() {
        // A genuine GM edit after our write must NOT be masked.
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        assert!(!p.matches("k.md", 99));
    }

    #[test]
    fn a_guard_survives_repeated_matches() {
        // One write emits several events (Create + Modify, Data + Metadata).
        // Consuming on first match would let the trailing events through.
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        assert!(p.matches("k.md", 42));
        assert!(p.matches("k.md", 42));
        assert!(p.matches("k.md", 42));
    }

    #[test]
    fn arming_the_same_key_twice_replaces_the_hash() {
        let p = PendingWrites::default();
        p.arm("k.md", 42);
        p.arm("k.md", 43);
        assert!(!p.matches("k.md", 42));
        assert!(p.matches("k.md", 43));
    }

    #[test]
    fn sweep_expires_guards_older_than_the_ttl() {
        let p = PendingWrites::default();
        p.arm_at("k.md", 42, std::time::Instant::now() - PendingWrites::TTL - std::time::Duration::from_secs(1));
        p.sweep();
        assert!(!p.matches("k.md", 42), "an event that never arrived must not pin a guard forever");
    }

    #[tokio::test]
    async fn drain_coalesces_repeat_enqueues_of_the_same_ref() {
        // Compiling 200 entities enqueues 200 refs; the drain writes each once.
        let (out, rx) = QueueOutbound::new();
        for _ in 0..5 { out.enqueue(vref("a")); }
        drop(out);

        let exported = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let e = std::sync::Arc::clone(&exported);
        drain_loop_with(rx, move |_vref| {
            e.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })
        .await;

        assert_eq!(exported.load(std::sync::atomic::Ordering::SeqCst), 1, "5 enqueues, 1 write");
    }

    #[tokio::test]
    async fn drain_continues_after_an_export_failure() {
        let (out, rx) = QueueOutbound::new();
        out.enqueue(vref("bad"));
        out.enqueue(vref("good"));
        drop(out);

        let seen = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let s = std::sync::Arc::clone(&seen);
        drain_loop_with(rx, move |v: VaultRef| {
            s.lock().unwrap().push(v.id.clone());
            if v.id == "bad" { Err(crate::VaultError::Frontmatter("boom".into())) } else { Ok(()) }
        })
        .await;

        assert_eq!(seen.lock().unwrap().len(), 2, "one failing ref must not stop the drain");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p chronacle-vault outbound`
Expected: FAIL — `cannot find type QueueOutbound in this scope`.

- [ ] **Step 4: Implement `outbound.rs`**

`drain_loop_with` is the testable core (takes a closure); `drain_loop` is the thin wrapper that calls `svc.export_one`. Coalescing: drain the channel with `try_recv` until empty, collect into a `HashSet<VaultRef>`, then export each once.

```rust
//! Non-blocking outbound queue and the write-loop guard.
//!
//! `enqueue` is a latency optimisation, never a correctness mechanism: a dropped
//! enqueue degrades to "the file updates on next reconcile". That is why the
//! producers depend on this one-method trait and nothing else vault-shaped.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chronacle_core::{VaultOutbound, VaultRef};

/// Producer handle. Fire-and-forget: a dropped receiver is not an error.
pub struct QueueOutbound {
    tx: tokio::sync::mpsc::UnboundedSender<VaultRef>,
}

impl QueueOutbound {
    /// Create the producer and its receiver.
    pub fn new() -> (Self, tokio::sync::mpsc::UnboundedReceiver<VaultRef>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

impl VaultOutbound for QueueOutbound {
    fn enqueue(&self, target: VaultRef) {
        // A closed channel means vault sync was turned off. Reconcile will
        // catch up if it is turned back on; never panic a producer for it.
        let _ = self.tx.send(target);
    }
}

/// Content-hash keyed loop guard with a TTL.
#[derive(Default)]
pub struct PendingWrites {
    inner: Mutex<HashMap<String, (u64, Instant)>>,
}

impl PendingWrites {
    /// A guard whose event never arrives expires after this long.
    pub const TTL: Duration = Duration::from_secs(30);

    /// Arm a guard for a key we are about to write.
    pub fn arm(&self, key: &str, hash: u64) {
        self.arm_at(key, hash, Instant::now());
    }

    /// Arm with an explicit timestamp. Test seam for TTL expiry.
    pub fn arm_at(&self, key: &str, hash: u64, at: Instant) {
        self.inner.lock().expect("poisoned").insert(key.to_owned(), (hash, at));
    }

    /// Whether an inbound event on `key` with this content is our own write.
    ///
    /// Deliberately does **not** consume the guard: one `write()` emits several
    /// events. Content-keyed, so a stale guard cannot mask a real later edit.
    pub fn matches(&self, key: &str, hash: u64) -> bool {
        let guard = self.inner.lock().expect("poisoned");
        guard.get(key).is_some_and(|(h, at)| *h == hash && at.elapsed() < Self::TTL)
    }

    /// Drop expired guards.
    pub fn sweep(&self) {
        self.inner.lock().expect("poisoned").retain(|_, (_, at)| at.elapsed() < Self::TTL);
    }
}
```

Then:

```rust
/// Drain the queue, coalescing repeats, calling `export` once per distinct ref.
pub async fn drain_loop_with<F>(mut rx: tokio::sync::mpsc::UnboundedReceiver<VaultRef>, export: F)
where
    F: Fn(VaultRef) -> Result<(), crate::VaultError> + Send + 'static,
{
    while let Some(first) = rx.recv().await {
        let mut batch = HashSet::new();
        batch.insert(first);
        while let Ok(next) = rx.try_recv() {
            batch.insert(next);
        }
        for vref in batch {
            if let Err(e) = export(vref.clone()) {
                // Reconcile is the correctness guarantee; a failed export is a
                // latency problem, not a data problem. Never abort the loop.
                eprintln!("vault: export of {} failed: {e}", vref.to_thing());
            }
        }
    }
}
```

`drain_loop(rx, svc, pending)` wraps it: for each ref, render, `pending.arm(&key, hash)`, then `svc.export_one(&vref).await`, then `pending.sweep()`.

- [ ] **Step 5: Run the tests, then the full CI gate, then open the D4a PR**

Run: `cargo test -p chronacle-vault outbound`
Expected: PASS — 9 tests.

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend

git add crates/chronacle-vault
git commit -m "feat(vault): outbound queue + content-hash write guard"
git push -u origin feat/d4a-outbound-queue:refs/heads/feat/d4a-outbound-queue
gh pr create --base feat/d3b-settings-ui --title "feat(vault): D4a — outbound queue + loop guard" --body "$(cat <<'EOF'
## What
`QueueOutbound` (one-method `VaultOutbound` producer over an unbounded mpsc),
`PendingWrites` (content-hash keyed loop guard with a TTL), and a drain loop
that coalesces repeat enqueues.

## Why
Compiling 200 entities enqueues 200 refs; the drain writes each key once. The
guard is **not** consumed on first match — one `write()` emits several watcher
events — and it is content-keyed, so a stale guard cannot mask a genuine later
edit. The TTL stops a guard whose event never arrives from living forever.

No watcher exists yet to trip the guard: D4a proves it in isolation, before D5a
(tranche 5) introduces the only thing that can produce a write→watch→write loop.

## Testing
`cargo test -p chronacle-vault outbound` — 9 tests: repeated matches, hash
mismatch (a real edit is not masked), TTL expiry via an injected `Instant`,
5-enqueues-1-write coalescing, and drain-survives-export-failure.
Full CI gate green including `cargo deny check`.

Stacked on #<D3b>.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

### Task 15: Wire the five producers — the D4b PR

**Files:**

- Modify: `crates/chronacle-extraction/src/entity_service/crud/update.rs` (+ create/delete siblings)
- Modify: `crates/chronacle-extraction/src/codex_service/compile.rs:220-224` (after the article `UPDATE`)
- Modify: `crates/chronacle-extraction/src/codex_service/rules.rs` (after each `rule_entry` write)
- Modify: `crates/chronacle-extraction/src/codex_service/proposals.rs:634` (after an accepted proposal's `UPDATE`)
- Modify: `crates/chronacle-domain/src/session_service.rs:105,221` (after session create/update)
- Modify: `crates/chronacle-extraction/Cargo.toml`, `crates/chronacle-domain/Cargo.toml` (`chronacle-core` if absent)
- Modify: `apps/desktop/src-tauri/src/lib.rs` (construct `QueueOutbound`, spawn `drain_loop`, inject `Arc<dyn VaultOutbound>`)
- Create: `apps/desktop/src-tauri/tests/vault_outbound_test.rs`
- Modify: `apps/desktop/tests/e2e/features/vault-sync.feature` (add the D4 scenarios)

**Interfaces:**

- Consumes: `VaultOutbound`, `NoopOutbound`, `VaultRef` (Task 1); `QueueOutbound`, `drain_loop` (Task 14).
- Produces: no new public API. Each producer gains an `outbound: &dyn VaultOutbound` parameter (or an `Arc<dyn VaultOutbound>` field), defaulting to `NoopOutbound` where vault sync is off.

**Why `NoopOutbound` rather than `Option`:** threading `Option<Arc<dyn VaultOutbound>>` through five call sites in three crates invites a forgotten `if let Some`. A `NoopOutbound` makes "vault sync is off" a value, not a branch — and `compile.rs` cannot silently skip the enqueue for the content the vault most needs.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/d4b-wire-producers feat/d4a-outbound-queue
```

- [ ] **Step 2: Write the failing integration test**

`apps/desktop/src-tauri/tests/vault_outbound_test.rs`:

```rust
//! Producers must enqueue after every successful record write — especially the
//! compiler, whose `codex_article` writes never touch `updated_at`.

use std::sync::{Arc, Mutex};

use chronacle_core::{VaultOutbound, VaultRef};

/// Records every enqueue for assertion.
#[derive(Default)]
struct SpyOutbound { seen: Mutex<Vec<VaultRef>> }
impl VaultOutbound for SpyOutbound {
    fn enqueue(&self, target: VaultRef) { self.seen.lock().unwrap().push(target); }
}

async fn db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
    let db = surrealdb::engine::any::connect("mem://").await.expect("mem db");
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.expect("migrations");
    db
}

/// Gherkin: Given a configured vault, when the GM edits an entity's notes in
/// Chronacle, then the corresponding .md body updates.
#[tokio::test]
async fn updating_an_entity_enqueues_it() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name='C', system='5e', created_at=time::now(), updated_at=time::now(); \
         CREATE npc:n1 SET name='A', created_at=time::now(), updated_at=time::now(); \
         RELATE campaign:c1->in_campaign->npc:n1;",
    ).await.expect("seed").check().expect("seed response");

    let spy = Arc::new(SpyOutbound::default());
    chronacle_extraction::entity_service::update_entity(
        &db, "npc:n1", "A", Some("Summary."), Some("Edited notes."), spy.as_ref(),
    )
    .await
    .expect("update");

    let seen = spy.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0], VaultRef { table: "npc".into(), id: "n1".into() });
}

/// The compiler is the producer that matters most: it rewrites `codex_article`
/// without touching `updated_at`, so a missed enqueue would leave the vault
/// stale until the next reconcile.
#[tokio::test]
async fn compiling_an_entity_article_enqueues_it() {
    use chronacle_extraction::extraction_service::test_support::{MockEmbeddingProvider, MockLlm};

    let db = db().await;
    db.query(
        "CREATE collection:k1 SET name='Core', created_at=time::now(), updated_at=time::now(); \
         CREATE npc:n1 SET name='A', codex_stale=true, created_at=time::now(), updated_at=time::now(); \
         RELATE collection:k1->in_collection->npc:n1;",
    ).await.expect("seed").check().expect("seed response");

    let spy = Arc::new(SpyOutbound::default());
    let llm = Arc::new(MockLlm::returning("Compiled article body."));
    let embed = Arc::new(MockEmbeddingProvider::default());

    chronacle_extraction::codex_service::compile_collection(
        &db, llm, embed, "collection:k1", spy.as_ref(),
    )
    .await
    .expect("compile");

    let seen = spy.seen.lock().unwrap();
    assert_eq!(
        seen.as_slice(),
        &[VaultRef { table: "npc".into(), id: "n1".into() }],
        "compile writes codex_article but never updated_at — a missed enqueue is invisible"
    );
}

#[tokio::test]
async fn accepting_an_entity_notes_proposal_enqueues_the_target() {
    use chronacle_extraction::extraction_service::test_support::MockEmbeddingProvider;

    let db = db().await;
    db.query(
        "CREATE collection:k1 SET name='Core', created_at=time::now(), updated_at=time::now(); \
         CREATE npc:n1 SET name='A', created_at=time::now(), updated_at=time::now(); \
         RELATE collection:k1->in_collection->npc:n1; \
         CREATE codex_proposal:p1 SET kind='entity_notes_update', target=npc:n1, \
             collection=collection:k1, payload={ proposed_text: 'New notes.', rationale: 'r' }, \
             origin={ kind: 'manual' }, status='pending', created_at=time::now();",
    ).await.expect("seed").check().expect("seed response");

    let spy = Arc::new(SpyOutbound::default());
    let embed = Arc::new(MockEmbeddingProvider::default());
    chronacle_extraction::codex_service::accept_proposal(
        &db, embed, "codex_proposal:p1", spy.as_ref(),
    )
    .await
    .expect("accept");

    let seen = spy.seen.lock().unwrap();
    assert_eq!(seen.as_slice(), &[VaultRef { table: "npc".into(), id: "n1".into() }]);
}

#[tokio::test]
async fn saving_a_session_enqueues_it() {
    let db = db().await;
    db.query(
        "CREATE campaign:c1 SET name='C', system='5e', created_at=time::now(), updated_at=time::now(); \
         CREATE session:s1 SET campaign=campaign:c1, session_number=1, title='One', \
             date_played='2026-01-01', notes='', created_at=time::now(), updated_at=time::now();",
    ).await.expect("seed").check().expect("seed response");

    let spy = Arc::new(SpyOutbound::default());
    chronacle_domain::session_service::update_session(
        &db, "session:s1", "One", "2026-01-01", "Recap written by the GM.", spy.as_ref(),
    )
    .await
    .expect("update session");

    let seen = spy.seen.lock().unwrap();
    assert_eq!(seen.as_slice(), &[VaultRef { table: "session".into(), id: "s1".into() }]);
}

#[tokio::test]
async fn a_rules_compile_enqueues_each_written_rule_entry() {
    let db = db().await;
    db.query(
        "CREATE collection:k1 SET name='Core', created_at=time::now(), updated_at=time::now(); \
         CREATE rule_entry:r1 SET collection=collection:k1, name='Grappling', \
             category='procedure', body='Old body.', compiled_at=time::now();",
    ).await.expect("seed").check().expect("seed response");

    let spy = Arc::new(SpyOutbound::default());
    // The rules pipeline enqueues after each successful `rule_entry` write.
    chronacle_extraction::codex_service::rules::persist_rule_entry(
        &db, "collection:k1", "Grappling", "procedure", "New body.", spy.as_ref(),
    )
    .await
    .expect("persist rule entry");

    let seen = spy.seen.lock().unwrap();
    assert_eq!(seen.as_slice(), &[VaultRef { table: "rule_entry".into(), id: "r1".into() }]);
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p chronacle --test vault_outbound_test`
Expected: FAIL — `update_entity` takes no `outbound` parameter.

- [ ] **Step 4: Thread `&dyn VaultOutbound` through each producer**

For each call site, add the parameter and enqueue **after** the write succeeds and its `.check()` passes — never before, and never on an error path:

```rust
// crates/chronacle-extraction/src/codex_service/compile.rs, after the UPDATE + .check()
outbound.enqueue(chronacle_core::VaultRef {
    table: node.kind.clone(),
    id: node.id.clone(),
});
```

Add `chronacle-core` to `chronacle-extraction`'s dependencies if absent. `chronacle-extraction` must depend on `chronacle-core` **only** — never on `chronacle-vault`. Existing call sites that have no vault (tests, background tasks without state) pass `&NoopOutbound`.

- [ ] **Step 5: Wire the composition root**

In `apps/desktop/src-tauri/src/lib.rs`, when `vault_sync_path` is configured:

```rust
let (outbound, rx) = chronacle_vault::outbound::QueueOutbound::new();
let pending = Arc::new(chronacle_vault::outbound::PendingWrites::default());
let svc = build_vault_service(db.clone(), &root);
tauri::async_runtime::spawn(chronacle_vault::outbound::drain_loop(
    rx, Arc::clone(&svc), Arc::clone(&pending),
));
```

Store `Arc<dyn VaultOutbound>` in `AppState` (falling back to `Arc::new(NoopOutbound)` when no vault is configured) and hand it to the command handlers that call the five producers. `set_vault_path` rebuilds the queue and respawns the drain task.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p chronacle --test vault_outbound_test && cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Add the D4 acceptance scenarios**

Append to `apps/desktop/tests/e2e/features/vault-sync.feature`:

```gherkin
  Scenario: Editing notes in Chronacle updates the vault file
    Given a campaign with a configured vault and an entity "Seraphina Aldric"
    When the GM edits that entity's notes
    Then the entity's vault file body contains the new notes

  Scenario: Recompiling writes each changed file exactly once
    Given a compiled collection with a configured vault
    When the GM compiles the collection again after a source changes
    Then each changed entity's file is written exactly once

  Scenario: Renaming an entity re-keys its file
    Given a campaign with a configured vault and an entity "Seraphina Aldric"
    When the GM renames that entity to "Seraphina the Archivist"
    Then a file exists at ".../seraphina-the-archivist.md"
    And no file exists at ".../seraphina-aldric.md"
```

- [ ] **Step 8: Full CI gate, commit, open the D4b PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && \
cargo test --workspace && cargo deny check && \
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && \
pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend

git add crates apps/desktop Cargo.lock
git commit -m "feat(vault): enqueue from every record producer"
git push -u origin feat/d4b-wire-producers:refs/heads/feat/d4b-wire-producers
gh pr create --base feat/d4a-outbound-queue --title "feat(vault): D4b — wire outbound producers" --body "$(cat <<'EOF'
## What
Threads `&dyn VaultOutbound` through the five record producers: `entity_service`
CRUD, `codex_service::compile`, the rules pipeline, accepted `codex_proposal`s,
and `session_service`. The composition root builds the queue and spawns the
drain task.

## Why
A trigger design that hooked only the obvious CRUD paths would silently miss the
compiler — the content the vault most needs to mirror, and the one producer that
never bumps `updated_at`. `NoopOutbound` makes "vault sync is off" a value
rather than a branch, so no call site can forget an `if let Some`.

`chronacle-extraction` depends on `chronacle-core` only. It never learns what a
file is.

## Testing
`cargo test -p chronacle --test vault_outbound_test` — a `SpyOutbound` asserts
one enqueue per producer, including the compile path.
3 new Gherkin scenarios (edit → file updates; recompile → one write per file;
rename → re-key). Full CI gate green including `cargo deny check`.

Stacked on #<D4a>. **Last PR of tranche 4** — after this merges, the vault
exports the codex to Obsidian one-way. Inbound is tranche 5.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Merge order & stack maintenance

Merge bottom-up. After each PR merges to `main`:

```bash
git checkout <next-branch>
git rebase --onto main <old-parent> <next-branch>
git push --force-with-lease origin <next-branch>:refs/heads/<next-branch>
gh pr edit <next-pr> --base main
```

| PR  | Branch                    | Tasks | Base (initial)            |
| --- | ------------------------- | ----- | ------------------------- |
| D0  | `chore/d0-vault-crate`    | 1–2   | `main`                    |
| D1a | `feat/d1a-frontmatter`    | 3–4   | `chore/d0-vault-crate`    |
| D1b | `feat/d1b-key-mapping`    | 5–6   | `feat/d1a-frontmatter`    |
| D2a | `feat/d2a-fs-store`       | 7     | `feat/d1b-key-mapping`    |
| D2b | `feat/d2b-record-store`   | 8–9   | `feat/d2a-fs-store`       |
| D3a | `feat/d3a-reconcile`      | 10–12 | `feat/d2b-record-store`   |
| D3b | `feat/d3b-settings-ui`    | 13    | `feat/d3a-reconcile`      |
| D4a | `feat/d4a-outbound-queue` | 14    | `feat/d3b-settings-ui`    |
| D4b | `feat/d4b-wire-producers` | 15    | `feat/d4a-outbound-queue` |

D2a and D2b are logically independent (both depend only on D1b); they are stacked
serially anyway to keep one linear chain.
