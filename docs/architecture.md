# TTRPG GM Agent — Architecture & Design

**Status:** Proposed  
**Date:** 2026-05-26  
**System:** System-agnostic (PDF-driven)  
**Target platforms:** Windows, Linux, macOS (cloud/mobile: later)

---

## Overview

A desktop application that lets a GM load their own TTRPG PDFs (rules, sourcebooks, lore), take structured notes, and query an AI agent for rules answers and lore lookups — with source citations on every response. The LLM backend is configurable at setup: cloud API or local via Ollama.

The backend is structured as a standalone HTTP service from day one, embedded inside the desktop app. This makes a future cloud or mobile deployment a deployment decision, not an architectural rewrite.

---

## ADR-001: Application Framework — Tauri 2 (Rust + TypeScript/Svelte)

**Status:** Proposed

### Context

The app must run on Windows, Linux, and macOS with no complicated setup. It needs a UI and a capable backend for PDF processing, embedding, and vector search.

### Options Considered

| Dimension | Tauri 2 (Rust backend) | Electron (Node backend) | Native per-platform |
|-----------|------------------------|-------------------------|---------------------|
| Binary size | ~8–15 MB | ~200 MB | Small but 3× codebase |
| Runtime required | None (WebView2 on Win, built-in elsewhere) | Bundled Node | None |
| Rust ecosystem fit | Native — PDF, embeddings, SQLite, LanceDB | Needs FFI or re-implementation | N/A |
| Setup for user | Double-click installer | Double-click installer | Double-click installer |
| Memory footprint | Low | ~300 MB base | Low |

**Decision:** Tauri 2 with a Rust backend and a TypeScript/Svelte frontend.

**Consequences:**
- Backend logic lives in Rust; frontend in TypeScript/Svelte.
- The frontend communicates with the backend via HTTP (`fetch`) — **not** Tauri IPC commands. See ADR-005.
- WebView2 must be present on Windows (auto-installed by the Tauri bootstrapper).

---

## ADR-002: Vector Store — LanceDB (Embedded)

**Status:** Proposed

### Options Considered

| Dimension | LanceDB | sqlite-vec | Qdrant (embedded) |
|-----------|---------|------------|-------------------|
| Rust support | Native (`lancedb` crate) | Via `rusqlite` | Via REST/gRPC |
| Metadata filtering | First-class (columnar) | Basic SQL | Full |
| Multi-collection | Yes (tables) | Separate virtual tables | Yes |
| Cloud offering | LanceDB Cloud | No | Qdrant Cloud |

**Decision:** LanceDB for vector storage; SQLite (via `sqlx`) for relational data. LanceDB's cloud offering provides a direct migration path when moving to cloud hosting (ADR-005).

---

## ADR-003: Embeddings — fastembed-rs (Bundled Local Model)

**Status:** Proposed

### Options Considered

| Option | Quality | Setup | Offline |
|--------|---------|-------|---------|
| OpenAI `text-embedding-3-small` | Excellent | API key | No |
| Ollama embedding model | Good | Requires Ollama | Yes |
| `fastembed-rs` (`nomic-embed-text-v1.5`) | Good (768-dim) | Downloads on first run | Yes |

**Decision:** `fastembed-rs` as the default. Optional override to use cloud embedding API in settings.

**Critical constraint:** The embedding model is baked into the vector index. Switching models requires full re-indexing. The settings screen must warn the user. Store the model identifier alongside each LanceDB table so a mismatch is detectable at startup.

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

| Implementation | Notes |
|----------------|-------|
| `OpenAIProvider` | OpenAI API, configurable `base_url` → also covers Azure OpenAI, OpenRouter |
| `AnthropicProvider` | Anthropic Messages API via `reqwest` |
| `OllamaProvider` | Reuses `OpenAIProvider` with `base_url = http://localhost:11434` |

The provider is resolved at startup from config and injected as `Arc<dyn LlmProvider>` into the service layer. In tests, replaced with `MockLlmProvider` (via `mockall`).

---

## ADR-005: Cloud-Readiness — Axum Sidecar Architecture

**Status:** Proposed

### Context

The GM wants to eventually run the backend on a server and interact via a mobile client. The architecture must support this without a rewrite. The risk is building a desktop app that is too coupled to the local process model (Tauri IPC, filesystem paths baked into handlers) to extract into a server.

### Decision

The Rust backend is structured as a standalone **`axum` HTTP server** from the start. In desktop mode, Tauri spawns it on a random localhost port and injects that port into the WebView. The Svelte frontend communicates with the backend using standard `fetch()` calls — it has no awareness of whether it's talking to localhost or a remote server.

```
Desktop (v1)                        Cloud (vN)
─────────────────────────           ─────────────────────────────────────
Tauri shell                         Mobile web app / Tauri Mobile
  └─ spawns axum server             Browser or native WebView
       on localhost:PORT    →           └─ points to https://api.example.com
  └─ injects PORT into
       WebView env
  └─ Svelte app calls
       http://localhost:PORT
```

**Migration steps from desktop to cloud (no rewrite required):**
1. Extract the axum binary into its own Docker container.
2. Swap storage: SQLite → PostgreSQL (sqlx feature flag), LanceDB embedded → LanceDB Cloud or Qdrant Cloud (behind the `VectorStore` trait).
3. Add auth middleware to the axum router (JWT / session cookies).
4. Ship the Svelte frontend as a static web app; or wrap in Tauri Mobile.

### VectorStore Trait (for cloud swap)

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn upsert(&self, table: &str, records: Vec<VectorRecord>) -> Result<()>;
    async fn search(&self, table: &str, query: Vec<f32>, filter: SearchFilter, top_k: usize)
        -> Result<Vec<SearchResult>>;
    async fn delete(&self, table: &str, ids: Vec<String>) -> Result<()>;
}
// Implementations: LanceDbLocal, LanceDbCloud, QdrantCloud
```

### Storage Migration Path

| Layer | Desktop | Cloud |
|-------|---------|-------|
| Relational | SQLite | PostgreSQL (same `sqlx` queries — keep SQL ANSI-compatible, no SQLite-isms) |
| Vector | LanceDB embedded | LanceDB Cloud or Qdrant Cloud |
| File storage (PDFs) | Local filesystem | S3 / GCS (abstract behind a `BlobStore` trait) |
| Auth | None (single-user) | JWT issued at login |

### axum API Surface

```
REST
  POST   /api/campaigns
  GET    /api/campaigns
  GET    /api/campaigns/:id
  PUT    /api/campaigns/:id
  DELETE /api/campaigns/:id

  POST   /api/campaigns/:id/sources          (upload PDF → triggers ingestion)
  GET    /api/campaigns/:id/sources
  DELETE /api/campaigns/:id/sources/:sid

  POST   /api/campaigns/:id/entities
  GET    /api/campaigns/:id/entities
  PUT    /api/campaigns/:id/entities/:eid
  DELETE /api/campaigns/:id/entities/:eid

  POST   /api/campaigns/:id/sessions
  GET    /api/campaigns/:id/sessions
  PUT    /api/campaigns/:id/sessions/:sid

  GET    /api/settings
  PUT    /api/settings

WebSocket
  WS     /ws/campaigns/:id/chat              (streaming agent responses + citations)
  WS     /ws/campaigns/:id/ingest/:source_id (streaming ingestion progress)
```

---

## ADR-006: Testing Strategy — TDD with Unit, Integration & E2E

**Status:** Proposed

### Philosophy

Tests are written **before or alongside** implementation (TDD). The trait-based design (ADR-004, ADR-005) is the enabler: every service boundary is mockable, so unit tests never hit the network or filesystem. Integration tests use real storage with isolated temp directories / in-memory databases. E2E tests drive the full app via its HTTP API and the browser.

### Rust — Unit Tests

Location: `#[cfg(test)]` modules inside each source file.

- Test pure logic: chunker, section detector, prompt builder, citation extractor, token counter.
- Mock all external dependencies using `mockall`:
  - `MockLlmProvider` — returns deterministic responses without hitting an API.
  - `MockVectorStore` — returns pre-set search results.
  - `MockBlobStore` — simulates file storage.
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
  - SQLite: use `sqlx::test` macro — creates a fresh in-memory DB per test, runs migrations automatically.
  - LanceDB: create a `tempdir` per test, drop on cleanup.
  - fastembed: use a small stub model or skip embedding in integration tests by injecting a `MockVectorStore`.
- Test the axum router using `axum-test` (no real port binding needed):
  ```rust
  let app = build_router(services);
  let client = TestClient::new(app);
  let res = client.post("/api/campaigns").json(&payload).await;
  assert_eq!(res.status(), StatusCode::CREATED);
  ```
- `cargo test --test '*'` runs all integration tests.

```toml
[dev-dependencies]
axum-test = "15"
sqlx = { version = "0.7", features = ["sqlite", "test"] }
tempfile = "3"
```

### Frontend — Unit & Component Tests

Tool: **Vitest** + `@testing-library/svelte`.

- Unit tests: utility functions, API client wrappers, date formatters, citation parsers.
- Component tests: render a component with props, assert DOM output, simulate user events.
- Mock the backend API using `msw` (Mock Service Worker) — intercepts `fetch()` calls, returns fixtures.

```bash
pnpm test            # watch mode
pnpm test --run      # CI mode (no watch)
```

### E2E Tests — Full App

Tool: **Playwright** against the running axum server directly (backend E2E) and optionally the full Tauri app via `tauri-driver` (UI E2E).

**Backend E2E (fast, no UI):**
- Spin up the axum server with a test database.
- Drive via HTTP and WebSocket using Playwright's `request` API.
- Cover: full PDF ingestion flow → query → citation returned.
- Run in CI on every PR (moderate cost).

**UI E2E via tauri-driver (slow, full stack):**
- Requires a built Tauri app; run via WebDriver protocol.
- Cover: happy paths only — load PDF, ask question, read response.
- Run in CI on merge to main only (expensive).

```bash
# Backend E2E
pnpm playwright test tests/e2e/backend/

# Full UI E2E (requires built app)
pnpm playwright test tests/e2e/ui/
```

### Test Data & Fixtures

- PDF fixtures: use a small, freely licensed PDF (e.g., an SRD excerpt or a synthetic test PDF) committed to the repo under `tests/fixtures/`.
- LLM responses: all LLM calls in tests go through `MockLlmProvider`; fixture responses stored as JSON in `tests/fixtures/llm/`.
- Database fixtures: SQL seed files in `tests/fixtures/db/`.

### Coverage

- Rust: `cargo-llvm-cov` for line coverage. Target: **≥ 80% on the service layer** (chunker, retrieval, agent, prompt builder). No coverage target on glue code.
- Frontend: Vitest's built-in Istanbul coverage. Target: **≥ 70% on utility modules**.
- Coverage reports generated in CI and uploaded as artifacts; not used as a hard gate (quality over metric-gaming).

---

## ADR-007: Code Quality — Linting, Formatting & CI

**Status:** Proposed

### Rust

| Tool | Purpose | Command |
|------|---------|---------|
| `rustfmt` | Canonical formatting | `cargo fmt --check` (CI), `cargo fmt` (local) |
| `clippy` | Lints and idiom enforcement | `cargo clippy --all-targets --all-features -- -D warnings` |
| `cargo audit` | Known vulnerability scan | `cargo audit` |
| `cargo deny` | License compliance + duplicate deps | `cargo deny check` |

`clippy` runs with `-D warnings` — any lint is a CI failure, no exceptions. Clippy allow-list exceptions must be documented inline with a comment explaining why.

### TypeScript / Svelte

| Tool | Purpose | Command |
|------|---------|---------|
| `prettier` + `prettier-plugin-svelte` | Formatting | `prettier --check .` (CI) |
| `eslint` + `@typescript-eslint` + `eslint-plugin-svelte` | Linting | `eslint .` |
| `tsc --noEmit` | Type checking | `pnpm typecheck` |

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
  │     ├── cargo fmt --check
  │     ├── cargo clippy -- -D warnings
  │     ├── cargo audit
  │     └── cargo test (unit + integration)
  ├── frontend-check
  │     ├── prettier --check
  │     ├── eslint
  │     ├── tsc --noEmit
  │     └── pnpm test --run (Vitest)
  └── e2e-backend
        └── pnpm playwright test tests/e2e/backend/

On merge to main:
  ├── All of the above
  ├── e2e-ui (tauri-driver, matrix: ubuntu, windows, macos)
  ├── cargo-llvm-cov (coverage report artifact)
  └── build-artifacts (installers for all platforms)
```

All CI steps are defined as reusable composite actions so they're runnable locally via `act`.

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
│  │    All API calls: fetch("http://localhost:{PORT}/...")  │   │
│  └──────────────────────────┬────────────────────────────┘   │
│                             │ HTTP + WebSocket                │
│  ┌──────────────────────────▼────────────────────────────┐   │
│  │                  axum HTTP Server                      │   │
│  │              (spawned by Tauri on startup)             │   │
│  │                                                       │   │
│  │  ┌──────────────────────────────────────────────────┐ │   │
│  │  │                 Agent Service                    │ │   │
│  │  │  query → retrieve → build context →              │ │   │
│  │  │  LLM call → stream response + citations          │ │   │
│  │  └───────┬───────────────────────────────────┬──────┘ │   │
│  │          │                                   │        │   │
│  │  ┌───────▼────────┐            ┌─────────────▼──────┐ │   │
│  │  │  Retrieval Svc │            │   LLM Provider     │ │   │
│  │  │ (VectorStore   │            │ (Arc<dyn LlmProv>) │ │   │
│  │  │  trait)        │            └────────────────────┘ │   │
│  │  └───────┬────────┘                                   │   │
│  │          │                                            │   │
│  │  ┌───────▼────────┐  ┌────────────────────────────┐  │   │
│  │  │  LanceDB        │  │  SQLite / PostgreSQL        │  │   │
│  │  │  (VectorStore   │  │  (sqlx — same queries)     │  │   │
│  │  │   trait impl)   │  │                            │  │   │
│  │  └────────────────┘  └────────────────────────────┘  │   │
│  │                                                       │   │
│  │  ┌──────────────────────────────────────────────────┐ │   │
│  │  │             PDF Ingestion Pipeline                │ │   │
│  │  │  pdfium-render → chunk → fastembed →              │ │   │
│  │  │  VectorStore::upsert (background tokio task)      │ │   │
│  │  └──────────────────────────────────────────────────┘ │   │
│  └───────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘

App data dir: ~/.local/share/ttrpg-gm-agent/
  ├── db.sqlite                      (→ postgres:// in cloud)
  ├── lancedb/{campaign_id}/         (→ LanceDB Cloud in cloud)
  │   ├── chunks/
  │   └── notes/
  ├── pdfs/{source_id}/              (→ S3/GCS in cloud)
  └── embeddings_cache/              (fastembed model)

Cloud deployment: extract axum binary → container → point frontend to HTTPS URL
Mobile:           same Svelte frontend → Tauri Mobile or static web app
```

---

## Data Model (SQLite / PostgreSQL)

```sql
-- Core
CREATE TABLE campaigns (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    system      TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

-- PDFs
CREATE TABLE sources (
    id           TEXT PRIMARY KEY,
    campaign_id  TEXT REFERENCES campaigns(id),  -- NULL = global/shared across campaigns
    filename     TEXT NOT NULL,
    display_name TEXT NOT NULL,
    source_type  TEXT NOT NULL CHECK(source_type IN ('rules', 'lore', 'supplement')),
    page_count   INTEGER,
    indexed_at   INTEGER,
    index_status TEXT NOT NULL DEFAULT 'pending'
        CHECK(index_status IN ('pending', 'indexing', 'done', 'error')),
    embed_model  TEXT           -- model identifier used at index time; mismatch triggers warning
);

-- Structured entities
CREATE TABLE entities (
    id           TEXT PRIMARY KEY,
    campaign_id  TEXT NOT NULL REFERENCES campaigns(id),
    entity_type  TEXT NOT NULL CHECK(entity_type IN (
                     'npc', 'location', 'faction', 'creature',
                     'item', 'event', 'player_character', 'misc')),
    name         TEXT NOT NULL,
    summary      TEXT,
    notes        TEXT,          -- free-form markdown; indexed into LanceDB
    is_gm_only   BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);

-- Event-specific temporal attributes (populated only when entity_type = 'event')
-- In-world dates are free-form strings — the GM knows their calendar.
CREATE TABLE event_details (
    entity_id           TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
    -- Temporal positioning
    date_start          TEXT,       -- in-world date, free-form: "15 Mirtul 1492 DR"
    date_end            TEXT,       -- NULL for point-in-time events
    is_ongoing          BOOLEAN NOT NULL DEFAULT FALSE,
    -- Relative ordering (for when exact in-world dates are unknown/fuzzy)
    sequence_index      INTEGER,    -- lower = earlier; manually assigned by GM
    -- Grouping
    era                 TEXT,       -- e.g. "Before the Cataclysm", "Year of the Dragon"
    -- Real-world anchoring
    session_id          TEXT REFERENCES sessions(id),  -- session where this occurred/was revealed
    -- Duration character
    duration_label      TEXT        -- e.g. "3 days", "an instant", "decades"
);
-- Notes: sequence_index enables relative timeline ordering independent of any calendar system.
-- era enables grouping events across multiple calendars or fuzzy time periods.
-- date_start/date_end are opaque strings — no date parsing — the Agent uses them verbatim.

-- Player characters
CREATE TABLE player_character_details (
    entity_id        TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
    player_name      TEXT NOT NULL,   -- real name of the player
    character_class  TEXT,            -- free-form: "Paladin / Warlock", "Fighter 5 / Rogue 3"
    character_level  INTEGER,
    status           TEXT NOT NULL DEFAULT 'active'
        CHECK(status IN ('active', 'retired', 'deceased', 'missing', 'on_hiatus'))
    -- All other PC details go in entities.notes (backstory, inventory, relationships, etc.)
    -- Mark entities.is_gm_only = TRUE for backstory secrets the player hasn't revealed
);

-- Sessions
CREATE TABLE sessions (
    id             TEXT PRIMARY KEY,
    campaign_id    TEXT NOT NULL REFERENCES campaigns(id),
    session_number INTEGER,
    title          TEXT,
    date_played    TEXT,          -- ISO date of the real-world session
    notes          TEXT,          -- free-form markdown
    is_gm_only     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at     INTEGER NOT NULL
);

-- Entity relationships
CREATE TABLE entity_links (
    from_id   TEXT REFERENCES entities(id) ON DELETE CASCADE,
    to_id     TEXT REFERENCES entities(id) ON DELETE CASCADE,
    rel_type  TEXT NOT NULL,   -- "allied_with", "located_in", "member_of", "caused_by", etc.
    notes     TEXT,
    PRIMARY KEY (from_id, to_id, rel_type)
);

-- Settings
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
    -- Keys: llm_provider, llm_model, llm_api_key (encrypted at rest),
    --       llm_base_url, embedding_backend, active_campaign_id
);
```

**Timeline queries the Agent can answer** using event_details:
- "What happened before the Cataclysm?" → `WHERE era = 'Before the Cataclysm'`
- "List events in order" → `ORDER BY sequence_index`
- "What happened in session 4?" → join `event_details.session_id` → sessions
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
Each chunk tagged: source_id, page_start, page_end,
                   section_heading, campaign_id, source_type, is_gm_only
  │
  ▼ fastembed (nomic-embed-text-v1.5, 768-dim)
Batch embed (async, ~500/s on CPU)
  │
  ▼ VectorStore::upsert
Table: {campaign_id}/chunks
Streamed progress events → WebSocket → frontend progress bar
```

### Notes Indexing

Entity notes and session notes → chunked → embedded → `{campaign_id}/notes` table. Re-indexed on save (debounced 2 s). PC backstory notes marked `is_gm_only` are tagged in LanceDB accordingly.

### Query & Retrieval

```
User query
  │
  ▼ fastembed (same model — consistency is critical)
Query vector
  │
  ▼ VectorStore::search
Filter: campaign_id ∈ {active_campaign, global}
        (is_gm_only unfiltered — GM sees all)
Top-20 candidates from chunks + notes tables
  │
  ▼ (v2) Cross-encoder rerank
  │
  ▼ Top-8 chunks selected
Each carries: text, source name, page range, section heading
  │
  ▼ Context builder → LLM prompt
```

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

- `is_gm_only` flag on: entities, sessions, sources, player_character backstory notes.
- LanceDB chunks from GM-secret sources inherit `is_gm_only = TRUE` at index time.
- Retrieval never filters out GM-secret chunks (single-user GM app).
- Responses that drew from GM-secret chunks are visually flagged in the UI (shield icon / distinct border).
- The LLM is instructed to mark GM-secret-derived content so future player-safe export can strip it.

---

## Multi-Campaign Support

- LanceDB is partitioned by `campaign_id` (separate tables per campaign).
- Sources: `campaign_id = NULL` → global (rules PDFs reused across campaigns); otherwise campaign-scoped.
- Retrieval searches: `global chunks + campaign chunks + campaign notes`.
- Switching campaigns is instant (in-memory pointer swap + UI update).

---

## Development Phases

Testing is not a phase — it is part of every phase from day one. No feature ships without unit tests. Integration and E2E tests ship with the feature.

### Phase 1 — Core RAG (MVP)

Goal: Load a PDF, ask a rules question, get a cited answer.

- [ ] Tauri + axum scaffold (random port, injected into WebView)
- [ ] `LlmProvider` trait + `OpenAIProvider` + `AnthropicProvider` + `OllamaProvider`
- [ ] `VectorStore` trait + `LanceDbLocal` implementation
- [ ] Settings screen: LLM provider config
- [ ] SQLite schema + sqlx migrations
- [ ] PDF ingestion pipeline with WebSocket progress streaming
- [ ] fastembed integration (first-run model download with onboarding screen)
- [ ] Chunker with section detection
- [ ] Basic chat UI with streaming responses + citation rendering
- **Tests shipped with Phase 1:**
  - Unit: chunker, section detector, prompt builder, citation parser
  - Integration: full ingest → query cycle using fixture PDF + MockLlmProvider
  - E2E backend: POST source → wait for index → POST query → assert citation in response
  - CI: fmt, clippy, unit, integration, e2e-backend

Milestone: "Ask the rulebook a question and get a cited answer."

### Phase 2 — Campaign & Notes

Goal: Multi-campaign support, hybrid notes, lore retrieval.

- [ ] Campaign CRUD
- [ ] Entity manager (NPC, location, faction, creature, item, misc)
- [ ] `event` entity type + temporal fields UI (timeline view)
- [ ] `player_character` entity type with player name / class / status
- [ ] Entity notes editor (markdown, `is_gm_only` toggle)
- [ ] Notes indexing pipeline
- [ ] Global vs campaign-scoped sources
- [ ] GM-secret visual indicators in chat
- **Tests shipped with Phase 2:**
  - Unit: event ordering logic, entity CRUD service
  - Integration: notes indexing → retrieval, is_gm_only propagation into LanceDB
  - E2E backend: create campaign → add NPC + event → query → assert both appear in response
  - Component tests: entity form validation, GM-secret toggle

Milestone: "Run a full session, take notes on NPCs and events, ask a lore question and get cited answers from both the sourcebook and your own notes."

### Phase 3 — Polish & Power Features

Goal: Production quality, power-user features.

- [ ] Session log timeline view
- [ ] Entity relationship graph (visualisation)
- [ ] Source enable/disable toggle per query
- [ ] Persistent chat history (searchable per campaign)
- [ ] Export: session summary → markdown / PDF
- [ ] Cross-encoder reranking (Phase 1 uses ANN only)
- [ ] Keyboard-first shortcuts (GM is at the table)
- [ ] `cargo-llvm-cov` coverage reporting in CI

### Phase 4 — Cloud / Mobile

Goal: Deploy backend as a server; access from mobile.

- [ ] sqlx feature flag: `sqlite` → `postgres`; test suite runs against both
- [ ] `LanceDbCloud` or `QdrantCloud` implementation of `VectorStore` trait
- [ ] `BlobStore` trait + S3 implementation for PDF storage
- [ ] Auth middleware (JWT) on the axum router
- [ ] Docker image for the axum server
- [ ] Svelte frontend deployed as static web app
- [ ] Tauri Mobile packaging (iOS / Android) — or progressive web app if Tauri Mobile is premature

---

## Key Technical Risks

| Risk | Mitigation |
|------|------------|
| PDF text extraction quality varies (multi-column, scanned) | Use `pdfium-render`; add "preview extracted text" view so GMs can spot bad extractions before indexing |
| fastembed first-run download (~250 MB) looks like a hang | Dedicated onboarding screen: "Downloading AI model (one time)" with a real progress bar |
| Embedding model locked in after indexing | Store model ID in `sources.embed_model`; detect mismatch at startup; offer re-index with a warning |
| SQLite → PostgreSQL query incompatibilities | Use ANSI SQL only; run the integration test suite against PostgreSQL in CI from Phase 3 onward |
| axum port conflict on desktop | Bind to port 0 (OS assigns) and pass the assigned port to the WebView; retry on bind failure |
| Context window overflow with many chunks | Count tokens before sending; surface a warning in the UI; let the GM limit sources per query |
| LLM hallucinating rules despite strict prompt | Track retrieval scores; show a low-confidence indicator when the top chunk similarity is below a threshold |

---

## Crate & Tool Summary

### Rust Crates

| Purpose | Crate |
|---------|-------|
| Desktop app framework | `tauri` 2.x |
| HTTP server | `axum` |
| PDF text extraction | `pdfium-render` |
| SQLite / PostgreSQL | `sqlx` (feature flags) |
| Vector store | `lancedb` |
| Local embeddings | `fastembed` |
| OpenAI / Ollama LLM | `async-openai` |
| HTTP client (Anthropic) | `reqwest` |
| Async runtime | `tokio` |
| Serialisation | `serde` + `serde_json` |
| Unique IDs | `uuid` |
| Mocking in tests | `mockall` |
| Test HTTP client | `axum-test` |
| Coverage | `cargo-llvm-cov` |
| Audit | `cargo-audit`, `cargo-deny` |

### Frontend / Tooling

| Purpose | Tool |
|---------|------|
| Framework | Svelte 5 + TypeScript |
| Build | Vite |
| Unit / component tests | Vitest + `@testing-library/svelte` |
| API mocking in tests | `msw` (Mock Service Worker) |
| E2E tests | Playwright |
| Linting | ESLint + `@typescript-eslint` + `eslint-plugin-svelte` |
| Formatting | Prettier + `prettier-plugin-svelte` |
| Pre-commit hooks | lefthook |
