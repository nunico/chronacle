# Monorepo restructure — reusable core crates — Design

**Date:** 2026-06-28
**Status:** Approved, ready for implementation planning
**Phase:** Cross-cutting (enables Phase 4 — Cloud / Mobile)

## Problem

Chronacle is a single Cargo crate (`src-tauri`) plus a root-level `pnpm`
frontend. All backend logic — PDF ingestion, entity extraction, RAG retrieval,
provider impls — lives inside the Tauri application crate. The architecture doc
(`docs/architecture.md`) frames the cloud path as a *tactical extraction*: the
service layer becomes an axum HTTP server, embedded SurrealDB becomes SurrealDB
Cloud, IPC handlers become route handlers. That extraction is far easier if the
reusable logic already lives in standalone, Tauri-free crates.

Today the seams are good but unrealised:

- `services/` and `providers/` have **zero** Tauri coupling — `tauri::` types
  appear only in `commands/` and `lib.rs`.
- The code is **already mostly connection-generic**: 56 service function
  signatures take `db: &Surreal<C>`; `SurrealDbVector<C: Connection>` is
  generic; `run_migrations<C>` is generic. Only `AppState.db` and two services
  (`campaign_service`, `custom_provider_service`) hardcode the embedded
  `engine::local::Db`.

The work is to **promote those seams into real crate boundaries** so the
ingestion and extraction logic (and the rest of the service layer) can be reused
by a future cloud binary without a rewrite.

## Goals

- Turn the codebase into a Cargo workspace of focused library crates under
  `crates/`, with the Tauri app as a thin shell under `apps/desktop/`.
- Extract the two named core modules — **PDF ingestion** and **entity
  extraction** — plus the supporting infra (core types/traits, providers,
  schema/db) and the rest of the service layer into reusable crates.
- Make the extracted crates genuinely cloud-reusable: they must compile against
  SurrealDB Cloud, i.e. not hardcode the embedded engine type.
- Co-locate the Svelte frontend with the Tauri shell under `apps/desktop/`.
- Keep the build, the test suite, and the UI-E2E pipeline green throughout.
- Update all documentation (`AGENTS.md`/`CLAUDE.md`, `docs/architecture.md`,
  `README.md`) to match the new layout.

## Non-goals (YAGNI)

- Writing the axum cloud binary. This restructure *enables* it; it does not
  build it.
- Changing any business logic, query, schema, or public behaviour. Stages B and
  C are mechanical; only Stage A changes types.
- Splitting the frontend into multiple JS packages. The Svelte app moves as one
  unit.
- Publishing any crate to crates.io (all stay `publish = false`).
- Embedding `.surql` via `include_str!` (optional cleanup; noted as future work).

## Target layout

```text
chronacle/
├── Cargo.toml                  # workspace root: members + [workspace.dependencies]
├── crates/
│   ├── chronacle-core/         # shared types, errors, dependency TRAITS, Db helpers
│   ├── chronacle-db/           # schema/*.surql + run_migrations
│   ├── chronacle-providers/    # concrete impls (LLM, embedding, vector, blob)
│   ├── chronacle-ingestion/    # pdf_extractor + chunker + ingestion_service + text_normalizer
│   ├── chronacle-extraction/   # entity_service + wikilink + extraction_service
│   ├── chronacle-retrieval/    # agent_service (RAG retrieval + cited answer)
│   └── chronacle-domain/       # campaign + session + collection + custom_provider (CRUD)
└── apps/
    └── desktop/                # the Tauri app (current src-tauri) + Svelte frontend
        ├── src/                # Svelte frontend (moved from repo-root src/)
        ├── src-tauri/          # Rust: commands/, lib.rs, AppState, main.rs, settings_service
        │   ├── capabilities/
        │   ├── resources/
        │   └── tauri.conf.json
        ├── index.html
        ├── vite.config.ts
        └── (svelte/ts/playwright config as needed)
```

> The exact placement of `apps/desktop/src-tauri` vs flattening the Tauri Rust
> directly under `apps/desktop` is an implementation detail resolved in Stage C;
> the constraint is that `tauri.conf.json`, `frontendDist`, and `devUrl` stay
> consistent and the SPA still embeds.

### Crate responsibilities & dependency direction

The dependency graph must be acyclic. Crates depend on the **traits in
`chronacle-core`**, not on `chronacle-providers` concretes — the app wires the
concrete providers in. This is the property that makes the crates cloud-reusable.

| Crate | Contains | Depends on |
|-------|----------|-----------|
| `chronacle-core` | domain types/records/DTOs, error types, dependency traits (`LlmProvider`, `VectorStore`, `BlobStore`, `EmbeddingProvider`), `Db` helpers/alias | (leaf) `surrealdb`, `serde`, `thiserror`, `async-trait` |
| `chronacle-db` | `schema/*.surql`, `run_migrations<C>` | `core` (if needed) |
| `chronacle-providers` | `SurrealDbVector`, `LocalFileStore`, fastembed/OpenAI/Mock embedding, OpenAI/Anthropic/Ollama LLM | `core` |
| `chronacle-ingestion` | `pdf_extractor`, `chunker`, `ingestion_service`, `text_normalizer` | `core` |
| `chronacle-extraction` | `entity_service`, `wikilink`, `extraction_service` | `core` (+ `domain` if a real edge exists) |
| `chronacle-retrieval` | `agent_service` | `core` (+ `extraction`/`domain` if real edges exist) |
| `chronacle-domain` | `campaign_service`, `session_service`, `collection_service`, `custom_provider_service` | `core` |
| `apps/desktop` (Tauri) | `commands/`, `lib.rs`, `AppState`, `main.rs`, `settings_service`, frontend | all crates above + `tauri` |

**Cycle rule:** the intended edges between `extraction`, `retrieval`, and
`domain` will be confirmed during Stage B. If a genuine cycle appears (e.g.
`extraction` ↔ `domain`), the two crates are merged rather than introducing a
back-edge. Document the final DAG in the architecture doc.

### Why these groupings

- **ingestion** and **extraction** are the two modules the restructure is for.
  Their internal modules are tightly bound (`chunker`↔`pdf_extractor`;
  `entity_service`↔`wikilink`↔`extraction_service`), so each is one crate.
- **`text_normalizer`** → `chronacle-ingestion`: its only consumer is the
  chunker.
- **`settings_service`** → stays in `apps/desktop`. No extracted crate depends
  on it in production: `extraction` reads the `extraction_enrich_neighbors`
  setting via a direct `db.query("SELECT * FROM setting:…")`, not via
  `settings_service`. The only references are two `upsert` calls in
  `extraction_service/seed_tests.rs`, which become direct `db.query` upserts (or
  a small in-crate test helper) when extraction moves.
- **traits in `core`, impls in `providers`**: keeps `ingestion`/`extraction`/
  `retrieval` independent of concrete provider crates — they accept
  `Arc<dyn LlmProvider>` etc., matching the existing hard constraint.

## Approach: three independently-verifiable stages

The single most important design decision: **do not ride the transformations
together.** Each stage is one (or a few) commits with a green `cargo test` /
`cargo clippy` before moving on, so any breakage has an unambiguous cause.

### Stage A — DB genericity (no files move)

Goal: remove the last hardcoded embedded-engine references so services compile
against any SurrealDB connection. Done **inside the current single crate.**

1. Switch `AppState.db` from `Surreal<engine::local::Db>` to
   `Surreal<engine::any::Any>`.
2. In `init_database`, construct the connection via
   `surrealdb::engine::any::connect("rocksdb://<path>")` instead of
   `Surreal::new::<RocksDb>(path)`. **Verify** this yields the same embedded
   RocksDB engine and that `kv-rocksdb`/`kv-mem` features still satisfy the
   `any` router (add the `protocol`/`any` engine feature only if required — do
   not widen features speculatively).
3. Genericise the two straggler services (`campaign_service`,
   `custom_provider_service`) from `Surreal<Db>` to `Surreal<C: Connection>`,
   matching the other 56 signatures (~19 call sites).
4. Update the few command/test sites that name `engine::local::Db`. Tests using
   `engine::local::Mem` directly stay as-is (generic fns accept `Mem`).

**Exit criteria:** `cargo build`, `cargo test`, `cargo clippy --all-targets
--all-features -- -D warnings` all green. Commit. *No files have moved.*

### Stage B — split into crates (no type/logic changes)

Goal: pure mechanical extraction. Move files into `crates/*`, wire the
workspace. **No business-logic, query, or signature changes** beyond `use`-path
and visibility adjustments forced by the new module boundaries.

1. Create the workspace root `Cargo.toml` with `members = ["crates/*",
   "apps/desktop/src-tauri"]` and a `[workspace.dependencies]` table; hoist
   shared deps (serde, tokio, surrealdb, thiserror, async-trait, reqwest, …) and
   have each crate reference them with `.workspace = true`. Each crate sets
   `license.workspace = true` (do **not** edit the LICENSE files).
2. Move modules into their crates per the table above. In-file `#[cfg(test)]`
   tests move with their module for free.
3. Fix `use` paths: `crate::services::foo` → `chronacle_<crate>::foo`. Promote
   the visibility of items that cross a crate boundary (`pub(crate)` → `pub`)
   only where genuinely needed.
4. Relocate `src-tauri/tests/*` integration tests: each test moves to the crate
   that owns the code it exercises (e.g. `entity_service_test.rs` →
   `crates/chronacle-extraction/tests/`), updating `chronacle_lib::` references
   to the new crate paths. Cross-cutting app-level tests stay under the desktop
   Tauri crate.
5. Replace the two `seed_tests.rs` `settings_service::upsert` calls with direct
   `db.query` upserts.
6. Resolve any dependency cycle per the cycle rule above.

**Exit criteria:** full `cargo test --workspace` and clippy green. Commit.

### Stage C — move the frontend into `apps/desktop/` (last)

Goal: co-locate the Svelte app with its Tauri shell. This stage reopens the
SPA-embed / `frontendDist` area that the current branch
(`fix/e2e-ui-spa-embed-and-ipc-origin`) just fixed, so it goes **last** and is
verified by an actual UI-E2E build.

1. Move `src/`, `index.html`, `vite.config.ts`, `svelte.config.js`,
   `tsconfig.json`, `eslint.config.js`, `.prettierrc`, and the `tests/e2e/`
   suite under `apps/desktop/` (final sub-paths decided here).
2. Update `tauri.conf.json` `build.frontendDist` and `build.devUrl`, the Vite
   `root`/build output path, `pnpm-workspace.yaml` `packages:`, and Playwright
   config paths.
3. Keep `package.json` scripts working (adjust paths). Do **not** widen any
   Tauri capability scope to silence a path error.
4. Rebuild the UI-E2E app via `pnpm exec tauri build --no-bundle` (per the
   `e2e_ui_release_build_no_spa` learning) and run `pnpm playwright test
   tests/e2e/ui/` to confirm the SPA still embeds and IPC origin is correct.

**Exit criteria:** `pnpm build`, `pnpm typecheck`, `pnpm lint`, `pnpm test
--run`, backend E2E, and UI-E2E all green. Commit.

### Stage D — documentation

Update to match the new layout (can land with Stage C or separately):

- `AGENTS.md` (`CLAUDE.md` symlink): the **Project structure** tree, the
  **Commands** section (paths/cwd for `cargo`/`pnpm`), the **Testing** section,
  and any path references.
- `docs/architecture.md`: replace the desktop-vs-cloud "tactical extraction"
  narrative with the realised crate DAG; note that the cloud binary now depends
  on the same `chronacle-*` crates. Update the **Crate & Tool Summary** if crate
  names belong there.
- `README.md`: update build/run instructions and any structure diagram.
- Update `deny.toml`, `lefthook.yml`, `mise.toml`, and `.github/workflows/*`
  paths if they reference `src-tauri/` or root frontend paths.

## Error handling

No new runtime error paths. Risks are build-time and caught by the staged exit
criteria:

- **`any::connect` engine mismatch** (Stage A): verified before relying on it;
  fall back to keeping `engine::local::Db` if `any` can't reach the embedded
  RocksDB with current features (would revisit the genericity decision).
- **Visibility / path breakage** (Stage B): surfaced immediately by
  `cargo build`.
- **SPA fails to embed** (Stage C): caught by the UI-E2E build, the exact
  failure mode previously fixed on this branch.

## Testing

- **Stage A:** existing unit + integration suite must stay green; this proves
  the genericity change is behaviour-preserving.
- **Stage B:** `cargo test --workspace` green; integration tests relocated to
  the owning crate still pass. Per the regression-test learning, if a crate
  boundary breaks an invariant, add a test at the new boundary rather than only
  patching the call site.
- **Stage C:** frontend Vitest, backend Playwright E2E, and tauri-driver UI-E2E
  all green, including a fresh `tauri build --no-bundle` to prove embedding.
- No tests are deleted; tests move with the code they cover.

## Future work

- Embed `.surql` via `include_str!` in `chronacle-db` to drop the
  `CARGO_MANIFEST_DIR` filesystem read (more portable for packaged/cloud use);
  verify against the existing migration test first.
- Build the axum cloud binary as a new `apps/server/` (or `crates/`) member
  reusing `chronacle-core/db/providers/ingestion/extraction/retrieval`.
