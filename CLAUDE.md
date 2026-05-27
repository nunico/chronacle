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

- **No Tauri IPC.** Frontend talks to the Rust backend via `fetch()` and WebSocket only. The axum server runs on a random `localhost` port injected into the WebView (ADR-005).
- **Traits for all external deps.** Never call LanceDB, filesystem, or LLM APIs directly — always through `Arc<dyn LlmProvider>`, `Arc<dyn VectorStore>`, `Arc<dyn BlobStore>`. Tests inject `Mock*` variants.
- **ANSI SQL only.** No SQLite-specific syntax. `sqlx::query!` macros. Every schema change needs `UP`+`DOWN` migrations.
- **Embedding model identity.** Store model ID in `sources.embed_model` at index time; detect mismatch at startup. Silently switching models corrupts retrieval (ADR-003).
- **`is_gm_only` propagates everywhere.** Flag on `source`/`entity`/`session`/`player_character_details` must inherit to every LanceDB chunk derived from it.
- **Approved crates only.** No new `Cargo.toml` entries outside the "Crate & Tool Summary" table in the architecture doc without an ADR.

## Testing

Tests ship with every feature — never after.

- **Unit:** `#[cfg(test)]` in the same file. Mock all deps with `mockall`.
- **Integration:** `tests/` dir. `sqlx::test` (in-memory SQLite), `tempfile::TempDir` for LanceDB, `axum-test::TestClient` (no real port).
- **Frontend:** Vitest + `@testing-library/svelte`. Backend mocked with `msw`.
- **E2E:** Playwright at `tests/e2e/backend/` (every PR); tauri-driver UI tests at `tests/e2e/ui/` (merge to main only).
- **Fixtures:** `tests/fixtures/pdfs/`, `tests/fixtures/llm/*.json`, `tests/fixtures/db/*.sql`.

## Key data-model facts

- `sources.campaign_id = NULL` → global source (shared across all campaigns).
- LanceDB partitioned by `campaign_id`; retrieval searches global + campaign chunks + campaign notes.
- `event_details.sequence_index` is the canonical ordering key; `date_start`/`date_end` are opaque strings — never parsed.
- `settings` keys: `llm_provider`, `llm_model`, `llm_api_key` (encrypted), `llm_base_url`, `embedding_backend`, `active_campaign_id`.

## Phases

| Phase | Goal |
|-------|------|
| 1 | Core RAG — load PDF, ask question, get cited answer ← *current* |
| 2 | Campaign & Notes — multi-campaign, entity manager, hybrid retrieval |
| 3 | Polish — session timeline, entity graph, cross-encoder reranking |
| 4 | Cloud/Mobile — Docker, PostgreSQL, auth, Tauri Mobile |

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
