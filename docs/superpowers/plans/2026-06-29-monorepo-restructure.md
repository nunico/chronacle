# Monorepo Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure the single-crate Tauri app into a Cargo workspace of reusable `chronacle-*` library crates under `crates/`, with the Tauri app + Svelte frontend under `apps/desktop/`, so PDF ingestion / entity extraction / the service layer can be reused by a future axum cloud binary.

**Architecture:** Three sequenced, independently-verifiable stages. **Stage A** removes the last hardcoded embedded-engine type (`Surreal<engine::local::Db>` → `Surreal<Any>`) *in the current single crate* — the only behavior-affecting change. **Stage B** mechanically splits files into crates with zero logic changes. **Stage C** moves the frontend under `apps/desktop/` last (it reopens the SPA-embed area). **Stage D** updates docs. Each stage ends with a full green `cargo test --workspace` / clippy / E2E gate before the next begins.

**Tech Stack:** Rust 2021, Cargo workspaces, `surrealdb` (embedded RocksDB via `engine::any`), Tauri 2, Svelte 5 + Vite, pnpm workspaces, Playwright.

**Spec:** `docs/superpowers/specs/2026-06-28-monorepo-restructure-design.md`

---

## File Structure

### Final crate layout

```text
chronacle/
├── Cargo.toml                         # workspace root: members + [workspace.dependencies]
├── crates/
│   ├── chronacle-core/                # types, errors, traits, Db helpers
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── chronacle-db/                  # schema/*.surql + run_migrations
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, schema/}
│   ├── chronacle-providers/           # concrete provider impls
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, embedding/, llm_provider/, vector_store.rs, blob_store.rs}
│   ├── chronacle-ingestion/           # pdf_extractor + chunker + ingestion_service + text_normalizer
│   │   ├── Cargo.toml
│   │   ├── src/{lib.rs, pdf_extractor/, chunker/, ingestion_service/, text_normalizer.rs}
│   │   └── tests/                     # relocated integration tests
│   ├── chronacle-extraction/          # entity_service + wikilink + extraction_service
│   │   ├── Cargo.toml
│   │   ├── src/{lib.rs, entity_service/, wikilink/, extraction_service/}
│   │   └── tests/
│   ├── chronacle-retrieval/           # agent_service
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, agent_service/}
│   └── chronacle-domain/              # campaign + session + collection + custom_provider
│       ├── Cargo.toml
│       ├── src/{lib.rs, campaign_service.rs, session_service.rs, collection_service/, custom_provider_service.rs}
│       └── tests/
└── apps/
    └── desktop/
        ├── package.json               # app frontend manifest (svelte, tauri-cli, vite)
        ├── vite.config.ts
        ├── svelte.config.js
        ├── tsconfig.json
        ├── eslint.config.js
        ├── index.html
        ├── src/                       # Svelte frontend (moved from repo-root src/)
        ├── tests/e2e/                 # moved from repo-root tests/e2e/
        ├── playwright.config.ts
        └── src-tauri/                 # Tauri Rust: commands/, lib.rs, AppState, main.rs, settings_service
            ├── Cargo.toml
            ├── build.rs
            ├── capabilities/
            ├── resources/
            ├── icons/
            ├── tauri.conf.json
            └── src/{main.rs, lib.rs, commands/, services/settings_service.rs, services/mod.rs}
```

### Module → crate mapping (current `src-tauri/src/...` → destination)

| Current path | Destination crate |
|---|---|
| `services/pdf_extractor/` | `chronacle-ingestion` |
| `services/chunker/` | `chronacle-ingestion` |
| `services/ingestion_service/` | `chronacle-ingestion` |
| `services/text_normalizer.rs` | `chronacle-ingestion` |
| `services/entity_service/` | `chronacle-extraction` |
| `services/wikilink/` | `chronacle-extraction` |
| `services/extraction_service/` | `chronacle-extraction` |
| `services/agent_service/` | `chronacle-retrieval` |
| `services/campaign_service.rs` | `chronacle-domain` |
| `services/session_service.rs` | `chronacle-domain` |
| `services/collection_service/` | `chronacle-domain` |
| `services/custom_provider_service.rs` | `chronacle-domain` |
| `services/settings_service.rs` | **stays** in `apps/desktop/src-tauri` |
| `providers/` (all) | `chronacle-providers` |
| `schema/` | `chronacle-db` |
| `commands/`, `lib.rs`, `main.rs` | `apps/desktop/src-tauri` |

> **Note on shared types:** Many domain structs currently live *inside* their
> service module (e.g. `Campaign`, `Entity`, `Chunk`, `SearchResult`). During
> Stage B these stay with their owning service unless another crate imports
> them across a boundary; only types imported across crates move to
> `chronacle-core`. The cross-crate type set is discovered empirically by the
> compiler in Stage B (Task 8) and recorded there — do not pre-move types.

---

## STAGE A — DB genericity (no files move)

Goal: switch the one concrete embedded-engine type to `Surreal<Any>` so all code
compiles against any SurrealDB connection. Done inside the current single crate.

### Task A1: Verify `engine::any` reaches embedded RocksDB

**Files:**
- Test: `src-tauri/src/lib.rs` (temporary `#[cfg(test)]` probe, removed in A4)

- [ ] **Step 1: Add a probe test that opens RocksDB via `any::connect`**

Add to the bottom of `src-tauri/src/lib.rs`:

```rust
#[cfg(test)]
mod any_engine_probe {
    #[tokio::test]
    async fn any_connect_opens_embedded_rocksdb() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("probe.db");
        let url = format!("rocksdb://{}", path.display());
        let db: surrealdb::Surreal<surrealdb::engine::any::Any> =
            surrealdb::engine::any::connect(&url)
                .await
                .expect("any::connect should open embedded RocksDB");
        db.use_ns("t").use_db("t").await.unwrap();
        db.query("DEFINE TABLE probe;").await.unwrap();
    }
}
```

- [ ] **Step 2: Run the probe**

Run: `cargo test -p Chronacle any_connect_opens_embedded_rocksdb -- --nocapture`
Expected: PASS. If it fails to compile with "no function `connect`" or a feature
error, add the `any` engine feature: in `src-tauri/Cargo.toml` change the
`surrealdb` line to include it, e.g.
`features = ["kv-rocksdb", "kv-mem", "protocol-ws"]` is **not** needed for
embedded — instead ensure the crate exposes `engine::any` (it is gated behind
having at least one `kv-*` backend, which is already present). If still failing,
the fallback is to keep `engine::local::Db` and stop Stage A here (revisit the
genericity decision with the user).
Expected after fix: PASS.

- [ ] **Step 3: Commit the verified probe**

```bash
git add src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "test: verify surrealdb engine::any opens embedded rocksdb"
```

### Task A2: Genericize `campaign_service` over the connection

**Files:**
- Modify: `src-tauri/src/services/campaign_service.rs`

- [ ] **Step 1: Replace the concrete `Db` alias with a generic parameter**

In `campaign_service.rs`, delete line 2 (`use surrealdb::engine::local::Db;`) and
add `use surrealdb::Connection;`. Change every function signature from
`db: &Surreal<Db>` to `db: &Surreal<C>` and add `<C: Connection>` to each `pub
async fn`. Example for `get_all`:

```rust
pub async fn get_all<C: Connection>(db: &Surreal<C>) -> Result<Vec<Campaign>, String> {
```

Apply the same transform to `create`, `get_by_id`, `update`, `delete` (and any
other fn taking `db: &Surreal<Db>` in this file).

- [ ] **Step 2: Compile**

Run: `cargo build -p Chronacle`
Expected: builds (the in-file `#[cfg(test)]` tests use `engine::local::Mem`,
which satisfies `C: Connection`).

- [ ] **Step 3: Run this service's tests**

Run: `cargo test -p Chronacle campaign_service`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/campaign_service.rs
git commit -m "refactor: make campaign_service generic over surreal connection"
```

### Task A3: Genericize `custom_provider_service` over the connection

**Files:**
- Modify: `src-tauri/src/services/custom_provider_service.rs`

- [ ] **Step 1: Apply the same transform as Task A2**

Delete `use surrealdb::engine::local::Db;` (line 2), add `use
surrealdb::Connection;`, change every `db: &Surreal<Db>` →
`db: &Surreal<C>` with `<C: Connection>` on each `pub async fn`.

- [ ] **Step 2: Compile and test**

Run: `cargo test -p Chronacle custom_provider`
Expected: builds and PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/custom_provider_service.rs
git commit -m "refactor: make custom_provider_service generic over connection"
```

### Task A4: Switch `AppState.db` and command signatures to `Surreal<Any>`

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/settings_commands.rs:30,47`

- [ ] **Step 1: Change the `AppState.db` field type**

In `src-tauri/src/lib.rs`, change:

```rust
pub db: surrealdb::Surreal<surrealdb::engine::local::Db>,
```
to:
```rust
pub db: surrealdb::Surreal<surrealdb::engine::any::Any>,
```

- [ ] **Step 2: Change `init_database` to use `any::connect`**

In `init_database`, change the return type and connection construction. Replace:

```rust
async fn init_database() -> (
    std::path::PathBuf,
    surrealdb::Surreal<surrealdb::engine::local::Db>,
) {
    let data_dir = app_data_dir();
    let db_path = data_dir.join("chronacle.db");

    let db = surrealdb::Surreal::new::<surrealdb::engine::local::RocksDb>(db_path)
        .await
        .expect("Failed to initialise SurrealDB (RocksDB)");
```
with:
```rust
async fn init_database() -> (
    std::path::PathBuf,
    surrealdb::Surreal<surrealdb::engine::any::Any>,
) {
    let data_dir = app_data_dir();
    let db_path = data_dir.join("chronacle.db");

    let db = surrealdb::engine::any::connect(format!("rocksdb://{}", db_path.display()))
        .await
        .expect("Failed to initialise SurrealDB (RocksDB)");
```

- [ ] **Step 3: Update remaining `engine::local::Db` references in non-test code**

In `src-tauri/src/lib.rs`, update the `read_settings_map`,
`build_llm_provider_from_db`, `build_embedding_provider_from_db`, and
`build_custom_provider` signatures that name
`surrealdb::Surreal<surrealdb::engine::local::Db>` to
`surrealdb::Surreal<surrealdb::engine::any::Any>`.

In `src-tauri/src/commands/settings_commands.rs` lines 30 and 47, change
`db: &surrealdb::Surreal<surrealdb::engine::local::Db>` to
`db: &surrealdb::Surreal<surrealdb::engine::any::Any>`.

- [ ] **Step 4: Remove the temporary probe test from Task A1**

Delete the `any_engine_probe` module added in Task A1 from
`src-tauri/src/lib.rs`.

- [ ] **Step 5: Build the whole crate**

Run: `cargo build -p Chronacle`
Expected: builds. Fix any remaining `engine::local::Db` references the compiler
flags in non-test files (`grep -rn "engine::local::Db" src-tauri/src` should
return only `#[cfg(test)]` files using `Mem`).

- [ ] **Step 6: Full test + clippy gate**

Run:
```bash
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```
Expected: all PASS. This proves the genericity change is behaviour-preserving.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/settings_commands.rs
git commit -m "refactor: use surrealdb engine::any for AppState connection (Stage A)"
```

---

## STAGE B — split into crates (no logic changes)

Goal: mechanical extraction. Each task creates one crate, moves its files with
`git mv` (preserves history), creates its `Cargo.toml`, fixes `use` paths, and
gates on `cargo build` / `cargo test`. **No business logic, query, or signature
changes** beyond `use`-path and `pub` visibility forced by boundaries.

> Convention for every crate Cargo.toml in this stage: use
> `<dep>.workspace = true` for shared deps (defined in Task B1) and
> `license.workspace = true`. Do not edit LICENSE files. Set `publish = false`.
> The library crate name uses underscores in code
> (`chronacle_core`) and hyphens in the package name (`chronacle-core`).

### Task B1: Create the workspace root and `[workspace.dependencies]`

**Files:**
- Modify: `Cargo.toml` (repo root)

- [ ] **Step 1: Rewrite the root `Cargo.toml` as a workspace with shared deps**

Replace the root `Cargo.toml` with:

```toml
[workspace]
# Glob member: `crates/*` auto-includes each crate as it is created in B2–B7,
# matching nothing until the first crate exists (no missing-dir errors).
members = [
    "crates/*",
    "src-tauri",
]
resolver = "2"

[workspace.package]
edition = "2021"
rust-version = "1.95"
license = "AGPL-3.0 WITH branding-exception"
publish = false

[workspace.dependencies]
surrealdb = { version = "2", features = ["kv-rocksdb", "kv-mem"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4"] }
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json", "stream"] }
thiserror = "2"
regex = "1"
futures-util = "0.3"
unicode-segmentation = "1"
fastembed = { version = "5", features = ["ort-load-dynamic"] }
pdfium-render = { version = "0.8", features = ["thread_safe", "image"] }
dirs-next = "2"
# dev
mockall = "0.13"
tempfile = "3"
pretty_assertions = "1"
lopdf = "0.38"

[profile.test]
opt-level = 0
debug = 2

[workspace.metadata.chronacle]
coverage = "cargo llvm-cov --html"
license = "AGPL-3.0 WITH branding-exception"
license-file = "LICENSE"
```

> `src-tauri` stays a workspace member here (it moves to `apps/desktop/src-tauri`
> only in Stage C). The `crates/*` glob auto-includes each crate as B2–B7 create
> them — no per-task `members` edits needed. In Stage C, Task C1 changes
> `"src-tauri"` → `"apps/desktop/src-tauri"`.

- [ ] **Step 2: Verify the workspace still builds**

Run: `cargo build --workspace`
Expected: builds (only `src-tauri` is a member; behaviour identical to Stage A).

- [ ] **Step 3: (Defer commit)**

Commit happens at the end of B2 (which adds the first crate). Proceed.

### Task B2: Extract `chronacle-core` (dependency traits + their DTOs)

**Files:**
- Create: `crates/chronacle-core/Cargo.toml`, `crates/chronacle-core/src/lib.rs`
  and `src/{vector_store.rs, blob_store.rs, llm.rs, embedding.rs}`
- Modify (still in place under `src-tauri/`): `providers/vector_store.rs`,
  `providers/blob_store.rs`, `providers/llm_provider/mod.rs`,
  `providers/embedding/mod.rs`

> **Why traits live in core (spec-critical):** `chronacle-ingestion` /
> `chronacle-extraction` / `chronacle-retrieval` must compile against the trait
> *contracts* without pulling in the concrete provider impls (which drag in
> `fastembed`, `pdfium-render`, `reqwest`). That is the property that makes them
> reusable by a cloud server with its own providers. So the four traits and the
> DTOs/errors that appear in their signatures move to `chronacle-core`; the
> impls stay in `chronacle-providers` (Task B4).
>
> This is feasible because the service-crate tests do **not** use `mockall`
> automock on these traits — `extraction` defines its own `MockLlm` /
> `MockVectorStore` (so it needs only the trait defs, no providers dep), and
> `ingestion` uses the hand-written `MockEmbeddingProvider` (satisfied by a
> dev-dependency on `chronacle-providers`).

- [ ] **Step 1: Create the crate manifest**

`crates/chronacle-core/Cargo.toml`:

```toml
[package]
name = "chronacle-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_core"

[dependencies]
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
thiserror.workspace = true
```

> Note: `chronacle-core` does **not** depend on `surrealdb` — the four traits
> are storage-agnostic (no `Surreal<C>` in their signatures; `VectorStore` is
> defined over `SearchResult`/`IndexedChunk` DTOs). Confirm during Step 2; if a
> trait signature genuinely needs `surrealdb`, add it then.

- [ ] **Step 2: Move each trait + its DTOs/errors into a core module (verbatim bodies)**

Move the following items **without changing their bodies**, into the named core
module file. Create each file with the listed items.

`crates/chronacle-core/src/vector_store.rs` ← from `providers/vector_store.rs`:
- `SearchResult` (struct), `VectorStore` (trait), `IndexedChunk` (struct).
- Leave `SurrealDbVector`, its `impl`, and `validate_collection_id` in
  `providers/vector_store.rs`.

`crates/chronacle-core/src/blob_store.rs` ← from `providers/blob_store.rs`:
- `BlobStore` (trait). Leave `LocalFileStore` + impl in providers.

`crates/chronacle-core/src/llm.rs` ← from `providers/llm_provider/mod.rs`:
- `ChatMessage` (struct), `LlmError` (enum), `LlmProvider` (trait).
- Leave `NoopProvider`, the `pub use anthropic/ollama/openai` re-exports, and the
  submodule declarations in `providers/llm_provider/mod.rs`.

`crates/chronacle-core/src/embedding.rs` ← from `providers/embedding/mod.rs`:
- `EmbeddingError` (enum), `EmbeddingProvider` (trait).
- Leave the `consistency`/`local`/`mock`/`openai` re-exports and submodules in
  `providers/embedding/mod.rs`.

- [ ] **Step 3: Write the core `lib.rs` re-exporting the contracts**

`crates/chronacle-core/src/lib.rs`:

```rust
//! Storage-agnostic dependency contracts (traits) and their DTOs for Chronacle.
//!
//! Concrete implementations live in `chronacle-providers`; consumers depend on
//! these traits so they can be reused by a future cloud server.
pub mod blob_store;
pub mod embedding;
pub mod llm;
pub mod vector_store;

pub use blob_store::BlobStore;
pub use embedding::{EmbeddingError, EmbeddingProvider};
pub use llm::{ChatMessage, LlmError, LlmProvider};
pub use vector_store::{IndexedChunk, SearchResult, VectorStore};
```

- [ ] **Step 4: Point the still-in-place provider modules at the core traits**

In `src-tauri/src/providers/vector_store.rs`, `blob_store.rs`,
`llm_provider/mod.rs`, and `embedding/mod.rs`, delete the moved definitions and
add the corresponding `use chronacle_core::...;` imports so the remaining impls
(`SurrealDbVector`, `LocalFileStore`, `NoopProvider`, fastembed/openai/mock)
reference the core traits. Add the dep to `src-tauri/Cargo.toml`:
`chronacle-core = { path = "../crates/chronacle-core" }`. Re-export the traits
from each provider module for source-compatibility with existing call sites,
e.g. in `providers/vector_store.rs` add `pub use chronacle_core::vector_store::{SearchResult, VectorStore, IndexedChunk};`.

- [ ] **Step 5: Build core and the app**

Run: `cargo build -p chronacle-core && cargo build -p Chronacle`
Expected: both build. Fix any signature that the compiler reports as needing a
type that did not move (move it too, or re-export it).

- [ ] **Step 6: Commit the workspace skeleton + core**

```bash
git add Cargo.toml crates/chronacle-core src-tauri
git commit -m "refactor: extract dependency traits into chronacle-core (Stage B)"
```

### Task B3: Extract `chronacle-db` (schema + migrations)

**Files:**
- Create: `crates/chronacle-db/Cargo.toml`
- Move: `src-tauri/src/schema/` → `crates/chronacle-db/src/schema/`
- Create: `crates/chronacle-db/src/lib.rs`

- [ ] **Step 1: Create the manifest**

`crates/chronacle-db/Cargo.toml`:

```toml
[package]
name = "chronacle-db"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_db"

[dependencies]
surrealdb.workspace = true

[dev-dependencies]
tokio.workspace = true
```

- [ ] **Step 2: Move the schema directory with history**

```bash
mkdir -p crates/chronacle-db/src
git mv src-tauri/src/schema crates/chronacle-db/src/schema
```

- [ ] **Step 3: Create `lib.rs` re-exporting the schema module**

`crates/chronacle-db/src/lib.rs`:

```rust
//! Schema definitions and migration runner for Chronacle's SurrealDB.
pub mod schema;
pub use schema::run_migrations;
```

- [ ] **Step 4: Confirm the `CARGO_MANIFEST_DIR` path still resolves**

`schema/mod.rs` builds the path as `CARGO_MANIFEST_DIR/src/schema`. After the
move, `CARGO_MANIFEST_DIR` is the `chronacle-db` crate root, and the `.surql`
files live at `src/schema/`, so the existing path join `join("src").join("schema")`
remains correct. No code change needed.

- [ ] **Step 5: Build and run the schema test**

Run: `cargo test -p chronacle-db`
Expected: `test_schema_runs_cleanly_against_in_memory_db` PASS.

- [ ] **Step 6: Update `src-tauri` to use the new crate (temporary shim)**

In `src-tauri/src/lib.rs`, remove `pub mod schema;` and change
`schema::run_migrations` to `chronacle_db::run_migrations`. Add `chronacle-db` to
`src-tauri/Cargo.toml` dependencies:
`chronacle-db = { path = "../crates/chronacle-db" }`. Update all
`crate::schema::run_migrations` references across `src-tauri/src` to
`chronacle_db::run_migrations` (28 sites, mostly tests):

```bash
grep -rl "crate::schema::run_migrations" src-tauri/src | xargs sed -i '' 's/crate::schema::run_migrations/chronacle_db::run_migrations/g'
```

Add `chronacle-db` as a dev-dependency too if test modules reference it.

- [ ] **Step 7: Build the whole workspace**

Run: `cargo build --workspace`
Expected: builds.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/chronacle-db src-tauri
git commit -m "refactor: extract chronacle-db crate (schema + migrations)"
```

### Task B4: Extract `chronacle-providers`

**Files:**
- Create: `crates/chronacle-providers/Cargo.toml`
- Move: `src-tauri/src/providers/` → `crates/chronacle-providers/src/`
- Create: `crates/chronacle-providers/src/lib.rs`

- [ ] **Step 1: Create the manifest**

`crates/chronacle-providers/Cargo.toml`:

```toml
[package]
name = "chronacle-providers"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_providers"

[dependencies]
chronacle-core = { path = "../chronacle-core" }
surrealdb.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
thiserror.workspace = true
reqwest.workspace = true
futures-util.workspace = true
fastembed.workspace = true
dirs-next.workspace = true

[dev-dependencies]
mockall.workspace = true
tempfile.workspace = true
pretty_assertions.workspace = true
tokio.workspace = true
```

- [ ] **Step 2: Move the providers tree**

```bash
mkdir -p crates/chronacle-providers/src
git mv src-tauri/src/providers/blob_store.rs crates/chronacle-providers/src/blob_store.rs
git mv src-tauri/src/providers/vector_store.rs crates/chronacle-providers/src/vector_store.rs
git mv src-tauri/src/providers/embedding crates/chronacle-providers/src/embedding
git mv src-tauri/src/providers/llm_provider crates/chronacle-providers/src/llm_provider
git mv src-tauri/src/providers/mod.rs crates/chronacle-providers/src/lib.rs
```

- [ ] **Step 3: Fix `lib.rs` module declarations**

`crates/chronacle-providers/src/lib.rs` should declare the modules (it already
contains `pub mod blob_store; pub mod embedding; pub mod llm_provider; pub mod
vector_store;`). Add a crate doc line at top:

```rust
//! Concrete provider implementations (LLM, embedding, vector store, blob store).
```

- [ ] **Step 4: Fix internal `crate::` paths**

Within the moved files, replace any `crate::providers::` with `crate::` and any
`crate::schema::` with `chronacle_db::` (add `chronacle-db` dep if the embedding
consistency check references migrations in tests). Search:

```bash
grep -rn "crate::providers::\|crate::schema::\|crate::services::" crates/chronacle-providers/src
```
Replace `crate::providers::X` → `crate::X`. If any `crate::services::` appears
(providers should not depend on services), flag it — it likely belongs to a test
that moves in a later task; if so, move that test out with its service in B5/B6.

- [ ] **Step 5: Point `src-tauri` at the crate**

In `src-tauri/Cargo.toml` add
`chronacle-providers = { path = "../crates/chronacle-providers" }` and remove the
now-crate-owned deps that are no longer used directly by `src-tauri` only if
unused (keep `fastembed`, `pdfium-render` etc. if `src-tauri` still references
them directly — verify with the compiler). In `src-tauri/src/lib.rs` remove `pub
mod providers;` and replace `providers::` / `crate::providers::` with
`chronacle_providers::`:

```bash
grep -rl "crate::providers::\|\bproviders::" src-tauri/src | xargs sed -i '' 's/crate::providers::/chronacle_providers::/g; s/\bproviders::/chronacle_providers::/g'
```
Then manually fix the `use providers::...` imports at the top of `lib.rs` to
`use chronacle_providers::...`.

- [ ] **Step 6: Build the workspace**

Run: `cargo build --workspace`
Expected: builds. Resolve any leftover path errors the compiler reports.

- [ ] **Step 7: Test providers + app**

Run: `cargo test -p chronacle-providers && cargo test -p Chronacle`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: extract chronacle-providers crate"
```

### Task B5: Extract `chronacle-ingestion`

**Files:**
- Create: `crates/chronacle-ingestion/Cargo.toml`
- Move: `pdf_extractor/`, `chunker/`, `ingestion_service/`, `text_normalizer.rs`
- Create: `crates/chronacle-ingestion/src/lib.rs`

- [ ] **Step 1: Create the manifest**

`crates/chronacle-ingestion/Cargo.toml`:

```toml
[package]
name = "chronacle-ingestion"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_ingestion"

[dependencies]
chronacle-core = { path = "../chronacle-core" }
surrealdb.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
thiserror.workspace = true
regex.workspace = true
unicode-segmentation.workspace = true
pdfium-render.workspace = true

[dev-dependencies]
chronacle-db = { path = "../chronacle-db" }
chronacle-providers = { path = "../chronacle-providers" }   # for MockEmbeddingProvider in tests only
mockall.workspace = true
tempfile.workspace = true
pretty_assertions.workspace = true
lopdf.workspace = true
tokio.workspace = true
```

> `chronacle-ingestion` lib-depends on `chronacle-core` (the `EmbeddingProvider`
> / `VectorStore` traits), **not** on `chronacle-providers`. The providers crate
> is a dev-dependency only, used by `ingestion_service/tests.rs` for
> `MockEmbeddingProvider`. This keeps the crate cloud-reusable.

- [ ] **Step 2: Move the modules**

```bash
mkdir -p crates/chronacle-ingestion/src
git mv src-tauri/src/services/pdf_extractor crates/chronacle-ingestion/src/pdf_extractor
git mv src-tauri/src/services/chunker crates/chronacle-ingestion/src/chunker
git mv src-tauri/src/services/ingestion_service crates/chronacle-ingestion/src/ingestion_service
git mv src-tauri/src/services/text_normalizer.rs crates/chronacle-ingestion/src/text_normalizer.rs
```

- [ ] **Step 3: Create `lib.rs`**

`crates/chronacle-ingestion/src/lib.rs`:

```rust
//! PDF ingestion: extraction, chunking, and the indexing pipeline.
pub mod chunker;
pub mod ingestion_service;
pub mod pdf_extractor;
pub mod text_normalizer;
```

- [ ] **Step 4: Fix internal paths**

```bash
grep -rn "crate::services::\|crate::providers::\|crate::schema::" crates/chronacle-ingestion/src
```
Replace `crate::services::pdf_extractor` → `crate::pdf_extractor` (and chunker,
ingestion_service, text_normalizer). Replace **trait** references
`crate::providers::embedding::EmbeddingProvider` /
`crate::providers::vector_store::{VectorStore,SearchResult,IndexedChunk}` →
`chronacle_core::...` (the traits/DTOs moved to core in B2). In test modules,
`crate::providers::embedding::MockEmbeddingProvider` →
`chronacle_providers::embedding::MockEmbeddingProvider` (dev-dependency).
Replace `crate::schema::` → `chronacle_db::`. If ingestion references
`entity_service`/`wikilink` (extraction crate), the boundary is wrong — report
it.

- [ ] **Step 5: Point `src-tauri` at the crate**

`src-tauri/Cargo.toml`:
`chronacle-ingestion = { path = "../crates/chronacle-ingestion" }`.
In `src-tauri/src`, replace `crate::services::{pdf_extractor,chunker,ingestion_service,text_normalizer}`
and `services::pdf_extractor` etc. with `chronacle_ingestion::...`:

```bash
grep -rl "services::pdf_extractor\|services::chunker\|services::ingestion_service\|services::text_normalizer" src-tauri/src
```
Edit each to `chronacle_ingestion::<module>`. Remove the four `pub mod` lines from
`src-tauri/src/services/mod.rs`.

- [ ] **Step 6: Build, test, commit**

Run: `cargo build --workspace && cargo test -p chronacle-ingestion && cargo test -p Chronacle`
Expected: PASS.
```bash
git add -A
git commit -m "refactor: extract chronacle-ingestion crate (pdf, chunker, pipeline)"
```

### Task B6: Extract `chronacle-extraction`

**Files:**
- Create: `crates/chronacle-extraction/Cargo.toml`
- Move: `entity_service/`, `wikilink/`, `extraction_service/`
- Create: `crates/chronacle-extraction/src/lib.rs`

- [ ] **Step 1: Create the manifest**

`crates/chronacle-extraction/Cargo.toml`:

```toml
[package]
name = "chronacle-extraction"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_extraction"

[dependencies]
chronacle-core = { path = "../chronacle-core" }
surrealdb.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
thiserror.workspace = true
regex.workspace = true

[dev-dependencies]
chronacle-db = { path = "../chronacle-db" }
mockall.workspace = true
tempfile.workspace = true
pretty_assertions.workspace = true
tokio.workspace = true
```

> `chronacle-extraction` depends only on `chronacle-core` for traits — **no**
> dependency on `chronacle-providers`, not even in tests: `test_support.rs`
> defines its own `MockLlm`, `BranchingLlm`, and `MockVectorStore` implementing
> the core traits directly. This is the cleanest cloud-reuse boundary in the
> workspace.

- [ ] **Step 2: Move the modules**

```bash
mkdir -p crates/chronacle-extraction/src
git mv src-tauri/src/services/entity_service crates/chronacle-extraction/src/entity_service
git mv src-tauri/src/services/wikilink crates/chronacle-extraction/src/wikilink
git mv src-tauri/src/services/extraction_service crates/chronacle-extraction/src/extraction_service
```

- [ ] **Step 3: Create `lib.rs`**

`crates/chronacle-extraction/src/lib.rs`:

```rust
//! Entity extraction: entity CRUD/relations, wikilink resolution, LLM extraction.
pub mod entity_service;
pub mod extraction_service;
pub mod wikilink;
```

- [ ] **Step 4: Replace the `settings_service` test coupling**

In `crates/chronacle-extraction/src/extraction_service/seed_tests.rs`, the two
calls `crate::services::settings_service::upsert(&db, "extraction_enrich_neighbors", "true")`
must become direct DB upserts (settings_service stays in the app):

```rust
db.query("UPSERT setting:extraction_enrich_neighbors SET value = 'true'")
    .await
    .unwrap();
```

- [ ] **Step 5: Fix internal paths**

```bash
grep -rn "crate::services::\|crate::providers::\|crate::schema::" crates/chronacle-extraction/src
```
Replace `crate::services::entity_service` → `crate::entity_service` (and
wikilink, extraction_service). Replace trait references
`crate::providers::llm_provider::{ChatMessage,LlmProvider}` and
`crate::providers::vector_store::{VectorStore,SearchResult,IndexedChunk}` →
`chronacle_core::...` (these are the only provider references in extraction — all
to traits/DTOs, none to concrete impls). Replace `crate::schema::` →
`chronacle_db::`. If a `chronacle_providers::` reference remains after this, it
means a concrete impl leaked into extraction — report it.

- [ ] **Step 6: Point `src-tauri` at the crate**

`src-tauri/Cargo.toml`:
`chronacle-extraction = { path = "../crates/chronacle-extraction" }`.
Replace `services::{entity_service,wikilink,extraction_service}` and
`crate::services::...` in `src-tauri/src` with `chronacle_extraction::...`.
Remove those three `pub mod` lines from `src-tauri/src/services/mod.rs`.

- [ ] **Step 7: Build, test, commit**

Run: `cargo build --workspace && cargo test -p chronacle-extraction && cargo test -p Chronacle`
Expected: PASS.
```bash
git add -A
git commit -m "refactor: extract chronacle-extraction crate (entity, wikilink, extraction)"
```

### Task B7: Extract `chronacle-retrieval` and `chronacle-domain`

**Files:**
- Create: `crates/chronacle-retrieval/Cargo.toml`, `src/lib.rs`; move `agent_service/`
- Create: `crates/chronacle-domain/Cargo.toml`, `src/lib.rs`; move CRUD services

- [ ] **Step 1: Create `chronacle-domain` manifest**

`crates/chronacle-domain/Cargo.toml`:

```toml
[package]
name = "chronacle-domain"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_domain"

[dependencies]
chronacle-core = { path = "../chronacle-core" }
surrealdb.workspace = true
serde.workspace = true
serde_json.workspace = true
uuid.workspace = true
thiserror.workspace = true

[dev-dependencies]
chronacle-db = { path = "../chronacle-db" }
tempfile.workspace = true
pretty_assertions.workspace = true
tokio.workspace = true
```

- [ ] **Step 2: Move CRUD services into domain**

```bash
mkdir -p crates/chronacle-domain/src
git mv src-tauri/src/services/campaign_service.rs crates/chronacle-domain/src/campaign_service.rs
git mv src-tauri/src/services/session_service.rs crates/chronacle-domain/src/session_service.rs
git mv src-tauri/src/services/collection_service crates/chronacle-domain/src/collection_service
git mv src-tauri/src/services/custom_provider_service.rs crates/chronacle-domain/src/custom_provider_service.rs
```

`crates/chronacle-domain/src/lib.rs`:

```rust
//! Campaign, session, collection, and custom-provider CRUD services.
pub mod campaign_service;
pub mod collection_service;
pub mod custom_provider_service;
pub mod session_service;
```

- [ ] **Step 3: Create `chronacle-retrieval` manifest and move `agent_service`**

`crates/chronacle-retrieval/Cargo.toml`:

```toml
[package]
name = "chronacle-retrieval"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lib]
name = "chronacle_retrieval"

[dependencies]
chronacle-core = { path = "../chronacle-core" }
chronacle-extraction = { path = "../chronacle-extraction" }
surrealdb.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
thiserror.workspace = true

[dev-dependencies]
chronacle-db = { path = "../chronacle-db" }
chronacle-providers = { path = "../chronacle-providers" }   # remove if agent_service tests define their own mocks
tempfile.workspace = true
pretty_assertions.workspace = true
tokio.workspace = true
```

> `chronacle-retrieval` lib-depends on `chronacle-core` (LLM/vector traits) and
> `chronacle-extraction` (entity types used in hybrid retrieval). It does **not**
> lib-depend on `chronacle-providers`. Inspect `agent_service`'s test modules:
> if they define their own mocks (like extraction), drop the providers
> dev-dependency; if they use `MockEmbeddingProvider`/`NoopProvider`, keep it.

```bash
mkdir -p crates/chronacle-retrieval/src
git mv src-tauri/src/services/agent_service crates/chronacle-retrieval/src/agent_service
```

`crates/chronacle-retrieval/src/lib.rs`:

```rust
//! RAG retrieval and cited-answer generation.
pub mod agent_service;
```

> The `chronacle-extraction` dep on retrieval is included because hybrid
> retrieval may reference entity types. If `cargo build` shows it is unused,
> remove it. If instead `agent_service` is referenced **by** extraction (reverse
> edge → cycle), apply the cycle rule from the spec: merge `retrieval` into
> `extraction`. Confirm direction in Step 5.

- [ ] **Step 4: Fix internal paths in both crates**

```bash
grep -rn "crate::services::\|crate::providers::\|crate::schema::" crates/chronacle-domain/src crates/chronacle-retrieval/src
```
- `crate::services::campaign_service` → `crate::campaign_service` (domain).
- `crate::services::agent_service` → `crate::agent_service` (retrieval).
- `crate::services::{entity_service,extraction_service,wikilink}` →
  `chronacle_extraction::...`.
- trait references `crate::providers::{llm_provider,vector_store,embedding}::<Trait/DTO>`
  → `chronacle_core::...`; `crate::schema::` → `chronacle_db::`. Only if a
  *concrete* provider impl is referenced (e.g. in a test) use
  `chronacle_providers::...`.
- The `campaign_service::create`-shaped UUID comment in
  `agent_service/persistence.rs` is just a comment — no change.

- [ ] **Step 5: Point `src-tauri` at both crates and remove `services/mod.rs` entries**

`src-tauri/Cargo.toml`: add
`chronacle-domain = { path = "../crates/chronacle-domain" }` and
`chronacle-retrieval = { path = "../crates/chronacle-retrieval" }`.
Replace `services::{campaign_service,session_service,collection_service,custom_provider_service}`
→ `chronacle_domain::...` and `services::agent_service` →
`chronacle_retrieval::...` in `src-tauri/src`. `src-tauri/src/services/mod.rs`
should now contain only `pub mod settings_service;`.

- [ ] **Step 6: Build the full workspace**

Run: `cargo build --workspace`
Expected: builds. If a cycle error appears between retrieval/extraction, merge
per Step 3's note and re-run.

- [ ] **Step 7: Test + commit**

Run: `cargo test -p chronacle-domain && cargo test -p chronacle-retrieval && cargo test -p Chronacle`
Expected: PASS.
```bash
git add -A
git commit -m "refactor: extract chronacle-domain and chronacle-retrieval crates"
```

### Task B8: Hoist cross-crate types into `chronacle-core` & relocate integration tests

**Files:**
- Modify: `crates/chronacle-core/src/lib.rs` and importers
- Move: `src-tauri/tests/*` → owning crates' `tests/`

- [ ] **Step 1: Find duplicated/cross-imported types**

By this point the workspace builds, meaning cross-crate type access already works
via the owning crate's public re-exports (e.g. a domain type imported as
`chronacle_extraction::entity_service::Entity`). Decide which, if any, truly
belong in `core` because **two or more sibling crates** import them. List them:

```bash
grep -rn "chronacle_extraction::\|chronacle_ingestion::\|chronacle_providers::" crates/*/src | grep -v "^crates/chronacle-providers" | sort
```
Only move a type to `chronacle-core` if a sibling crate imports it AND importing
through the owning crate would create an unwanted dependency. Otherwise leave it
(YAGNI). Document the moved set in the commit message.

- [ ] **Step 2: Relocate integration tests to their owning crate**

Move each `src-tauri/tests/*` file to the crate that owns the code it exercises,
updating `chronacle_lib::` references to the new crate path:

```bash
git mv src-tauri/tests/entity_service_test.rs crates/chronacle-extraction/tests/entity_service_test.rs
git mv src-tauri/tests/pdf_fixture_ingest.rs crates/chronacle-ingestion/tests/pdf_fixture_ingest.rs
git mv src-tauri/tests/retrieval_recall.rs crates/chronacle-retrieval/tests/retrieval_recall.rs
git mv src-tauri/tests/rag_quality_integration.rs crates/chronacle-retrieval/tests/rag_quality_integration.rs
git mv src-tauri/tests/campaign_service_test.rs crates/chronacle-domain/tests/campaign_service_test.rs
git mv src-tauri/tests/session_service_test.rs crates/chronacle-domain/tests/session_service_test.rs
git mv src-tauri/tests/collection_service_test.rs crates/chronacle-domain/tests/collection_service_test.rs
git mv src-tauri/tests/settings_service_test.rs src-tauri/tests/settings_service_test.rs   # stays
```

Leave app-level cross-cutting tests in `src-tauri/tests/`:
`integration_test.rs`, `chat_history_test.rs`, `e2e_campaign_notes_query.rs`
(these likely wire multiple crates + commands — keep them in the app, which
depends on all crates). Move the shared `tests/fixtures/` only if a single crate
needs it; otherwise reference fixtures via a relative path constant, or duplicate
the minimal fixture into the crate that needs it. Inspect each moved test's
`chronacle_lib::...` imports and rewrite to the proper `chronacle_<crate>::...`
path.

- [ ] **Step 3: Build all tests across the workspace**

Run: `cargo test --workspace --no-run`
Expected: all test binaries compile. Fix import paths the compiler flags.

- [ ] **Step 4: Full Stage B gate**

Run:
```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --check
```
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: relocate integration tests and hoist cross-crate types (Stage B complete)"
```

---

## STAGE C — move the frontend under `apps/desktop/` (last)

Goal: co-locate Svelte + Tauri under `apps/desktop/`. Highest-risk stage; gated
by an actual UI-E2E build. Do not widen any Tauri capability scope to fix a path.

### Task C1: Move the Tauri Rust crate under `apps/desktop/`

**Files:**
- Move: `src-tauri/` → `apps/desktop/src-tauri/`
- Modify: root `Cargo.toml` member path; crate path-deps inside the moved crate

- [ ] **Step 1: Move the directory**

```bash
mkdir -p apps/desktop
git mv src-tauri apps/desktop/src-tauri
```

- [ ] **Step 2: Update workspace member path**

In the root `Cargo.toml`, change the `"src-tauri"` member to
`"apps/desktop/src-tauri"`.

- [ ] **Step 3: Fix the path dependencies inside the moved crate**

`apps/desktop/src-tauri/Cargo.toml` path deps were `../crates/...`; from the new
location they are `../../../crates/...`. Update each
`chronacle-* = { path = "../crates/chronacle-*" }` to
`{ path = "../../../crates/chronacle-*" }`. Also fix `license-file = "../LICENSE"`
→ `license-file = "../../../LICENSE"` (or switch to `license.workspace = true`).

- [ ] **Step 4: Build the workspace**

Run: `cargo build --workspace`
Expected: builds. The `build.rs` and `resources/` move with the crate, so the
pdfium/ONNX paths (resolved via `CARGO_MANIFEST_DIR`) remain correct.

- [ ] **Step 5: Test + commit**

Run: `cargo test --workspace`
Expected: PASS.
```bash
git add -A
git commit -m "refactor: move tauri crate to apps/desktop/src-tauri"
```

### Task C2: Move the Svelte frontend under `apps/desktop/`

**Files:**
- Move: `src/`, `index.html`, `vite.config.ts`, `svelte.config.js`,
  `tsconfig.json`, `eslint.config.js`, `.prettierrc`, `tests/e2e/`,
  `playwright.config.ts` → `apps/desktop/`
- Create: `apps/desktop/package.json`; modify root `package.json` +
  `pnpm-workspace.yaml`

- [ ] **Step 1: Move frontend source and config**

```bash
git mv src apps/desktop/src
git mv index.html apps/desktop/index.html
git mv vite.config.ts apps/desktop/vite.config.ts
git mv svelte.config.js apps/desktop/svelte.config.js
git mv tsconfig.json apps/desktop/tsconfig.json
git mv eslint.config.js apps/desktop/eslint.config.js
git mv .prettierrc apps/desktop/.prettierrc
git mv playwright.config.ts apps/desktop/playwright.config.ts
git mv tests apps/desktop/tests
```

- [ ] **Step 2: Split `package.json` into root workspace + app**

Create `apps/desktop/package.json` containing the app's `name`, `scripts`
(`dev`, `build`, `preview`, `tauri`, `typecheck`, `lint`, `test`, `test:run`,
`test:coverage`, `e2e:ui`), and **all** the current `dependencies` +
`devDependencies` from the root `package.json`. Keep the root `package.json`
minimal:

```json
{
  "name": "chronacle-monorepo",
  "private": true,
  "license": "AGPL-3.0 WITH branding-exception",
  "packageManager": "pnpm@11.5.1+sha512.93f7b57422ea7068257235b4c16eb60762eb68e1dc23723199cc739043ea9be2c4143274a399d8c6defa2b1176226d9ca1c4b63482d6200c1a8fbaa78c1d1485"
}
```

Update `pnpm-workspace.yaml` to add a `packages:` list:

```yaml
packages:
  - apps/desktop
allowBuilds:
  esbuild: true
  msw: true
settings:
  minimumReleaseAge: 0
```

- [ ] **Step 3: Reinstall to relink the workspace**

Run: `pnpm install`
Expected: resolves `apps/desktop` as a workspace package; lockfile updates via
pnpm (do not hand-edit `pnpm-lock.yaml`).

- [ ] **Step 4: Fix Vite/Tauri/Playwright paths**

- `apps/desktop/vite.config.ts`: the `ignored: ['**/src-tauri/**']` glob still
  matches (`apps/desktop/src-tauri`). No change needed unless `root` was set
  (it isn't). Vite `root` defaults to the config dir (`apps/desktop`), so
  `index.html` resolves; build output goes to `apps/desktop/dist`.
- `apps/desktop/src-tauri/tauri.conf.json`: `frontendDist` is `"../dist"` →
  now resolves to `apps/desktop/dist` ✓. `beforeBuildCommand`/`beforeDevCommand`
  run from the frontend dir; ensure they run in `apps/desktop` (Tauri runs them
  from the tauri.conf parent = `apps/desktop`) — `pnpm build`/`pnpm dev` work
  there. No change unless commands must be `pnpm --filter`.
- `apps/desktop/playwright.config.ts` and `apps/desktop/tests/e2e/`: update any
  paths that referenced repo-root `dist/` or `src-tauri/` to the new relative
  locations.

- [ ] **Step 5: Verify frontend toolchain**

Run from `apps/desktop`:
```bash
pnpm --filter chronacle dev --version >/dev/null 2>&1 || true
pnpm -C apps/desktop typecheck
pnpm -C apps/desktop lint
pnpm -C apps/desktop test:run
pnpm -C apps/desktop build
```
Expected: typecheck, lint, vitest, and the production build all PASS, producing
`apps/desktop/dist/`.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: move svelte frontend under apps/desktop (Stage C)"
```

### Task C3: Verify the UI-E2E still embeds the SPA and IPC origin is correct

**Files:**
- Verify only (no edits unless a path breaks)

- [ ] **Step 1: Build the UI-E2E app via the Tauri CLI (not plain cargo)**

Per the `e2e_ui_release_build_no_spa` learning, build the UI-E2E binary with the
CLI so `frontendDist` is embedded:

```bash
pnpm -C apps/desktop exec tauri build --no-bundle
```
Expected: succeeds; the built binary embeds `apps/desktop/dist`.

- [ ] **Step 2: Run the UI-E2E suite**

Run: `pnpm -C apps/desktop playwright test tests/e2e/ui/`
(or the `e2e:ui` mocha script if that is the canonical UI runner:
`pnpm -C apps/desktop run e2e:ui`)
Expected: PASS — webview navigates to `tauri://localhost/` (not `about:blank`),
IPC origin is valid, SPA is served. If the SPA fails to embed, the cause is a
`frontendDist` path mismatch from C2 — fix the path, do not widen capabilities.

- [ ] **Step 3: Run the backend E2E suite**

Run: `pnpm -C apps/desktop playwright test tests/e2e/backend/`
Expected: PASS.

- [ ] **Step 4: Commit any path fixes**

```bash
git add -A
git commit -m "test: verify UI/backend E2E green after frontend relocation"
```

---

## STAGE D — documentation & CI

### Task D1: Update CI workflows and config paths

**Files:**
- Modify: `.github/workflows/ci.yml`, `e2e-ui.yml`, `release.yml`
- Modify: `lefthook.yml`, `deny.toml`, `mise.toml` (if they reference paths)

- [ ] **Step 1: Find path references**

```bash
grep -rn "src-tauri\|pnpm \|working-directory\|src/\|tests/e2e\|/dist" .github/workflows lefthook.yml deny.toml mise.toml
```

- [ ] **Step 2: Update each to the new layout**

For each hit: `src-tauri/` → `apps/desktop/src-tauri/`; frontend `pnpm`
commands gain `-C apps/desktop` (or a `working-directory: apps/desktop`);
`cargo` commands become `cargo <cmd> --workspace` where they previously targeted
the single crate. In `lefthook.yml`, ensure the rustfmt/clippy hooks run
`--workspace` and the prettier/eslint hooks point at `apps/desktop`.

- [ ] **Step 3: Run hooks locally**

Run: `lefthook run pre-commit`
Expected: rustfmt, clippy, prettier, eslint all PASS against the new paths.

- [ ] **Step 4: Commit**

```bash
git add .github lefthook.yml deny.toml mise.toml
git commit -m "ci: update workflow and hook paths for monorepo layout"
```

### Task D2: Update `AGENTS.md`, `architecture.md`, `README.md`

**Files:**
- Modify: `AGENTS.md` (CLAUDE.md is a symlink — edit AGENTS.md only)
- Modify: `docs/architecture.md`
- Modify: `README.md`

- [ ] **Step 1: Rewrite the AGENTS.md "Project structure" tree**

Replace the `src/` and `src-tauri/` sections of the tree with the new
`crates/*` + `apps/desktop/` layout (mirror the File Structure section of this
plan). Update the **Commands** section: Rust commands become workspace-aware
(`cargo build --workspace`, `cargo test --workspace`), frontend commands run via
`pnpm -C apps/desktop ...`. Update the **Testing** section paths
(`crates/<x>/tests/`, `apps/desktop/tests/e2e/`). Update **Subagents** /
**Security & boundaries** path references (`src-tauri/capabilities/*` →
`apps/desktop/src-tauri/capabilities/*`).

- [ ] **Step 2: Update `docs/architecture.md`**

In the cloud/Phase-4 section, replace the "tactical extraction (future)"
narrative with the realised crate DAG (core ← db/providers/ingestion/extraction/
retrieval/domain ← apps/desktop). State that the future axum binary becomes
`apps/server/` reusing the same `chronacle-*` crates. Record the trait-with-impl
decision from Task B2 and the final retrieval/extraction edge from Task B7.

- [ ] **Step 3: Update `README.md`**

Update build/run instructions (`cargo tauri dev` is now
`pnpm -C apps/desktop tauri dev`) and any directory diagram.

- [ ] **Step 4: Final full verification gate**

Run:
```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --check
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run
```
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md docs/architecture.md README.md
git commit -m "docs: update structure, commands, and architecture for monorepo"
```

---

## Verification summary (per stage)

| Stage | Gate command(s) | Proves |
|---|---|---|
| A | `cargo test --all-targets` + clippy + fmt | genericity is behaviour-preserving |
| B | `cargo test --workspace` + clippy + fmt | crate split changed no behaviour |
| C | `pnpm -C apps/desktop build` + `tauri build --no-bundle` + UI/backend E2E | frontend relocation keeps SPA embedded + IPC origin valid |
| D | full workspace test + frontend checks + `lefthook run pre-commit` | CI/docs consistent with layout |

## Rollback notes

Each task is a single commit; revert the offending commit to roll back one step.
Stage A is independent of B/C/D and can ship alone if later stages stall. If
`engine::any` cannot reach embedded RocksDB (Task A1 fallback), stop at Stage A
and re-consult — the crate split (Stage B) does not require the genericity change
to compile, but the crates would not be cloud-reusable without it.
