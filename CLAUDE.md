# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

Chronacle is a desktop TTRPG GM assistant: load rulebook PDFs, take structured notes, query an AI agent that answers with source citations. Stack and all architectural decisions: [`docs/architecture.md`](docs/architecture.md). Status: **pre-implementation, Phase 1**.

## Commands

```bash
# Rust
cargo build
cargo fmt && cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test                              # unit + integration
cargo test -- --nocapture <test_name>   # single test
cargo test --test '*'                   # integration only
cargo audit && cargo deny check
cargo llvm-cov --html

# Frontend
pnpm dev && pnpm typecheck && pnpm lint
pnpm test --run                         # Vitest CI mode
pnpm test --run --coverage
pnpm playwright test tests/e2e/backend/
pnpm playwright test tests/e2e/ui/      # requires built Tauri app

# Full app
cargo tauri dev && cargo tauri build
```

## Hard constraints

- **Tauri IPC only.** Frontend talks to the Rust backend via `invoke()` commands and `app.listen()` events. No HTTP server, no WebSocket (ADR-005).
- **Traits for all external deps.** Never call SurrealDB, filesystem, or LLM APIs directly — always through `Arc<dyn LlmProvider>`, `Arc<dyn VectorStore>`, `Arc<dyn BlobStore>`. Tests inject `Mock*` variants.
- **SurrealQL for all queries.** No SQL. Schema is defined via `DEFINE` statements in `.surql` migration files. No compile-time query validation — all queries are tested at runtime.
- **Embedding model identity.** Store model ID in `source.embed_model` and `chunk.embed_model` at index time; detect mismatch at startup. Silently switching models corrupts retrieval (ADR-003).
- **`is_gm_only` deferred to Phase 2.** In Phase 1, everything is GM-visible. No `is_gm_only` field in the data model until Phase 2.
- **Approved crates only.** No new `Cargo.toml` entries outside the "Crate & Tool Summary" table in the architecture doc without an ADR.

## Testing

Tests ship with every feature — never after.

- **Unit:** `#[cfg(test)]` in the same file. Mock all deps with `mockall`.
- **Integration:** `tests/` dir. SurrealDB in-memory engine (`mem::Db`) per test — run schema setup, drop on completion. Test service layer directly, no HTTP layer. `tempfile::TempDir` for filesystem tests.
- **Frontend:** Vitest + `@testing-library/svelte`. Backend mocked with `msw`.
- **E2E:** Playwright at `tests/e2e/backend/` (every PR); tauri-driver UI tests at `tests/e2e/ui/` (merge to main only).
- **Fixtures:** `tests/fixtures/pdfs/` (diverse suite: single-column, multi-column, tables, stat-block, scanned), `tests/fixtures/llm/*.json`, `tests/fixtures/db/*.surql`.

## Key data-model facts

- `source.campaign = NULL` → global source (shared across all campaigns).
- SurrealDB partitioned by `campaign` record link; retrieval searches global + campaign chunks.
- `entity.sequence_index` is the canonical ordering key; `date_start`/`date_end` are opaque strings — never parsed.
- `setting` keys: `llm_provider`, `llm_model`, `llm_api_key` (encrypted), `llm_base_url`, `embedding_backend`, `active_campaign_id`, `vault_sync_path`, `vault_include_gm_only`.
- `message` table stores chat history from Phase 1.

## Phases

| Phase | Goal |
|-------|------|
| 1 | Core RAG — load PDF, ask question, get cited answer ← *current* |
| 2 | Campaign & Notes — multi-campaign, entity manager, hybrid retrieval |
| 3 | Polish — session timeline, entity graph, vault sync, conditional cross-encoder |
| 4 | Cloud/Mobile — axum extraction, SurrealDB Cloud, JWT auth, mobile |

## Subagents (`.claude/agents/`)

| Agent | Use for |
|-------|---------|
| `planner` | Feature decomposition, ADR drafting |
| `implementer` | Production Rust / Svelte code |
| `test-engineer` | Writing or debugging tests |
| `librarian` | "Where is X?" / architecture Q&A |
| `doc-writer` | ADRs, `///` doc comments, README |
| `code-reviewer` | Pre-merge review |
| `bug-detective` | Root-cause analysis and fixes |
| `dependency-auditor` | `cargo audit`, licenses, unapproved crates |
| `user-guide-writer` | GM-facing feature guides, how-tos, worked examples |
