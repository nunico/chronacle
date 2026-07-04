# TTRPG GM Agent — Architecture & Design

**Status:** Proposed  
**Date:** 2026-05-26  
**System:** System-agnostic (PDF-driven)  
**Target platforms:** Windows, Linux, macOS (cloud/mobile: later)

---

## Overview

A desktop application that lets a GM load their own TTRPG PDFs (rules, sourcebooks, lore), take structured notes, and query an AI agent for rules answers and lore lookups — with source citations on every response. The LLM backend is configurable at setup: cloud API or local via Ollama.

The backend runs in-process with the Tauri app, using IPC commands for request/response and Tauri events for streaming. All external dependencies (database, vector store, file storage) are behind traits. The service layer already lives in standalone `chronacle-*` library crates under `crates/` (realisable in a future cloud deployment); the Tauri shell in `apps/desktop/src-tauri/` is a thin adapter that wires them together.

---

## ADR-001: Application Framework — Tauri 2 (Rust + TypeScript/Svelte)

**Status:** Proposed

### Context

The app must run on Windows, Linux, and macOS with no complicated setup. It needs a UI and a capable backend for PDF processing, embedding, and vector search.

### Options Considered

| Dimension          | Tauri 2 (Rust backend)                     | Electron (Node backend)        | Native per-platform    |
| ------------------ | ------------------------------------------ | ------------------------------ | ---------------------- |
| Binary size        | ~8–15 MB                                   | ~200 MB                        | Small but 3× codebase  |
| Runtime required   | None (WebView2 on Win, built-in elsewhere) | Bundled Node                   | None                   |
| Rust ecosystem fit | Native — PDF, embeddings, SurrealDB        | Needs FFI or re-implementation | N/A                    |
| Setup for user     | Double-click installer                     | Double-click installer         | Double-click installer |
| Memory footprint   | Low                                        | ~300 MB base                   | Low                    |

**Decision:** Tauri 2 with a Rust backend and a TypeScript/Svelte frontend.

**Consequences:**

- Backend logic lives in Rust; frontend in TypeScript/Svelte.
- The frontend communicates with the backend via **Tauri IPC commands** (type-safe, zero-serialization-cost) and **Tauri events** for streaming responses. See ADR-005.
- WebView2 must be present on Windows (auto-installed by the Tauri bootstrapper).

---

## ADR-002: Unified Store — SurrealDB (Relational + Vector + Graph)

**Status:** Proposed

### Context

The app needs relational storage (campaigns, sources, entities, sessions), vector storage (embedding chunks for RAG), and graph relationships (entity-to-entity links). Traditionally this means three storage engines — a pattern that complicates backups, transactional consistency, and deployment.

### Options Considered

| Dimension                 | SurrealDB                             | SQLite + LanceDB                    | SQLite + sqlite-vec                  |
| ------------------------- | ------------------------------------- | ----------------------------------- | ------------------------------------ |
| Stores                    | Relational + vector + graph (unified) | Relational + vector (split)         | Relational + vector (sqlx extension) |
| Rust SDK                  | Native (`surrealdb` crate)            | `sqlx` + `lancedb`                  | `sqlx` + rusqlite                    |
| Graph edges               | Native (RELATION tables)              | Manual JOINs on entity_links        | Manual JOINs on entity_links         |
| Vector indexes            | MTREE index (COSINE distance)         | Columnar ANN                        | Via sqlite-vec extension             |
| Cloud path                | SurrealDB Cloud                       | LanceDB Cloud + PostgreSQL          | PostgreSQL + pgvector                |
| Test isolation            | In-memory engine (`mem::Db`)          | Two temp dirs (sqlx::test + tmpdir) | sqlx::test (single DB)               |
| Transactional consistency | Single store — atomic updates         | Two stores — no cross-store tx      | Single DB — atomic updates           |
| Migration tooling         | SurrealQL schema definitions          | sqlx migrations (UP + DOWN)         | sqlx migrations                      |
| Learning curve            | SurrealQL (new dialect)               | SQL + LanceDB API                   | SQL + sqlite-vec                     |

**Decision:** SurrealDB as the single store for relational data, vector indexes, and graph edges. The unified engine simplifies the architecture (one backup, one connection, one migrations system) and enables graph traversal that would otherwise require manual JOIN chains or application-level traversal code.

**Consequences:**

- The project uses **SurrealQL** instead of SQL. No compile-time query validation (unlike `sqlx::query!`), so query errors are caught in tests rather than at compile time.
- Embedded mode uses RocksDB under the hood — first `cargo build` is slower (~30–60 s for the C++ transitive build).
- The `VectorStore` trait is retained for abstraction, but its SurrealDB implementation is the primary; a future `QdrantCloud` implementation can be added when the cloud path is needed.
- Schema migrations use SurrealQL `DEFINE` statements in ordered `.surql` files, applied at startup. SurrealDB's schema changes are mostly additive — no rollback mechanism (unlike sqlx's UP/DOWN migrations).

---

## ADR-003: Embeddings — fastembed-rs (Bundled Local Model)

**Status:** Proposed

### Options Considered

| Option                                   | Quality        | Setup                  | Offline |
| ---------------------------------------- | -------------- | ---------------------- | ------- |
| OpenAI `text-embedding-3-small`          | Excellent      | API key                | No      |
| Ollama embedding model                   | Good           | Requires Ollama        | Yes     |
| `fastembed-rs` (`nomic-embed-text-v1.5`) | Good (768-dim) | Downloads on first run | Yes     |

**Decision:** `fastembed-rs` as the default. Optional override to use cloud embedding API in settings.

**Critical constraint:** The embedding model is baked into the vector index. Switching models requires full re-indexing. The settings screen must warn the user and provide a batch re-index workflow: show which sources are affected, allow one-click re-index with progress, and clean up orphaned old chunks.

Store the model identifier alongside each table in SurrealDB so a mismatch is detectable at startup. The re-index orchestration handles: (1) detecting affected sources, (2) queuing them for re-indexing, (3) purging old chunks, and (4) streaming progress per source so the GM can see where the process stands.

**Asymmetric prefixes.** `nomic-embed-text-v1.5` was trained with task prefixes
and **requires** them at inference time: `search_document: <text>` for indexed
chunks and `search_query: <text>` for user queries. `fastembed-rs` (unlike the
Python `fastembed` library) does not add these automatically. Prefixing is
enforced inside `FastEmbedProvider::embed_documents()` and
`FastEmbedProvider::embed_query()`; callers MUST pass un-prefixed text. Missing
prefixes silently degrade retrieval recall (the failure mode that motivated this
change — see `docs/superpowers/plans/2026-05-31-rag-quality-improvements.md`).

**ONNX Runtime provisioning.** `fastembed` is built with the `ort-load-dynamic`
feature, so ONNX Runtime is loaded from a dynamic library at runtime rather than
linked. `build.rs` downloads the matching ONNX Runtime binary (pinned to the
version `ort-sys` expects — currently **1.24.2**) for the build target into
`apps/desktop/src-tauri/resources/onnxruntime/`, mirroring the pdfium provisioning. Tauri
bundles it via `bundle.resources`; `embedding.rs::ensure_ort_dylib_path()`
resolves the bundled library and sets `ORT_DYLIB_PATH` before the first session
is created (dev resolves via `CARGO_MANIFEST_DIR`, bundled via the executable's
resource dir). **Without this, every `FastEmbedProvider::try_new` fails at ONNX
session creation and the app silently falls back to the mock provider.** Supported
targets: macOS arm64, Linux x86_64/aarch64, Windows x86_64/aarch64. Microsoft
publishes no macOS x86_64 build for 1.24, so no binary is bundled there.

When no bundled binary exists, `ensure_ort_dylib_path()` falls back to a
**system/Homebrew install** — it probes `/opt/homebrew/lib`, `/usr/local/lib`,
Linuxbrew, and the conventional system lib dirs for `libonnxruntime.{dylib,so}`.
So an Intel-Mac user who runs `brew install onnxruntime` (currently 1.27, which is
ABI forward-compatible — `GetApi(N)` succeeds on any runtime ≥ N) gets real local
embeddings, same nomic model and 768-dim, with no re-indexing. This library is
**unpinned** (we don't control its version), so the bundled path remains the
version-controlled default; the system fallback is best-effort. `ort` would also
discover such a library on its own via the dynamic-loader search path, but probing
explicitly lets `local_embeddings_available()` report it so the UI picks `local`
rather than steering to the cloud.

**Cloud embedding backend.** The `embedding_backend` setting selects `local`
(fastembed) or `openai` (any OpenAI-compatible `/embeddings` endpoint, configured
via `embedding_model` / `embedding_api_key` / `embedding_base_url`). The cloud
provider requests `dimensions: 768` (OpenAI v3 Matryoshka) so its output matches
the `MTREE DIMENSION 768` indexes with **no schema migration** — switching
backends only requires re-indexing, handled by the existing `embed_model`
mismatch detection + `reindex_all_sources`. When `embedding_backend` is unset the
default is `local` where ONNX Runtime is bundled and `openai` where it is not, so
Intel Macs (and anyone who would rather not download the local model) steer to the
cloud automatically. `OpenAiEmbeddingProvider` is symmetric — no nomic
document/query prefixes — and is hot-swappable via `reconfigure_embedding_provider`.

---

## ADR-004: LLM Abstraction — Unified Provider Interface

**Status:** Proposed

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, messages: Vec<ChatMessage>, opts: CompletionOptions)
        -> Result<LlmResponse>;
    async fn stream(&self, messages: Vec<ChatMessage>, opts: CompletionOptions)
        -> Result<impl Stream<Item = Result<String>>>;
    fn context_window(&self) -> usize;
    fn model_name(&self) -> &str;
}
```

| Implementation      | Notes                                                                                                                                                                                         |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `OpenAIProvider`    | OpenAI API, configurable `base_url` → also covers Azure OpenAI, OpenRouter                                                                                                                    |
| `AnthropicProvider` | Anthropic Messages API via `reqwest`                                                                                                                                                          |
| `OllamaProvider`    | Native Ollama client speaking Ollama's `/api/chat` wire format (NDJSON streaming with `done: true` sentinel, `keep_alive`/`low_vram`/`num_ctx` params — not an OpenAI-compatible passthrough) |

The provider is resolved at startup from config and injected as `Arc<dyn LlmProvider>` into the service layer. In tests, replaced with `MockLlmProvider` (via `mockall`).

**Note:** A previous version of this ADR listed `OllamaProvider` as reusing `OpenAIProvider` with a `base_url` swap. This is incorrect — Ollama's API differs from OpenAI's in streaming format (NDJSON vs SSE), response schemas (`done`/`done_reason` fields), and provider-specific parameters. Each provider has its own wire-format client.

---

## ADR-005: Architecture — Tauri IPC + SurrealDB Embedded

**Status:** Proposed

### Context

The GM wants to eventually run the backend on a server and interact via a mobile client. The architecture must support this without a rewrite. The risk is building a desktop app too coupled to the local process model to extract into a server.

### Decision

The Rust backend runs **in-process** with Tauri. The frontend communicates via:

- **Tauri IPC commands** (`#[tauri::command]`) for request/response operations (CRUD, settings, PDF upload). Each command delegates to the service layer — the handler is a thin adapter.
- **Tauri events** for streaming operations (agent responses, ingestion progress). The streaming source (e.g., an `LlmProvider::stream` response channel) emits events into the Tauri event loop, which delivers them directly to the WebView.

The service layer is organized around traits (`LlmProvider`, `VectorStore`, `BlobStore`) so that extracting into a cloud server later means replacing the IPC handlers with axum handlers — the service layer itself doesn't change.

```
Desktop (v1)                        Cloud (vN)
─────────────────────────           ─────────────────────────────────────
Tauri shell                         Tauri Mobile / static web app
  └─ Rust backend in-process          └─ axum HTTP server (extracted
  └─ IPC commands (CRUD)                  service layer + new handlers)
  └─ Tauri events (streaming)          └─ SurrealDB Cloud or alternative
  └─ SurrealDB embedded                  └─ JWT auth middleware
```

**Realised crate workspace (as of the monorepo restructure):**

The service layer is no longer coupled to the Tauri binary. The codebase is a Cargo workspace of standalone library crates under `crates/`:

| Crate                    | Contains                                                                                          | Runtime deps (lib)                         |
| ------------------------ | ------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| `chronacle-core`         | Dependency traits (`LlmProvider`, `VectorStore`, `BlobStore`, `EmbeddingProvider`) + DTOs/errors  | (leaf) `serde`, `async-trait`, `thiserror` |
| `chronacle-db`           | `schema/*.surql` + `run_migrations`                                                               | `surrealdb`                                |
| `chronacle-providers`    | `SurrealDbVector`, `LocalFileStore`, fastembed/OpenAI/Mock embedding, OpenAI/Anthropic/Ollama LLM | `chronacle-core`                           |
| `chronacle-ingestion`    | `pdf_extractor`, `chunker`, `ingestion_service`, `text_normalizer`                                | `chronacle-core`                           |
| `chronacle-extraction`   | `entity_service`, `wikilink`, `extraction_service`                                                | `chronacle-core`                           |
| `chronacle-retrieval`    | `agent_service` (RAG chat + citation)                                                             | `chronacle-core`                           |
| `chronacle-domain`       | `campaign_service`, `session_service`, `collection_service`, `custom_provider_service`            | `chronacle-core`, `chronacle-extraction`   |
| `apps/desktop/src-tauri` | IPC commands, `AppState`, `settings_service`                                                      | all `chronacle-*` crates + `tauri`         |

`chronacle-ingestion`, `chronacle-extraction`, and `chronacle-retrieval` depend only on `chronacle-core` traits — not on `chronacle-providers`. `chronacle-db` is a dev-dependency (migrations in tests) for all service crates; only the desktop app needs it at runtime. `chronacle-domain` lib-depends on `chronacle-extraction` because `session_service` uses entity/wikilink types. The `chronacle-ingestion` tests use `chronacle-providers` as a dev-dependency for `MockEmbeddingProvider`; `chronacle-extraction` and `chronacle-retrieval` define their own test mocks.

The database connection type is `Surreal<engine::any::Any>`: the same `run_migrations` and query code compiles against both the embedded RocksDB (`rocksdb://<path>`) and SurrealDB Cloud. `settings_service` intentionally stays in `apps/desktop/src-tauri` — no extracted crate depends on it at runtime.

**Remaining path from desktop to cloud:**

1. Add `apps/server/` as a new workspace member: an axum binary that wires the existing `chronacle-*` crates to HTTP route handlers. The IPC command handlers in `apps/desktop/src-tauri/src/commands/` are the mapping reference.
2. Swap the SurrealDB connection string: `rocksdb://<path>` → SurrealDB Cloud URL. Schema and queries are unchanged.
3. Add auth middleware (JWT) to the axum router.
4. Ship the Svelte frontend as a static web app, or wrap in Tauri Mobile.

### Core Traits

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(&self, table: &str, records: Vec<VectorRecord>) -> Result<()>;
    async fn search(&self, table: &str, query: Vec<f32>, filter: SearchFilter, top_k: usize)
        -> Result<Vec<SearchResult>>;
    async fn delete(&self, table: &str, ids: Vec<String>) -> Result<()>;
}
// Implementations: SurrealDbVector, QdrantCloud (future)

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn store(&self, path: &str, data: &[u8]) -> Result<()>;
    async fn retrieve(&self, path: &str) -> Result<Vec<u8>>;
    async fn delete(&self, path: &str) -> Result<()>;
}
// Implementations: LocalFileStore (Phase 1–3), S3Store (Phase 4)
```

### Storage Path

| Layer                       | Desktop                          | Cloud                            |
| --------------------------- | -------------------------------- | -------------------------------- |
| Relational + Vector + Graph | SurrealDB embedded (RocksDB)     | SurrealDB Cloud (same SurrealQL) |
| File storage (PDFs)         | Local filesystem via `BlobStore` | S3 / GCS via `BlobStore`         |
| Auth                        | None (single-user)               | JWT issued at login              |

### Tauri IPC API Surface

```
// Campaigns
#[tauri::command] create_campaign(name, system) -> Campaign
#[tauri::command] get_campaigns() -> Vec<Campaign>
#[tauri::command] get_campaign(id) -> Campaign
#[tauri::command] update_campaign(id, name, system) -> Campaign
#[tauri::command] delete_campaign(id) -> ()

// Sources (PDFs)
#[tauri::command] upload_source(campaign_id, file_path) -> Source
#[tauri::command] get_sources(campaign_id) -> Vec<Source>
#[tauri::command] delete_source(campaign_id, source_id) -> ()

// Entities
#[tauri::command] create_entity(campaign_id, entity_type, name, ...) -> Entity
#[tauri::command] get_entities(campaign_id) -> Vec<Entity>
// ... etc

// Sessions
// Settings

// Streaming — uses Tauri events, not commands:
//   app.on_event(|source| { … }) — agent response tokens
//   app.on_event(|source| { … }) — ingestion progress
```

### Connectivity & Offline

The app detects network status at startup and during operation:

- If using a cloud LLM provider (OpenAI/Anthropic) and the network is down, surface a clear error in the chat UI: "Network unavailable — switch to Ollama in Settings or retry."
- If using Ollama locally, no network dependency — works fully offline.
- SurrealDB embedded is always local — no network dependency for storage.
- PDF ingestion is always local — no network dependency.
- Feature to auto-detect network loss and suggest switching to a local provider is planned for Phase 2.

---

## ADR-006: Testing Strategy — TDD with Unit, Integration & E2E

**Status:** Proposed

### Philosophy

Tests are written **before or alongside** implementation (TDD). The trait-based design (ADR-004, ADR-005) is the enabler: every service boundary is mockable, so unit tests never hit the network or filesystem. Integration tests use real storage with isolated in-memory databases. E2E tests drive the full app via the Tauri IPC layer (backend E2E) or WebDriver (full UI).

### Rust — Unit Tests

Location: `#[cfg(test)]` modules inside each source file.

- Test pure logic: chunker, section detector, prompt builder, citation extractor, token counter.
- Mock all external dependencies using `mockall`:
  - `MockLlmProvider` — returns deterministic responses without hitting an API.
  - `MockVectorStore` — returns pre-set search results.
  - `MockBlobStore` — simulates file storage.

**Note:** Prefer in-memory real implementations over mockall where practical. An in-memory `HashMap`-backed `VectorStore` catches more real bugs than a mock and produces less brittle test setup. Use mocks only for external I/O that's genuinely costly to simulate (LLM API, filesystem).

- Assertion crate: `pretty_assertions` for readable diffs on complex types.

```toml
[dev-dependencies]
mockall = "0.12"
pretty_assertions = "1"
tokio = { version = "1", features = ["test-utils"] }
```

Example test structure:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[tokio::test]
    async fn chunker_preserves_page_numbers() { ... }

    #[tokio::test]
    async fn agent_cites_retrieved_chunks() {
        let mut mock_llm = MockLlmProvider::new();
        mock_llm.expect_complete()
            .returning(|msgs, _| Ok(fixture_response()));
        // ...
    }
}
```

### Rust — Integration Tests

Location: `tests/` directory (compiled as separate crates by Cargo).

- Test the full service stack against **real storage** in isolated environments:
  - SurrealDB: use the in-memory engine (`mem::Db`) — run schema setup per test, drop on completion.
  - fastembed: use a small stub model or skip embedding by injecting a `MockVectorStore`. At least one integration test should exercise real fastembed with a small model (e.g., `all-MiniLM-L6-v2`, ~80 MB) to catch dimension errors and API mismatches.
- Test the service layer directly (no HTTP layer): construct services with real SurrealDB in-memory, call methods, assert results.
- `cargo test --test '*'` runs all integration tests.

```rust
// Integration test pattern
async fn setup_test_db() -> Surreal<Any> {
    let db = Surreal::new::<mem::Db>().await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    // Run schema definitions
    db.query(include_str!("../migrations/001_schema.surql")).await.unwrap();
    db
}
```

```toml
[dev-dependencies]
surrealdb = { version = "2", features = ["kv-mem"] }
tempfile = "3"
```

### Frontend — Unit & Component Tests

Tool: **Vitest** + `@testing-library/svelte`.

- Unit tests: utility functions, API client wrappers, date formatters, citation parsers.
- Component tests: render a component with props, assert DOM output, simulate user events.
- Mock the backend API using MSW (Mock Service Worker) or by wrapping Tauri IPC calls in a test harness.
- **Coverage target:** ≥ 70% on components (not just utilities — the hard parts are rendering, streaming state, and error recovery).

```bash
pnpm -C apps/desktop test        # watch mode
pnpm -C apps/desktop test:run    # CI mode (no watch)
```

### E2E Tests — Full App

Tool: **Playwright** against the backend service layer (backend E2E) and optionally the full Tauri app via `tauri-driver` (UI E2E).

**Backend E2E (fast, no UI):**

- Construct the service layer with a real SurrealDB in-memory database.
- Drive via the service API directly (no IPC layer — test the service, not the glue).
- Cover: full PDF ingestion flow → query → citation returned.
- Run in CI on every PR.

**UI E2E via tauri-driver (slow, full stack):**

- Requires a built Tauri app; run via WebDriver protocol.
- Cover: happy paths only — load PDF, ask question, read response.
- Run on Linux only in CI on merge to main (expand matrix when platform-specific bugs emerge).

```bash
# Backend E2E (service-layer, no UI)
pnpm -C apps/desktop exec playwright test tests/e2e/backend/

# Full UI E2E (requires built app: pnpm -C apps/desktop exec tauri build --no-bundle)
pnpm -C apps/desktop run e2e:ui
```

### Test Data & Fixtures

- **PDF fixtures:** A diverse suite of small (2–3 page) PDFs covering the extraction edge cases the app will face:
  - `single-column-text.pdf` — clean, simple text extraction (happy path)
  - `multi-column.pdf` — two-column body text to validate reading-order handling
  - `tables.pdf` — proficiency tables, spell lists, equipment tables
  - `scanned.pdf` — image-only PDF to validate graceful failure or OCR notice
  - `stat-block.pdf` — monster/NPC stat block with mixed positioning
  - Free licensing: SRD excerpts, synthetic PDFs generated programmatically
- LLM responses: all LLM calls in tests go through `MockLlmProvider`; fixture responses stored as JSON in `tests/fixtures/llm/`.
- Database fixtures: SurrealQL seed files in `tests/fixtures/db/`.

### Coverage

- Rust: `cargo-llvm-cov` for line coverage. Setup in Phase 1 (one `Cargo.toml` addition + CI step). Target: **≥ 80% on the service layer** (chunker, retrieval, agent, prompt builder). No coverage target on glue code.
- Frontend: Vitest's built-in Istanbul coverage. Target: **≥ 70% on key components** (streaming chat UI, entity forms, citation display).
- Coverage reports generated in CI and uploaded as artifacts; not used as a hard gate (quality over metric-gaming).

**Note:** Coverage tooling (`cargo-llvm-cov`) is set up in Phase 1, not deferred. Targets are aspirational — measurement from day one prevents surprises later.

---

## ADR-007: Code Quality — Linting, Formatting & CI

**Status:** Proposed

### Rust

| Tool          | Purpose                             | Command                                                    |
| ------------- | ----------------------------------- | ---------------------------------------------------------- |
| `rustfmt`     | Canonical formatting                | `cargo fmt --check` (CI), `cargo fmt` (local)              |
| `clippy`      | Lints and idiom enforcement         | `cargo clippy --all-targets --all-features -- -D warnings` |
| `cargo audit` | Known vulnerability scan            | `cargo audit`                                              |
| `cargo deny`  | License compliance + duplicate deps | `cargo deny check`                                         |

`clippy` runs with `-D warnings` — any lint is a CI failure, no exceptions. Clippy allow-list exceptions must be documented inline with a comment explaining why.

### TypeScript / Svelte

| Tool                                                     | Purpose       | Command                          |
| -------------------------------------------------------- | ------------- | -------------------------------- |
| `prettier` + `prettier-plugin-svelte`                    | Formatting    | `prettier --check .` (CI)        |
| `eslint` + `@typescript-eslint` + `eslint-plugin-svelte` | Linting       | `eslint .`                       |
| `tsc --noEmit`                                           | Type checking | `pnpm -C apps/desktop typecheck` |

### Pre-commit Hooks

Use `lefthook` (fast, cross-language, single config file) rather than husky + cargo-husky:

```yaml
# lefthook.yml
pre-commit:
  parallel: true
  commands:
    rustfmt:
      glob: "*.rs"
      run: cargo fmt --check
    clippy:
      glob: "*.rs"
      run: cargo clippy --all-targets -- -D warnings
    prettier:
      glob: "*.{ts,svelte,json,css}"
      run: prettier --check {staged_files}
    eslint:
      glob: "*.{ts,svelte}"
      run: eslint {staged_files}
```

Hooks are enforced locally and mirrored in CI — local and CI must be identical.

### CI Pipeline (GitHub Actions)

```
On every PR:
  ├── rust-check
  │     ├── cargo fmt --all --check
  │     ├── cargo clippy --workspace --all-targets -- -D warnings
  │     ├── cargo audit
  │     └── cargo test --workspace (unit + integration)
  ├── frontend-check
  │     ├── prettier --check
  │     ├── eslint
  │     ├── pnpm -C apps/desktop typecheck
  │     └── pnpm -C apps/desktop test:run (Vitest)
  └── e2e-backend
        └── pnpm -C apps/desktop exec playwright test tests/e2e/backend/

On merge to main:
  ├── All of the above
  ├── e2e-ui (tauri-driver via pnpm -C apps/desktop run e2e:ui, matrix: ubuntu, windows, macos)
  ├── cargo-llvm-cov --workspace (coverage report artifact)
  └── build-artifacts (installers for all platforms)
```

All CI steps are defined as reusable composite actions so they're runnable locally via `act`.

---

## ADR-008: Markdown Vault Sync — Bidirectional Filesystem Sync

**Status:** Proposed

### Context

GMs want to reuse and edit their campaign notes in other tools, particularly Obsidian, with changes flowing in both directions — edits made in Obsidian appear in Chronacle and vice versa. Notes are stored as Markdown strings in SurrealDB (`entity.notes`, `session.notes`).

### Decision

Introduce a **`VaultSyncService`** that keeps a user-configured directory of `.md` files in bidirectional sync with SQLite. The user can point the sync target at any existing folder, including a live Obsidian vault.

**Vault directory layout:**

```
<vault_root>/
  <campaign-slug>/
    sessions/
      001-the-awakening.md
      002-shadows-of-the-keep.md
    entities/
      npc/
        seraphina-aldric.md
      location/
        the-iron-tower.md
      faction/
      creature/
      item/
      event/
      player_character/
      misc/
```

**File format — YAML frontmatter + Markdown body:**

```markdown
---
id: "ent_abc123"
name: "Seraphina Aldric"
type: "npc"
campaign: "Shadows of Valdris"
is_gm_only: false
created_at: "2026-05-28T14:00:00Z"
updated_at: "2026-05-28T18:32:00Z"
---

Seraphina is the half-elven archivist of the Iron Tower...
```

Session files include `session_number`, `title`, and `date_played` in frontmatter. The `id` field is the stable link between a file and its DB row; it must not be removed or changed by the user.

**Sync behaviour — Chronacle → vault (outbound):**

| Event                    | Action                                                                                 |
| ------------------------ | -------------------------------------------------------------------------------------- |
| Note saved in Chronacle  | Write / overwrite the `.md` file; suppress the inbound file-watch event for that write |
| Entity / session deleted | Delete the `.md` file                                                                  |
| Entity / session renamed | Delete old file, write new file with updated slug                                      |
| Campaign renamed         | Rename the campaign slug folder                                                        |
| Vault path configured    | Full reconcile pass: write any file missing or older than `updated_at`                 |

**Sync behaviour — vault → Chronacle (inbound):**

| Event                                                                            | Action                                                                                                                            |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `.md` file modified                                                              | Parse frontmatter; if `id` matches a known entity/session, update `notes` + `name` in SurrealDB and re-index in SurrealDB         |
| `.md` file created (no `id` in frontmatter, inside a known campaign/type folder) | Create new entity or session in SurrealDB; write back the assigned `id` into the frontmatter                                      |
| `.md` file deleted                                                               | Soft-delete: mark `vault_deleted = TRUE` on the record in SurrealDB; surface a "restore or confirm delete" prompt in Chronacle UI |
| `.md` file moved within vault                                                    | If destination folder maps to a different entity type, update `entity_type` in SurrealDB; slug is cosmetic only                   |
| `.md` file with unknown `id` or outside campaign folders                         | Ignored — not managed by Chronacle                                                                                                |

**Conflict resolution (both sides modified before sync could propagate):**

- Last-write-wins based on `updated_at` (SurrealDB) vs. file mtime.
- If the delta is under 5 seconds (simultaneous edit), Chronacle surfaces a conflict card in the UI showing both versions; the GM picks one or merges manually.
- The losing version is written to `<file>.conflict.<timestamp>.md` in the same folder so no content is ever silently discarded.

**`is_gm_only` notes:** Written to the vault by default (the vault is on the GM's own machine). The `vault_include_gm_only` setting (default `true`) lets the GM opt out when sharing their vault.

**Loop-prevention:** Outbound writes set an in-memory `pending_write` guard (keyed by file path + content hash) that causes the file-watcher to skip the next inbound event for that path, preventing write → watch → write cycles.

### Implementation

- `VaultSyncService` holds `vault_root: Option<PathBuf>` and a `notify::RecommendedWatcher` that watches the vault directory recursively.
- **Outbound:** `async fn on_note_saved(&self, event: NoteEvent) -> Result<()>` — called by entity/session service layers after every successful DB write.
- **Inbound:** the `notify` watcher emits events into a debounced channel (100 ms); a background task drains the channel, parses changed files, and calls the entity/session service to apply updates.
- Vault path is persisted in the `setting` table under `vault_sync_path`; set via a Tauri IPC command.
- A `vault_deleted` field is added to `entity` and `session` records to track soft-deletes from the vault.
- The service is tested by writing files to a temp directory and driving the watcher directly — no `FileStore` abstraction needed (mock at the `VaultSyncService` boundary instead).

### New migration required

```surql
-- Add vault_deleted field to entity and session tables
DEFINE FIELD vault_deleted ON entity TYPE bool DEFAULT false;
DEFINE FIELD vault_deleted ON session TYPE bool DEFAULT false;
```

### Consequences

- The `notify` crate is added to approved crates (see Crate & Tool Summary); it uses OS-native APIs (`inotify`, FSEvents, ReadDirectoryChangesW) with no polling overhead.
- `serde_yaml` is added for frontmatter parsing and serialisation.
- Inbound changes trigger SurrealDB re-indexing of the updated note, so vault-edited notes remain searchable without any manual action in Chronacle.
- Deletions from the vault are intentionally non-destructive (soft-delete + UI prompt) to prevent accidental data loss from a misplaced `rm`.
- **Implementation note:** File I/O uses `tokio::fs` directly, tested by writing to a temp directory. No `FileStore` abstraction is needed — the `VaultSyncService` trait itself provides the test boundary.

---

## System Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                     Tauri Desktop App                        │
│                                                              │
│  ┌───────────────────────────────────────────────────────┐   │
│  │              Frontend (TypeScript + Svelte)            │   │
│  │                                                       │   │
│  │  ┌──────────┐ ┌────────────┐ ┌──────────────────────┐ │   │
│  │  │  Chat UI │ │   Entity   │ │  PDF & Source Mgr    │ │   │
│  │  │(streamed)│ │  Manager   │ │                      │ │   │
│  │  └──────────┘ └────────────┘ └──────────────────────┘ │   │
│  │  ┌──────────────────────────────────────────────────┐  │   │
│  │  │        Campaign Switcher + Settings               │  │   │
│  │  └──────────────────────────────────────────────────┘  │   │
│  │                                                       │   │
│  │    All IPC: invoke("command_name", { args })           │   │
│  │    Events:  app.listen("event_name", callback)          │   │
│  └──────────────────────┬───────────────────────────────┘   │
│                         │ Tauri IPC commands + events        │
│  ┌──────────────────────▼───────────────────────────────┐   │
│  │              Rust Backend (in-process)                 │   │
│  │                                                       │   │
│  │  ┌──────────────────────────────────────────────────┐ │   │
│  │  │           #[tauri::command] handlers              │ │   │
│  │  │  (thin adapters → delegate to services)          │ │   │
│  │  └───────┬───────────────────────────────────┬──────┘ │   │
│  │          │                                   │        │   │
│  │  ┌───────▼────────┐            ┌─────────────▼──────┐ │   │
│  │  │   Agent Svc    │            │  Entity/Session    │ │   │
│  │  │  query→retrieve│            │  CRUD services     │ │   │
│  │  │  →LLM→stream   │            │                    │ │   │
│  │  └───────┬────────┘            └──────────┬─────────┘ │   │
│  │          │                                 │          │   │
│  │  ┌───────▼─────────────────────────────────▼──────┐   │   │
│  │  │              SurrealDB Embedded                  │   │   │
│  │  │  (RocksDB: relational + vector + graph)        │   │   │
│  │  │  Tables: campaign, source, chunk, entity,       │   │   │
│  │  │  session, setting, relates_to, message          │   │   │
│  │  │  Vector index: chunk.embedding (MTREE, COSINE) │   │   │
│  │  └────────────────────────────────────────────────┘   │   │
│  │                                                       │   │
│  │  ┌──────────────────────────────────────────────────┐ │   │
│  │  │             PDF Ingestion Pipeline                │ │   │
│  │  │  pdfium-render → chunk → fastembed →              │ │   │
│  │  │  SurrealDB chunk upsert (background task)         │ │   │
│  │  │  Error recovery: checkpoint per page batch,       │ │   │
│  │  │  resume on crash, index_status tracks state       │ │   │
│  │  └──────────────────────────────────────────────────┘ │   │
│  │                                                       │   │
│  │  ┌──────────────────────────────────────────────────┐ │   │
│  │  │         Vault Sync Service (ADR-008)              │ │   │
│  │  │  notify watcher ↔ SurrealDB via entity/session   │ │   │
│  │  │  services; bidirectional with conflict detect    │ │   │
│  │  └──────────────────────────────────────────────────┘ │   │
│  └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘

App data dir: ~/.local/share/chronacle/
  ├── surreal.db/                     (RocksDB data directory)
  ├── pdfs/{source_id}/               (stored PDF files)
  └── embeddings_cache/               (fastembed model)

Cloud deployment: add apps/server/ (axum) reusing chronacle-* crates → SurrealDB Cloud → container
Mobile:           same Svelte frontend → Tauri Mobile or static web app
```

---

## Data Model (SurrealDB)

SurrealDB unifies relational, vector, and graph storage. All data lives in a single embedded RocksDB store with SurrealQL schemas.

```surql
-- Core
DEFINE TABLE campaign SCHEMAFULL;
DEFINE FIELD name ON campaign TYPE string;
DEFINE FIELD system ON campaign TYPE string;
DEFINE FIELD created_at ON campaign TYPE datetime;
DEFINE FIELD updated_at ON campaign TYPE datetime;

-- PDF sources
DEFINE TABLE source SCHEMAFULL;
DEFINE FIELD campaign ON source TYPE record<campaign> | NULL;  -- NULL = global/shared
DEFINE FIELD filename ON source TYPE string;
DEFINE FIELD display_name ON source TYPE string;
DEFINE FIELD source_type ON source TYPE string
  ASSERT $value IN ['rules', 'lore', 'supplement'];
DEFINE FIELD page_count ON source TYPE int;
DEFINE FIELD indexed_at ON source TYPE datetime;
DEFINE FIELD index_status ON source TYPE string
  ASSERT $value IN ['pending', 'indexing', 'done', 'error'];
DEFINE FIELD embed_model ON source TYPE string;  -- model identifier; mismatch at startup triggers warning

-- Ingested text chunks (with vector index for RAG)
DEFINE TABLE chunk SCHEMAFULL;
DEFINE FIELD source ON chunk TYPE record<source>;
DEFINE FIELD campaign ON chunk TYPE record<campaign>;
DEFINE FIELD text ON chunk TYPE string;
DEFINE FIELD page_start ON chunk TYPE int;
DEFINE FIELD page_end ON chunk TYPE int;
DEFINE FIELD section_heading ON chunk TYPE string;
DEFINE FIELD source_type ON chunk TYPE string;
-- is_gm_only: Phase 3 (AI-detected passage-level flag) — not in the current schema
DEFINE FIELD embedding ON chunk TYPE array<float>;
DEFINE FIELD embed_model ON chunk TYPE string;

-- Vector index on chunk embeddings
DEFINE INDEX chunk_embedding_idx ON chunk FIELDS embedding
  MTREE DIMENSION 768 DIST COSINE;

-- Structured entities (NPCs, locations, factions, creatures, items, events, PCs, misc)
DEFINE TABLE entity SCHEMAFULL;
DEFINE FIELD campaign ON entity TYPE record<campaign>;
DEFINE FIELD entity_type ON entity TYPE string
  ASSERT $value IN ['npc', 'location', 'faction', 'creature', 'item', 'event', 'player_character', 'misc'];
DEFINE FIELD name ON entity TYPE string;
DEFINE FIELD summary ON entity TYPE string;
DEFINE FIELD notes ON entity TYPE string;          -- free-form markdown; indexed for RAG
DEFINE FIELD created_at ON entity TYPE datetime;
DEFINE FIELD updated_at ON entity TYPE datetime;

-- Event-specific temporal attributes (populated only when entity_type = 'event')
DEFINE FIELD date_start ON entity TYPE string;     -- in-world date, free-form: "15 Mirtul 1492 DR"
DEFINE FIELD date_end ON entity TYPE string;       -- NULL for point-in-time events
DEFINE FIELD is_ongoing ON entity TYPE bool DEFAULT false;
DEFINE FIELD sequence_index ON entity TYPE int;    -- lower = earlier; manual ordering
DEFINE FIELD era ON entity TYPE string;            -- e.g. "Before the Cataclysm"
DEFINE FIELD session ON entity TYPE record<session>;  -- session where this occurred
DEFINE FIELD duration_label ON entity TYPE string; -- e.g. "3 days", "an instant"

-- Player character fields (populated only when entity_type = 'player_character')
DEFINE FIELD player_name ON entity TYPE string;
DEFINE FIELD character_class ON entity TYPE string;
DEFINE FIELD character_level ON entity TYPE int;
DEFINE FIELD status ON entity TYPE string
  ASSERT $value IN ['active', 'retired', 'deceased', 'missing', 'on_hiatus'];

-- Sessions
DEFINE TABLE session SCHEMAFULL;
DEFINE FIELD campaign ON session TYPE record<campaign>;
DEFINE FIELD session_number ON session TYPE int;
DEFINE FIELD title ON session TYPE string;
DEFINE FIELD date_played ON session TYPE string;   -- ISO date of the real-world session
DEFINE FIELD notes ON session TYPE string;          -- free-form markdown
DEFINE FIELD created_at ON session TYPE datetime;

-- Chat messages (Phase 1 — persistent, non-searchable history)
DEFINE TABLE message SCHEMAFULL;
DEFINE FIELD campaign ON message TYPE record<campaign>;
DEFINE FIELD role ON message TYPE string ASSERT $value IN ['user', 'assistant', 'system'];
DEFINE FIELD content ON message TYPE string;
DEFINE FIELD citations ON message TYPE array<object>;  -- [{source_name, page, text_excerpt}]
DEFINE FIELD created_at ON message TYPE datetime;

-- Graph edges (entity-to-entity relationships)
DEFINE TABLE relates_to TYPE RELATION SCHEMAFULL FROM entity TO entity;
DEFINE FIELD rel_type ON relates_to TYPE string;    -- "allied_with", "located_in", "member_of", etc.
DEFINE FIELD notes ON relates_to TYPE string;

-- Key-value settings
DEFINE TABLE setting SCHEMAFULL;
DEFINE FIELD value ON setting TYPE string;
-- Keys: llm_provider, llm_model, llm_api_key (encrypted at rest),
--       llm_base_url, embedding_backend, active_campaign_id,
--       vault_sync_path, vault_include_gm_only
```

**Note on timestamps:** All datetime fields use SurrealDB's `datetime` type (RFC 3339 / ISO 8601). This avoids the ambiguity of Unix epoch integers (seconds vs milliseconds) and is forward-compatible with SurrealDB Cloud. The vault sync frontmatter uses the same format.

**Timeline queries the Agent can answer** using event fields:

- "What happened before the Cataclysm?" → `era = 'Before the Cataclysm'`
- "List events in order" → `ORDER BY sequence_index`
- "What happened in session 4?" → `session = $session_record_id`
- "Was event X before event Y?" → compare `sequence_index`

---

## RAG Pipeline

### PDF Ingestion

```
PDF file
  │
  ▼ pdfium-render
Extract text per page (preserves page numbers)
  │
  ▼ Section detector
Identify headings via font-size heuristics / regex
  │
  ▼ Chunker
Sliding window: ~400 tokens, 80-token overlap
Each chunk tagged: source, campaign, page_start, page_end,
                   section_heading, source_type, embed_model
                   (is_gm_only: Phase 3 — AI-detected, see GM-Secret Handling)
  │
  ▼ fastembed (nomic-embed-text-v1.5, 768-dim)
Batch embed (async, ~500/s on CPU)
  │
  ▼ SurrealDB chunk upsert (batch insert)
Index: chunk_embedding_idx (MTREE, COSINE)
  │
  ▼ Update source.index_status = 'done'
Streamed progress events → Tauri events → frontend progress bar
```

**Error recovery:** The ingestion pipeline checkpoints progress per page batch. If a crash occurs mid-ingestion, `source.index_status` stays at `'indexing'` and the pipeline can resume from the last checkpointed page on retry. Partial chunks from a failed run are cleaned up by comparing the persisted checkpoint against what's in the `chunk` table.

### Notes Indexing

Entity notes and session notes → embedded → searchable (see [Notes indexing in Phase 2](#phase-2--campaign--notes)). `is_gm_only` is not yet modelled — it is deferred to Phase 3 as an AI-detected passage-level flag (see [GM-Secret Handling](#gm-secret-handling)).

### Codex Compilation (ADR-009)

Compilation is manual and staleness-driven: a per-collection "Compile" action
turns stale entities (including pre-migration rows whose `codex_stale` is
unset) into grounded `codex_article` markdown with `[Source: "…", p.N]`
citations and `codex_sources` provenance, then re-embeds the entity over
name + summary + notes + article. Provenance retrieval is scoped per the
reference rules: a campaign-bound collection searches its owner campaign's
full subscription set; a regular collection searches only itself. Runs are
capped (`MAX_COMPILE_PER_RUN = 50`) and per-entity failures are logged and
skipped, never aborting the run. Progress streams over the `codex-progress`
event; `cancel_compile` aborts between entities.

### Query & Retrieval

```
User query
  │
  ▼ fastembed (same model — consistency is critical)
Query vector
  │
  ▼ SurrealDB vector search
SELECT *, vector::distance::knn() AS distance FROM chunk
  WHERE embedding <|20|> $query_vector
    AND (campaign = $active_campaign OR campaign IS NULL)
  ORDER BY distance ASC
  LIMIT 20
  │
  ▼ Top-8 chunks selected (ANN only — no cross-encoder until Phase 3)
Each carries: text, source name, page range, section heading
  │
  ▼ Context builder → LLM prompt
```

**Note:** Cross-encoder reranking is deferred. Top-k ANN is evaluated against a test set of 50 real TTRPG queries in Phase 1. If retrieval recall@5 is below 70%, cross-encoder is added in Phase 3. If above 85%, it ships as-is.

#### SurrealQL KNN pitfalls

The MTREE KNN operator has two non-obvious constraints. Both are caught only at runtime (no compile-time query validation — see ADR-002):

1. **KNN goes in `WHERE`, not `ORDER BY`.** `embedding <|K|> $vec` must live in the `WHERE` clause to activate the MTREE index; ordering is by the computed distance, selected as `vector::distance::knn() AS distance` and used as `ORDER BY distance ASC`. Writing `ORDER BY embedding <|K|> $vec` is rejected: _"Missing order idiom `embedding` in statement selection."_

2. **KNN does not compose with an `id IN (subquery)` filter.** AND-ing `embedding <|K|> $vec` with `id IN (SELECT VALUE out FROM …)` **silently returns zero rows**. A field comparison (`collection IN [...]`), a graph-traversal predicate (`<-in_collection<-collection CONTAINS type::thing('collection','id')`), or an explicit-id array (`id IN [type::thing(...)]`) all compose correctly. Collection-scoped entity retrieval in `agent_service::fetch_entity_context` uses the graph-traversal form for this reason; regression test: `fetch_entity_context_knn_over_collection_executes`.

### System Prompt

```
You are an expert Game Master assistant for the campaign "{campaign_name}" ({system}).

REFERENCE MATERIAL:
{formatted_chunks_with_citation_markers [1] [2] ...}

INSTRUCTIONS:
- Answer using ONLY information from the reference material above.
- Every factual claim must cite its source: [Source: <name>, p.<page>].
- If the answer is not in the sources, say so explicitly — do not speculate.
- [GM SECRET] tagged material is for the GM's eyes only; you may use it freely.
- Be concise. The GM is running a table.
```

---

## GM-Secret Handling

**Deferred to Phase 3** (a Phase 2 attempt was built then reverted — see below). In
Phase 1–2 everything is GM-visible (single-user app), so the flag has no functional
consumer yet; its real payoff is a player-safe view/export that actually strips secrets.

**Why the manual flag was the wrong model.** GM-secret content is rarely a whole book — it
is _passage-level_: the boxed "For the GM" sidebars, secret lore, adventure spoilers,
read-aloud vs behind-the-screen text. A manual per-source / per-entity boolean models a
granularity that mostly doesn't exist. A Phase 2 implementation (per-source/entity/session
toggles + chunk inheritance + a chat "GM only" badge) was built and then reverted for this
reason.

When implemented in Phase 3, alongside player-safe export:

- **AI-detected at index time.** Classify each chunk as GM-secret vs player-safe by
  inspecting its text, rather than asking the GM to flag whole sources. A cheap keyword
  prefilter (headings / first lines hitting "For the GM", "Secret", "Behind the Screen",
  "Development", spoiler-y cues) narrows the LLM pass to candidate chunks to bound cost.
  - _Known limitation:_ GM-secret cues are often **visual** (shaded boxes, sidebars, icons)
    and `pdfium` text extraction drops them, so detection leans on explicit textual labels;
    needs a small eval set to tune precision/recall (false negatives leak secrets).
- `is_gm_only` then lives on `chunk` (and entity/session notes) as a _derived_ flag.
- Retrieval never filters GM-secret chunks (single-user GM app); the flag drives a
  player-safe export/view that strips them and a "spoiler" indicator in chat.

---

## Multi-Campaign Support

> **Updated (Phase 2):** Source sharing is modeled via **collections**, not a global
> `campaign = NULL` scope. Sources attach to a collection; campaigns `subscribes_to`
> one or more collections (migration `003_collections.surql`). A "shared rulebook" is
> a collection multiple campaigns subscribe to. There is no global source scope.

- Sources belong to a **collection**; a campaign sees a source's chunks when it
  subscribes to that source's collection.
- Entities/sessions are campaign-scoped via the `in_campaign` edge (migration
  `006_collection_entities.surql`); entities may also be collection-scoped via
  `in_collection`.
- Retrieval searches: chunks from the active campaign's subscribed collections +
  the campaign's own entity/session note embeddings.
- Switching campaigns updates the active campaign pointer; queries use the new
  subscription set automatically.

---

## Development Phases

Testing is not a phase — it is part of every phase from day one. No feature ships without unit tests. Integration and E2E tests ship with the feature.

### Phase 1 — Core RAG (MVP)

**Status:** Complete (2026-06-03). Retrieval recall@5 = 100% on the Phase 1 query set; see [`docs/phase-1-retrieval-eval.md`](phase-1-retrieval-eval.md).

Goal: Load a PDF, ask a rules question, get a cited answer.

- [x] Tauri scaffold with IPC commands and event system
- [x] `LlmProvider` trait + `OpenAIProvider` + `AnthropicProvider` + `OllamaProvider`
- [x] `VectorStore` trait + `SurrealDbVector` implementation
- [x] `BlobStore` trait + `LocalFileStore` implementation
- [x] SurrealDB embedded setup (RocksDB) + schema via `.surql` migration files
- [x] Settings screen: LLM provider config
- [x] PDF ingestion pipeline with Tauri event progress streaming
- [x] **Ingestion error recovery:** on any failure, mark source `'error'` and delete orphan chunks so retry starts clean (true checkpoint/resume deferred — cleanup + retry is the simplest correct behavior)
- [x] fastembed integration (first-run model download with onboarding screen)
- [x] Chunker with section detection
- [x] Basic chat UI with streaming responses + citation rendering
- [x] **Chat history:** `message` table in SurrealDB, persist + display on page load
- [x] **Coverage tooling:** `cargo-llvm-cov` setup in CI from day one
- [x] **Embedding model identity check:** startup warning + re-index banner when `source.embed_model` differs from active provider (ADR-003)
- **Tests shipped with Phase 1:**
  - [x] Unit: chunker, section detector, prompt builder, citation parser
  - [x] Integration: full ingest → query cycle using diverse PDF fixture suite (`single-column`, `multi-column`, `tables`, `stat-block`, `scanned`) — see `tests/pdf_fixture_ingest.rs`
  - [x] Integration: real fastembed (Nomic) tests — see `tests/rag_quality_integration.rs` and `tests/retrieval_recall.rs`
  - [x] Backend E2E: service-layer test with real SurrealDB in-memory — ingest → query → assert citation
  - [x] CI: fmt, clippy, unit, integration, e2e-backend, cargo-llvm-cov

Milestone: "Ask the rulebook a question and get a cited answer." ✓

### Phase 2 — Campaign & Notes

**Status:** Complete. Entity/campaign/session CRUD, all 8 entity types, the notes editor,
notes indexing/retrieval (entity + session), keyboard-first g-chord navigation, and the
Phase 2 test suite (event timeline ordering, notes→retrieval, campaign→NPC+event backend
E2E) are all done; source scoping resolved as collection-based. **`is_gm_only` was pulled**
— a manual whole-source/entity flag is the wrong granularity (GM-secret content is
passage-level) and has no real consumer in a single-user app, so it moves to Phase 3 as
AI-detected passage-level secrecy + player-safe export. See
[`docs/superpowers/plans/2026-06-13-phase-2-finalization.md`](superpowers/plans/2026-06-13-phase-2-finalization.md).

Goal: Multi-campaign support, hybrid notes, lore retrieval.

- [x] Campaign CRUD — `campaign_service` + `CampaignView.svelte`; tested (`campaign_service_test.rs`, `CampaignView.test.ts`)
- [x] Entity manager (NPC, location, faction, creature, item, misc) — `entity_service` + `EntityManager.svelte` / `EntityForm.svelte`; all 8 typed node tables (migration `004_graph_entities.surql`)
- [x] `event` entity type + temporal fields UI — all fields (`date_start`/`date_end`/`is_ongoing`/`sequence_index`/`era`/`duration_label`/`session`) in form; _timeline visualisation moved to Phase 3_
- [x] `player_character` entity type with player name / class / status — form fields + status enum; tested
- [x] Entity notes editor (markdown) — `WikiLinkEditor.svelte` with `[[Entity]]` autocomplete + `WikiText` rendering
- [~] **`is_gm_only` — deferred to Phase 3.** Built then reverted (commits `6a1634b`/`01d63ac` reverted): a manual whole-source/entity boolean models a granularity that rarely exists — GM-secret material is passage-level (boxed "For the GM" sidebars, secret lore, spoilers) — and in a single-user app the flag only powers a cosmetic badge. The real payoff (player-safe view/export) is Phase 3, where it returns as **AI-detected passage-level** secrecy (classify chunks at index time) rather than a manual toggle. See Phase 3.
- [x] Notes indexing pipeline (entity + session notes → embed → SurrealDB) — `entity_service::embed_node` (name+summary+notes, single source of truth, called by manual create/update **and** extraction) + `session_service::embed_session` (migration `007_session_embedding.surql`); `agent_service::fetch_entity_context` now includes entity _and_ session note excerpts in the LLM context
- [x] Collection-scoped sources — sources attach to collections; campaigns `subscribes_to` collections (migration `003_collections.surql`). _Supersedes the original "global vs campaign-scoped (NULL)" design — there is no global source scope._
- [x] Keyboard-first shortcuts (GM is at the table) — Vim-style g-chords for navigation (`g o/p/n/l/f/c/i/e/s/m/,`), `c` new entity, `/` focus chat, `?` help overlay, Esc close; suppressed while typing (`lib/shortcuts.ts` + `Shell.svelte`); unit + Shell integration tests
- **Tests shipped with Phase 2:**
  - [x] Unit: entity CRUD service (`entity_service_test.rs`); event `sequence_index` timeline ordering (`order_events_for_timeline` unit + `get_events_timeline` integration)
  - [x] Integration: notes indexing → retrieval (entity/session note embedding + context inclusion tested in `entity_service`/`session_service`/`agent_service`)
  - [x] Backend E2E: create campaign → add NPC + event → query → assert both appear in response (`tests/e2e_campaign_notes_query.rs`)
  - [x] Component tests: entity form validation (`EntityForm.test.ts`)

Milestone: "Run a full session, take notes on NPCs and events, ask a lore question and get cited answers from both the sourcebook and your own notes."

### Phase 3 — Polish & Power Features

Goal: Production quality, power-user features.

- [x] Session log timeline view
- [x] Entity relationship graph (SurrealDB `relates_to` graph edges → visualisation)
- [ ] Source enable/disable toggle per query
- [ ] Searchable chat history (full-text search on `message.content`)
- [ ] Export: session summary → markdown / PDF
- [ ] Markdown vault sync (ADR-008): bidirectional `.md` sync with a user-configured folder (Obsidian-compatible); inbound file-watch via `notify`; conflict detection with `.conflict.<ts>.md` preservation; soft-delete on vault file removal; `vault_include_gm_only` toggle; startup reconcile pass
- [ ] **`/extract-all` rework:** replace the brute-force full-sweep extraction placeholder with a smarter, incremental approach (scope/cost-aware; avoid re-extracting unchanged sources).
- [ ] **Cross-encoder reranking:** only if Phase 1 retrieval recall@5 measured below 70%. If above 85%, skip.
- [ ] Campaign rename UI (update slug, vault folder name)

### Phase 4 — Cloud / Mobile

Goal: Deploy backend as a server; access from mobile.

- [ ] Add `apps/server/` workspace member: an axum binary reusing the existing `chronacle-*` crates; IPC handlers in `apps/desktop/src-tauri/src/commands/` are the mapping reference (service layer already extracted — see ADR-005)
- [ ] SurrealDB embedded → SurrealDB Cloud (different connection string, same SurrealQL queries)
- [ ] `S3Store` implementation of `BlobStore` trait for PDF storage
- [ ] Auth middleware (JWT) on the axum router
- [ ] Docker image for the axum server
- [ ] Svelte frontend deployed as static web app
- [ ] Tauri Mobile packaging (iOS / Android) — or progressive web app if Tauri Mobile is premature

---

## Key Technical Risks

| Risk                                                        | Mitigation                                                                                                                                                             |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PDF text extraction quality varies (multi-column, scanned)  | Use `pdfium-render`; add "preview extracted text" view so GMs can spot bad extractions before indexing                                                                 |
| fastembed first-run download (~250 MB) looks like a hang    | Dedicated onboarding screen: "Downloading AI model (one time)" with a real progress bar                                                                                |
| Embedding model locked in after indexing                    | Store model ID in `sources.embed_model`; detect mismatch at startup; offer re-index with a warning                                                                     |
| SurrealDB embedded RocksDB compile time                     | Expected on first build (~30–60 s for C++ transitive); mitigated by caching CI builds via `sccache` or GitHub Actions cache                                            |
| SurrealQL learning curve (no compile-time query validation) | Mitigated by comprehensive integration tests using SurrealDB in-memory engine; all queries exercised in test suite before hitting production                           |
| Embedding model locked in after indexing                    | Store model ID in `source.embed_model` and `chunk.embed_model`; detect mismatch at startup; offer re-index with a warning and provide a batch re-index workflow        |
| Context window overflow with many chunks                    | Count tokens before sending; surface a warning in the UI; let the GM limit sources per query                                                                           |
| LLM hallucinating rules despite strict prompt               | Track retrieval scores; show a low-confidence indicator when the top chunk similarity is below a threshold                                                             |
| Vector index performance at scale (>100K chunks)            | SurrealDB's MTREE index performs well at moderate scale; if needed, add a `QdrantCloud` implementation behind the `VectorStore` trait — no data model changes required |

---

## Crate & Tool Summary

### Internal Workspace Crates (`crates/`)

These are first-party library crates in the Cargo workspace. They are not external dependencies and do not require an ADR to modify; changes are governed by the crate boundary rules in ADR-005.

| Crate                  | Responsibility                                                                                                    |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `chronacle-core`       | Dependency traits (`LlmProvider`, `VectorStore`, `BlobStore`, `EmbeddingProvider`) + DTOs and error types         |
| `chronacle-db`         | SurrealQL schema (`.surql` files) + `run_migrations`                                                              |
| `chronacle-providers`  | Concrete impls: `SurrealDbVector`, `LocalFileStore`, fastembed/OpenAI/Mock embedding, OpenAI/Anthropic/Ollama LLM |
| `chronacle-ingestion`  | PDF extraction (`pdfium-render`), chunker, text normalizer, ingestion pipeline                                    |
| `chronacle-extraction` | Entity CRUD/relations, wikilink resolution, LLM-driven entity extraction                                          |
| `chronacle-retrieval`  | RAG agent service: retrieval, context assembly, cited-answer generation                                           |
| `chronacle-domain`     | Campaign, session, collection, and custom-provider CRUD services                                                  |

### Rust Crates

| Purpose                                               | Crate                                                        |
| ----------------------------------------------------- | ------------------------------------------------------------ |
| Desktop app framework                                 | `tauri` 2.x                                                  |
| Unified store (relational + vector + graph)           | `surrealdb` (embedded, `kv-rocksdb` feature)                 |
| PDF text extraction                                   | `pdfium-render`                                              |
| Local embeddings                                      | `fastembed` (ONNX Runtime via `ort-load-dynamic`)            |
| Native-lib fetch at build time (pdfium, ONNX Runtime) | `reqwest` (blocking) + `flate2` + `tar` + `zip` (build-deps) |
| OpenAI LLM                                            | `async-openai`                                               |
| HTTP client (Anthropic, Ollama)                       | `reqwest`                                                    |
| Async runtime                                         | `tokio`                                                      |
| Serialisation                                         | `serde` + `serde_json`                                       |
| Unique IDs                                            | `uuid`                                                       |
| Mocking in tests                                      | `mockall`                                                    |
| YAML frontmatter (vault sync)                         | `serde_yaml`                                                 |
| Filesystem watcher (vault sync)                       | `notify`                                                     |
| Coverage                                              | `cargo-llvm-cov`                                             |
| Audit                                                 | `cargo-audit`, `cargo-deny`                                  |

### Frontend / Tooling

| Purpose                | Tool                                                                    |
| ---------------------- | ----------------------------------------------------------------------- |
| Framework              | Svelte 5 + TypeScript                                                   |
| Build                  | Vite                                                                    |
| Unit / component tests | Vitest + `@testing-library/svelte`                                      |
| API mocking in tests   | `msw` (Mock Service Worker) — intercepts Tauri IPC calls in the WebView |
| E2E tests              | Playwright                                                              |
| BDD acceptance specs   | `playwright-bdd` + `@cucumber/cucumber` (Gherkin `.feature` files) — ADR-011 |
| Linting                | ESLint + `@typescript-eslint` + `eslint-plugin-svelte`                  |
| Formatting             | Prettier + `prettier-plugin-svelte`                                     |
| Pre-commit hooks       | lefthook                                                                |
| Graph layout           | `d3-force` (entity relationship graph, Phase 3)                         |

---

## ADR-009: Compiled World Model — The Codex

**Status:** Accepted (2026-07-03). Schema landed in PR-A2a; behaviour lands
across the A2b–C2 series. Full design:
`docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md`.

### Context

Chronacle answered every question by re-deriving from raw chunks plus thin
entity summaries: no durable compiled knowledge, no write-back of durable
results, no linting, and no compiled rules layer (the LLM Wiki gap).

### Decision

A compiled layer — the **Codex** — sits between extraction and answering:

- **Setting articles live on the entity tables** as machine-owned fields
  (`codex_article`, `codex_compiled_at`, `codex_stale`, `codex_sources`).
  User `summary`/`notes` are never machine-overwritten.
- **Rules are a separate aggregate**, `rule_entry`, collection-scoped, with
  a closed category enum (`mechanic`, `ability`, `state`, `procedure`,
  `resource`, `statistic`, `entry`), compiler-owned body/page-refs and a
  GM-owned `notes` field. Corrections go through "redo with objections".
- **Write-back is a review queue** (`codex_proposal`): chat answers and
  session notes propose changes; nothing mutates the compiled layer
  without explicit accept.
- **Compilation is manual with staleness markers** — never automatic.
- **Reference rules are enforced**: content of a campaign-bound collection
  may reference collections its owner campaign subscribes to; content of a
  regular collection may reference only that same collection. Enforced at
  relation write time, at compile-provenance time, and by lint pass.
- Retrieval consumes compiled layers first: RULES → CODEX → ENTITIES →
  CHUNKS block ordering (lands in PR-B3).
- Everything compiler-owned is derived state, recompilable from chunks +
  accepted proposals + stored objections — the layer's core safety
  property.

### Consequences

- Positive: durable, citable, incrementally-maintained knowledge; rules
  and setting stay separate in the domain model, retrieval, and UX.
- Negative: LLM compile cost (mitigated: manual trigger + staleness
  increments); entity embeddings change semantics once articles are
  folded in (accepted — strictly richer signal, same embed model).
- The `codex_service` lives in `chronacle-extraction` (same dependency
  shape as extraction); extracting a dedicated crate later is mechanical.

---

## ADR-010: Campaign-owned collections

**Status:** Accepted (PR-A1a).

Companion ADR-009 (Compiled World Model — The Codex), accepted in PR-A2a,
introduces the codex and rules aggregates that live *inside* an owned
collection. ADR-010 is scoped to the collection-ownership plumbing only.

### Context

Chronacle historically treats collections as flat, shareable groups of source
material — a collection can be subscribed to by any number of campaigns.
There is no place to store material that belongs *only* to one campaign
(session-derived NPCs, the party's home town, table-specific factions).
Users have worked around this by creating an ad-hoc "MyCampaign notes"
regular collection and remembering never to subscribe another campaign to
it.

The LLM Wiki layer (A2 onward) needs a durable, per-campaign home for
compiled wiki entries and rules. Rather than invent a new aggregate outside
the collection model, we mark specific collections as *owned* by a campaign.

### Decision

A collection gains one optional field, `owner_campaign: option<record<campaign>>`.

- `NONE` → **regular collection** — indistinguishable in behaviour from every
  pre-A1a collection, freely shareable across campaigns.
- `Some(campaign)` → **campaign-bound** — auto-created when a campaign is
  created, subscribed to that campaign, and cannot be transferred to another
  campaign. It can only be turned back into a regular collection by
  deleting its owner (see below).

Every new campaign now auto-creates exactly one owned collection with the
campaign's own name. Existing regular collections are unaffected.

### Campaign deletion

Deleting a campaign requires an explicit choice about its owned collection:

- **`OnOwnedCollection::Delete`** — cascade: delete the owned collection and
  every DB-side artefact inside it (sources, chunks, entities,
  `relates_to`/`in_collection`/`in_campaign` edges). Source blob files on
  disk are deliberately left in place — filesystem cleanup is the caller's
  responsibility (source-command layer).
- **`OnOwnedCollection::ConvertToRegular`** — keep the collection, clear its
  `owner_campaign` field, and delete every `relates_to` edge whose *both*
  endpoints are inside the collection. Each dropped edge is recorded as a
  `lint_finding` row with `kind = "orphaned_edge"` so nothing is silently
  lost.

Edges that cross into a regular collection (only one endpoint inside) are
preserved by convert — they now legitimately connect two shareable
collections. Regular collections the campaign was subscribed to are never
touched.

The Tauri command layer surfaces this as a **required** `on_owned_collection`
parameter on `delete_campaign` (made required in PR-A1b, 2026-07, together
with the two-mode confirmation dialog). Callers must pass `"delete"` or
`"convert_to_regular"`; omitting it is a command error.

### `lint_finding` table

A minimal, additive `lint_finding` table is introduced early to give
convert-to-regular somewhere to write its findings. In A1a exactly one
`kind` is produced (`orphaned_edge`); later PRs (C1 onward) extend the
schema of `kind`s and add UI to review and resolve findings. Payload shape
per kind is documented in `002_wiki_layer.surql`.

### Consequences

- **Positive.** Every campaign has a durable, private home for its own
  material — the same shape the wiki layer will land into in A2. The delete
  flow no longer forces a false choice between "lose the notes" and "leave
  a dangling campaign shell".
- **Negative.** The `delete_campaign` command surface grows a required
  parameter (once A1b lands), so any third-party automation calling that
  command directly must update. This is judged worthwhile against the
  alternative of silently defaulting to cascade, which would destroy user
  data.
- **Retrieval.** No retrieval changes in A1a. The wiki-layer retrieval
  integration (block ordering RULES → WIKI → ENTITIES → CHUNKS) is planned
  for PR-B3 after the aggregates land in A2.

### Migration

Additive only: `002_wiki_layer.surql` adds the field, its index, and the
`lint_finding` table with all `DEFINE ... OVERWRITE` so repeat-runs are
idempotent (matching the pattern established by `001_base_schema.surql`).
Pre-A1a databases pick up the field with `owner_campaign = NONE`
everywhere — no backfill needed.

---

## ADR-011: Executable BDD Acceptance Specs — Cucumber/Gherkin via playwright-bdd

**Status:** Accepted (2026-07-03). Tooling lands in the Codex series' PR A0.

### Context

Design specs under `docs/superpowers/specs/` express user-visible behaviour
as BDD scenarios (Given/When/Then), but those scenarios are prose: nothing
executes them, so they drift from the implementation the moment a PR merges.
The E2E layer (backend Playwright suite, ADR-006) tests real behaviour but is
organised around code paths, not the acceptance criteria the specs promised.

### Decision

BDD scenarios become **executable Cucumber specs** and are **mandatory for
all future feature development**:

- Scenarios live as Gherkin `.feature` files in
  `apps/desktop/tests/e2e/features/`, one file per feature area.
- Step definitions live in `apps/desktop/tests/e2e/backend/steps/`
  (TypeScript), reusing the backend service-layer E2E harness.
- **`playwright-bdd`** (with `@cucumber/cucumber` for Gherkin parsing)
  generates Playwright test files from the features at test time into a
  gitignored `.features-gen/` directory. The existing Playwright runner
  stays the **single** E2E runner — same config, reporters, and CI job.
- The backend E2E CI step runs the generated BDD specs alongside the
  existing Playwright tests on every PR.
- **Process rule:** every PR that adds or changes user-visible behaviour
  must add or update `.feature` scenarios in the same PR. Design specs
  author their acceptance criteria in Gherkin so scenarios transfer
  verbatim from spec to `.feature` file.

### Options Considered

| Option                                        | Verdict                                                                                     |
| --------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `playwright-bdd` + `@cucumber/cucumber`       | **Chosen** — real Gherkin, one test runner, existing CI job unchanged                        |
| Standalone `@cucumber/cucumber` runner        | Rejected — second test runner with its own config, reporting, and CI wiring to keep in sync |
| Prose-only BDD in specs (status quo)          | Rejected — not executable; scenarios drift from implementation silently                     |

### Consequences

- **Positive.** Specs become regression tests; acceptance criteria cannot
  silently rot. Reviewers check a PR against its `.feature` diff.
- **Negative.** Two new npm devDependencies in `apps/desktop`
  (`playwright-bdd`, `@cucumber/cucumber`) — recorded in the tooling table
  per the dependency policy. Step-definition maintenance is a real cost;
  mitigated by reusing the existing E2E harness helpers.
- **Scope.** UI-level (tauri-driver) E2E is unaffected; features bind to
  the backend service-layer suite. Rust unit/integration and Vitest layers
  are unaffected (ADR-006 unchanged).
