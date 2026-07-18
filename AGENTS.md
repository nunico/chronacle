# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, Codex, Cursor, Copilot, Gemini, Jules, and others) when working with code in this repository. It follows the open [AGENTS.md](https://agents.md) standard. `CLAUDE.md` is a symlink to this file for backward compatibility.

## Project structure

```text
chronacle/
├── AGENTS.md             # this file — agent guidance (CLAUDE.md → symlink)
├── README.md             # human-facing overview
├── Cargo.toml            # Rust workspace root: members = ["crates/*", "apps/desktop/src-tauri"]
├── package.json          # pnpm workspace root (minimal, no scripts)
├── pnpm-workspace.yaml   # packages: [apps/desktop]
├── lefthook.yml          # git hooks (pre-commit, commit-msg) — ADR-007
├── mise.toml             # toolchain pinning + task definitions (mise run dev / build)
├── deny.toml             # cargo-deny license/advisory policy
├── docs/
│   ├── architecture.md   # authoritative stack + ADRs (source of truth)
│   ├── user-guide.md     # GM-facing usage docs
│   └── superpowers/      # design specs + implementation plans
├── crates/
│   ├── chronacle-core/       # dependency traits + DTOs: LlmProvider, VectorStore, BlobStore, EmbeddingProvider
│   ├── chronacle-db/         # schema/*.surql + run_migrations (SurrealDB embedded + cloud)
│   ├── chronacle-providers/  # concrete impls: SurrealDbVector, LocalFileStore, fastembed/OpenAI/Mock embedding, OpenAI/Anthropic/Ollama LLM
│   ├── chronacle-ingestion/  # pdf_extractor, chunker, ingestion_service, text_normalizer
│   ├── chronacle-extraction/ # entity_service, wikilink, extraction_service
│   ├── chronacle-retrieval/  # agent_service (RAG chat + citation)
│   └── chronacle-domain/     # campaign, session, collection, custom_provider services
├── apps/
│   └── desktop/              # Svelte frontend + Tauri shell
│       ├── src/              # Svelte 5 frontend
│       │   ├── components/   # reusable UI components
│       │   ├── views/        # top-level screens
│       │   ├── shell/        # app chrome / layout
│       │   ├── lib/          # frontend utilities, Tauri invoke wrappers
│       │   ├── App.svelte    # root component
│       │   └── main.ts       # entrypoint
│       ├── src-tauri/        # Rust backend (Tauri)
│       │   ├── src/
│       │   │   ├── commands/ # Tauri IPC command handlers (invoke targets)
│       │   │   └── services/ # settings_service (desktop-only; other services live in crates/)
│       │   ├── capabilities/ # Tauri permission manifests
│       │   ├── resources/    # bundled assets (ONNX models, etc.)
│       │   ├── tests/        # Rust integration tests + fixtures
│       │   └── tauri.conf.json # Tauri app config
│       ├── tests/e2e/        # end-to-end tests
│       │   ├── features/     # Gherkin .feature acceptance specs (ADR-011)
│       │   ├── backend/      # Playwright backend service-layer E2E (every PR)
│       │   │   └── steps/    # playwright-bdd step definitions for features/
│       │   └── ui/           # tauri-driver UI E2E (merge to main only)
│       ├── package.json      # app frontend scripts + deps
│       └── vite.config.ts    # Vite build config
├── .agents/skills/       # portable agent skill packages (source of truth)
├── .claude/
│   ├── agents/           # subagent definitions (see Subagents below)
│   └── skills/           # symlinks → .agents/skills/*
└── scripts/              # release + dev-data helpers
```

## What this project is

Chronacle is a desktop TTRPG GM assistant: load rulebook PDFs, take structured notes, query an AI agent that answers with source citations. Stack and all architectural decisions: [`docs/architecture.md`](docs/architecture.md). Status: **pre-implementation, Phase 1**.

## Commands

```bash
# Rust (run from the workspace root)
cargo build --workspace
cargo fmt --all && cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace                              # unit + integration
cargo test -p <crate> -- --nocapture <test_name>   # single test in a specific crate
cargo test -p <crate> --test '*'                   # integration only for a specific crate
cargo audit && cargo deny check
cargo llvm-cov --workspace --html

# Shared pull-request jobs (run from the workspace root)
scripts/ci/backend-quality.sh                    # Backend quality
scripts/ci/frontend-quality.sh                   # Frontend quality
scripts/ci/acceptance.sh                         # Acceptance tests
scripts/ci/local-pr.sh                           # authoritative Docker PR gate

# Frontend (commands target apps/desktop)
pnpm -C apps/desktop dev && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint
pnpm -C apps/desktop test:run                      # Vitest CI mode
pnpm -C apps/desktop test:coverage
scripts/ci/acceptance.sh                           # generates BDD specs, then runs Playwright
pnpm -C apps/desktop run e2e:ui                    # build first with: tauri build --no-bundle --features rocksdb

# Full app
pnpm -C apps/desktop tauri dev --features rocksdb  # dev with persistent storage
pnpm -C apps/desktop exec tauri build --features rocksdb # production bundle (or: mise run build)
```

The workspace defaults to SurrealDB's memory-only backend for normal builds and tests. The desktop
`rocksdb` feature is explicit: use it for Tauri development/builds and persistence-specific tests
such as `cargo test -p Chronacle --features rocksdb --test rocksdb_persistence`.

**Before creating any Chronacle PR, agents must successfully run `scripts/ci/local-pr.sh`.** It is
the authoritative pre-PR command and executes the Backend quality, Frontend quality, and Acceptance
tests scripts in the repository's Docker toolchain. Its scope is the PR gates only: it does not run
merge-only coverage, release builds, or the separate real-app `tauri-driver` UI E2E workflow.

## License

This project is licensed under **AGPL-3.0 with a Branding Exception**.

- **Code**: Licensed under the GNU Affero General Public License v3.0.
  See [`LICENSE`](LICENSE).
- **Brand Assets**: The project name "Chronacle", its logos, icons, and trade
  dress are **not** covered by the AGPL. They may not be used in modified
  or redistributed versions without explicit permission.
  See [`LICENSE-EXCEPTION.md`](LICENSE-EXCEPTION.md).
- Forks must remove or replace all Brand Assets.

## Hard constraints

- **Tauri IPC only.** Frontend talks to the Rust backend via `invoke()` commands and `app.listen()` events. No HTTP server, no WebSocket (ADR-005).
- **Traits for all external deps.** Never call SurrealDB, filesystem, or LLM APIs directly — always through `Arc<dyn LlmProvider>`, `Arc<dyn VectorStore>`, `Arc<dyn BlobStore>`. Tests inject `Mock*` variants.
- **SurrealQL for all queries.** No SQL. Schema is defined via `DEFINE` statements in `.surql` migration files. No compile-time query validation — all queries are tested at runtime.
- **Embedding model identity.** Store model ID in `source.embed_model` and `chunk.embed_model` at index time; detect mismatch at startup. Silently switching models corrupts retrieval (ADR-003).
- **`is_gm_only` deferred to Phase 3.** Everything is GM-visible (single-user app); no `is_gm_only` field in the data model. A Phase 2 manual-flag attempt was reverted — GM-secret content is passage-level, so it returns in Phase 3 as an AI-detected per-chunk flag alongside player-safe export. See architecture.md "GM-Secret Handling".
- **Approved crates only.** No new `Cargo.toml` entries outside the "Crate & Tool Summary" table in the architecture doc without an ADR.

## Testing

Tests ship with every feature — never after.

- **Unit:** `#[cfg(test)]` in the same file within each crate. Mock all deps with `mockall`.
- **Integration:** `apps/desktop/src-tauri/tests/` dir. SurrealDB in-memory engine (`mem::Db`) per test — run schema setup, drop on completion. Test service layer directly, no HTTP layer. `tempfile::TempDir` for filesystem tests.
- **Frontend:** Vitest + `@testing-library/svelte` under `apps/desktop/src/`. Backend mocked with `msw`.
- **E2E:** Playwright at `apps/desktop/tests/e2e/backend/` (every PR); tauri-driver UI tests at `apps/desktop/tests/e2e/ui/` (merge to main only).
- **Acceptance (BDD, mandatory — ADR-011):** every feature ships Gherkin scenarios in `apps/desktop/tests/e2e/features/*.feature`, executed by the backend Playwright suite via `playwright-bdd` step definitions in `tests/e2e/backend/steps/`. Design specs write acceptance criteria in Gherkin so scenarios transfer verbatim into `.feature` files. Generated `.features-gen/` output is gitignored — never edit or commit it.
- **Fixtures:** `apps/desktop/src-tauri/tests/fixtures/pdfs/` (diverse suite: single-column, multi-column, tables, stat-block, scanned), `tests/fixtures/llm/*.json`, `tests/fixtures/db/*.surql`.

## Code style

Formatting and linting are enforced by tooling and run automatically via `lefthook` pre-commit hooks — do not hand-format. The config files are authoritative; the rules below are the highlights agents must follow.

**Rust (workspace crates)**

- Format with `cargo fmt`; CI runs `cargo fmt --check`. Lint with `cargo clippy --all-targets --all-features -- -D warnings` — **clippy warnings are errors**, leave none.
- Prefer `?` propagation with typed errors over `unwrap()`/`expect()` outside tests.
- Use the dependency traits (`Arc<dyn LlmProvider>`, etc.) — never concrete external clients (see Hard constraints).
- Public items in library crates carry `///` doc comments.

**Frontend (`apps/desktop/src/`) — TypeScript + Svelte 5**

- Formatted by Prettier (`.prettierrc`): semicolons, single quotes, trailing commas (`all`), 2-space indent, 100-char print width, `prettier-plugin-svelte` for `.svelte`.
- Linted by ESLint (`eslint.config.js`): `typescript-eslint` strict + stylistic, `eslint-plugin-svelte` recommended.
  - Arrays: use `array-simple` form — `string[]` for simple, `Array<T>` for complex.
  - Unused vars are warnings; prefix intentionally-unused with `_` (e.g. `_event`).
- Svelte 5 runes (`$state`, `$derived`, `$props`, `$effect`) — do not use legacy `export let` / reactive `$:` syntax.
- `.agents/*`, `.claude/*`, `dist/*`, `target/*` are ESLint-ignored — do not lint or reformat them.

**Markdown / config** — Prettier formats `*.{json,css,yaml,yml,md}`; keep it clean or the pre-commit hook fails.

## Commit & PR guidelines

**Commits**

- Subject line **max 72 characters** — enforced by the `commit-msg` hook; longer subjects are rejected.
- Use imperative mood ("Add entity retrieval", not "Added"/"Adds").
- Conventional-commit prefixes are encouraged: `feat:`, `fix:`, `chore:`, `docs:`, `test:`, `refactor:`.
- Reference the ADR or plan when a change implements one (e.g. `feat: hybrid entity retrieval (ADR-00X)`).
- All `lefthook` pre-commit checks (rustfmt, clippy, prettier, eslint) must pass — never commit with `--no-verify`.

**Pull requests**

- Branch naming: `<type>/<short-slug>` (e.g. `feat/timeline-graph`, `fix/embed-mismatch`, `chore/adopt-agents-md`).
- Keep PRs scoped to one logical change; split unrelated refactors.
- PR description must state: **what** changed, **why**, and **how it was tested** (commands run). Link the relevant `docs/superpowers/` plan or spec.
- Tests ship in the same PR as the feature — never a follow-up (see Testing).
- New or changed user-visible behaviour must add or update `.feature` acceptance scenarios in the same PR (ADR-011).
- Green CI is required: Backend quality, Frontend quality, and Acceptance tests all pass before
  merge. Agents must run `scripts/ci/local-pr.sh` successfully before creating a PR.
- New `Cargo.toml` dependencies require an ADR and an entry in the architecture doc's "Crate & Tool Summary" (see Hard constraints).

## Security & boundaries

**Never read, edit, commit, or print the contents of these — they are generated, secret-bearing, or policy-controlled:**

- **Lockfiles (never hand-edit):** `Cargo.lock`, `pnpm-lock.yaml`, `mise.lock`, `skills-lock.json`. Update only via the owning tool (`cargo`, `pnpm`, `mise`).
- **Secrets & local state:** any `.env*` file, `llm_api_key` / `embedding_api_key` settings values (stored **encrypted** in the `setting` table — never log, echo, or commit decrypted secrets), local SurrealDB data files, and anything matched by `.gitignore`.
- **Generated / build output:** `target/`, `dist/`, `apps/desktop/src-tauri/gen/` — never edit by hand; regenerate via the toolchain.
- **Vendored agent assets:** `.agents/skills/*` and `.claude/skills/*` (the latter are symlinks) — do not reformat or lint; edit a skill only with intent, not as a side effect.
- **License & branding:** do not modify `LICENSE`, `LICENSE-EXCEPTION.md`, or any Brand Assets (name, logos, icons) — these are excluded from the AGPL (see License). Forks must remove/replace Brand Assets.
- **Tauri capability manifests:** treat `apps/desktop/src-tauri/capabilities/*` and `apps/desktop/src-tauri/tauri.conf.json` permissions as security-sensitive — widen scopes only deliberately, never to silence an error.

**General:** never commit credentials, tokens, or API keys in code, fixtures, or test data. Use `apps/desktop/src-tauri/tests/fixtures/` placeholders for anything secret-shaped.

## Key data-model facts

- `source.campaign = NULL` → global source (shared across all campaigns).
- SurrealDB partitioned by `campaign` record link; retrieval searches global + campaign chunks.
- `entity.sequence_index` is the canonical ordering key; `date_start`/`date_end` are opaque strings — never parsed.
- `setting` keys: `llm_provider`, `llm_model`, `llm_api_key` (encrypted), `llm_base_url`, `embedding_backend` (`local` | `openai`; defaults to `local` where ONNX Runtime is bundled, else `openai`), `embedding_model`, `embedding_api_key`, `embedding_base_url` (cloud embedding config; used when `embedding_backend` is `openai`), `active_campaign_id`, `vault_sync_path`, `extraction_enrich_neighbors` (opt-in second-pass that rewrites related-entity summaries to be entity-centric; off by default, capped at 20/extraction).
- `message` table stores chat history from Phase 1.

## Phases

| Phase | Goal                                                                           |
| ----- | ------------------------------------------------------------------------------ |
| 1     | Core RAG — load PDF, ask question, get cited answer ← _current_                |
| 2     | Campaign & Notes — multi-campaign, entity manager, hybrid retrieval            |
| 3     | Polish — session timeline, entity graph, vault sync, conditional cross-encoder |
| 4     | Cloud/Mobile — axum extraction, SurrealDB Cloud, JWT auth, mobile              |

## Subagents (`.claude/agents/`)

| Agent                | Use for                                            |
| -------------------- | -------------------------------------------------- |
| `planner`            | Feature decomposition, ADR drafting                |
| `implementer`        | Production Rust / Svelte code                      |
| `test-engineer`      | Writing or debugging tests                         |
| `librarian`          | "Where is X?" / architecture Q&A                   |
| `doc-writer`         | ADRs, `///` doc comments, README                   |
| `code-reviewer`      | Pre-merge review                                   |
| `bug-detective`      | Root-cause analysis and fixes                      |
| `dependency-auditor` | `cargo audit`, licenses, unapproved crates         |
| `user-guide-writer`  | GM-facing feature guides, how-tos, worked examples |
