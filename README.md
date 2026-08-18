# Chronacle

<p align="center">
  <img src="apps/desktop/src-tauri/app-icon.png" alt="Chronacle app icon" width="128" height="128">
</p>

**A desktop TTRPG GM assistant.** Load your own rulebook PDFs, take structured notes, and query an AI agent that answers with source citations.

No cloud dependency, no subscription — the LLM backend is configurable at setup: use a cloud API (OpenAI, Anthropic, OpenRouter) or run locally via Ollama.

---

## Features

- **PDF Ingestion** — Import TTRPG sourcebooks, rules references, and lore. Extracts text with chapter/section structure preserved.
- **RAG Query** — Ask your GM questions in natural language. Every answer cites the source page and chunk.
- **Structured Notes** — Organise campaigns with entities, relationships, and session logs.
- **Configurable AI** — Bring your own LLM (OpenAI, Anthropic, Ollama) and embedding model.
- **Local-First** — All data stays on your machine. No accounts, no telemetry.

---

## Architecture

```
┌────────────────────────────────────┐
│  Tauri Shell (Desktop UI)          │
│  ┌──────────────┐  ┌─────────────┐ │
│  │ Svelte 5     │◄─┤ Tauri IPC   │ │
│  │ Frontend     │  │ (commands + │ │
│  │              │  │  events)    │ │
│  └──────────────┘  └───────┬─────┘ │
├────────────────────────────┼───────┤
│  Rust Backend              │       │
│  ┌─────────────────────────▼─────┐ │
│  │ Service Layer                 │ │
│  │  LlmProvider · VectorStore ·  │ │
│  │  BlobStore (all via traits)   │ │
│  └─────────────────┬─────────────┘ │
│  ┌─────────────────▼─────────────┐ │
│  │ SurrealDB (relational +       │ │
│  │  vector + graph, embedded)    │ │
│  └───────────────────────────────┘ │
└────────────────────────────────────┘
```

All external dependencies are behind Rust traits (`Arc<dyn LlmProvider>`, `Arc<dyn VectorStore>`, `Arc<dyn BlobStore>`), making the service layer testable and portable to a server deployment later.

---

## Prerequisites

| Tool    | Version | Note                                                                               |
| ------- | ------- | ---------------------------------------------------------------------------------- |
| Rust    | ≥ 1.95  | Managed by [mise](https://mise.jdx.dev)                                            |
| Node.js | ≥ 24    | Managed by [mise](https://mise.jdx.dev); pinned to LTS in [`mise.toml`](mise.toml) |
| pnpm    | ≥ 11    | Managed by [mise](https://mise.jdx.dev)                                            |
| Tauri   | ≥ 2.11  | CLI bundled via pnpm; system deps listed below                                     |

### Install with mise

This project uses **[mise](https://mise.jdx.dev)** to manage tool versions (defined in [`mise.toml`](mise.toml)).

```bash
# Install mise (macOS / Linux)
curl https://mise.run | sh
# Or via Homebrew: brew install mise

# Activate mise in your shell (add to ~/.zshrc / ~/.bashrc)
eval "$(mise activate)"

# Install all project tools (Rust, Node.js, pnpm)
cd chronacle
mise install
```

> **Note:** Node.js, pnpm, and Rust are pinned per-project by `mise.toml` and installed with a single `mise install` command. No need for rustup, nvm, or npm `-g` installs.

### System Dependencies (Tauri 2)

- **macOS:** Xcode Command Line Tools (`xcode-select --install`)
- **Linux:** `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`
- **Windows:** WebView2 (included in Windows 11 / 10 with latest updates)

For full details, see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/).

---

## Install a Release

Release builds are configured for these platforms and architectures:

| Platform | Architectures        | Packages                                                  |
| -------- | -------------------- | --------------------------------------------------------- |
| Linux    | x86_64, aarch64      | AppImage, Debian, RPM, Flatpak bundle                     |
| macOS    | Apple Silicon, Intel | DMG, app archive                                          |
| Windows  | x86_64               | MSI, NSIS; usable on Windows 11 Arm through x64 emulation |

Download the package for your system from the GitHub release. Flatpak users should select the
single-file bundle matching their Linux architecture. Set `VERSION` to the release version without
the leading `v`, then install it with the matching command:

```bash
VERSION=1.2.3

# x86_64 Linux
flatpak install --user "./Chronacle_${VERSION}_x86_64.flatpak"

# aarch64 Linux
flatpak install --user "./Chronacle_${VERSION}_aarch64.flatpak"

flatpak run dev.tea-driven.chronacle.desktop
```

These bundles are GitHub release downloads, not a Flathub listing, and do not receive
repository-backed automatic updates. Flatpak keeps Chronacle's application data in its private
per-application storage under `~/.var/app/dev.tea-driven.chronacle.desktop/`. PDFs and vault
directories that you select are accessed through desktop portal grants; the package does not have
broad access to your home directory.

---

## Build & Run

```bash
# Clone the repository
git clone https://github.com/nunico/chronacle.git
cd chronacle

# Install frontend dependencies for the desktop app and public website
pnpm install

# Run in development mode (hot-reload frontend + persistent RocksDB backend)
pnpm -C apps/desktop tauri dev --features rocksdb
# Alternatively: mise run dev (uses the same rocksdb feature)

# Build a production release
pnpm -C apps/desktop exec tauri build --features rocksdb
# Alternatively: mise run build (uses the same rocksdb feature)
```

---

## Development Commands

### Rust Backend

```bash
cargo build --workspace                          # Build all workspace crates
cargo fmt --all && cargo fmt --all --check       # Format / check formatting
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace                           # Unit + integration tests
cargo test -p <crate> -- --nocapture <name>      # Single test in a specific crate
cargo llvm-cov --workspace --html                # Coverage report (HTML)
cargo audit                                      # Security audit
cargo deny check                                 # License + dependency check
```

The workspace's default SurrealDB feature is memory-only, which keeps normal backend builds and
tests fast. Enable the desktop crate's `rocksdb` feature only when exercising the persistent app
runtime, for example:

```bash
cargo test -p Chronacle --features rocksdb --test rocksdb_persistence
```

### Frontend

```bash
pnpm -C apps/desktop dev                 # Vite dev server (standalone)
pnpm -C apps/desktop typecheck           # TypeScript type checking
pnpm -C apps/desktop lint                # ESLint
pnpm -C apps/desktop test:run            # Vitest (CI mode)
pnpm -C apps/desktop test:coverage       # With coverage
scripts/ci/acceptance.sh                     # Playwright backend acceptance tests
```

### Public website and manual

The static SvelteKit site lives in `apps/website`. Its landing page is `/`; the canonical manuals
are `/en/manual` and `/de/handbuch`. Manual search is built with Pagefind and searches only the
language currently being browsed. English is the source text; the German first translation is
marked in source wherever proofreading is still pending.

```bash
pnpm -C apps/website dev              # Local Vite development server
pnpm -C apps/website build            # Static build plus Pagefind index in apps/website/build
pnpm -C apps/website typecheck        # Svelte and TypeScript checks
pnpm -C apps/website lint             # ESLint and Prettier checks
pnpm -C apps/website test:run         # Vitest and static-preview tests
pnpm -C apps/website test:e2e         # Build and run Playwright browser tests
pnpm -C apps/website test:pagefind    # Built-index language isolation tests
```

### Full App

```bash
pnpm -C apps/desktop tauri dev --features rocksdb           # Dev with persistence
pnpm -C apps/desktop exec tauri build --features rocksdb    # Production bundle
```

### Pull-request validation

CI and local validation share three named repository scripts:

- `scripts/ci/backend-quality.sh` — Rust formatting, Clippy, workspace tests, and `cargo deny`
- `scripts/ci/frontend-quality.sh` — desktop and website type checks, lint, tests, and static build
- `scripts/ci/acceptance.sh` — Playwright backend acceptance tests

Before creating a Chronacle pull request, run the authoritative PR gate successfully from the
repository root:

```bash
scripts/ci/local-pr.sh
```

This builds the Docker `pr-gate` target and runs the same three scripts as the pull-request jobs.
BuildKit caches Cargo data, build output, Playwright Chromium, and a shared pnpm store across the
frontend and acceptance stages. The local PR gate intentionally excludes merge-only coverage and
release builds, and the separate real-app `tauri-driver` UI E2E workflow.

---

## Project Structure

```
chronacle/
├── Cargo.toml              # Workspace root: members = ["crates/*", "apps/desktop/src-tauri"]
├── crates/
│   ├── chronacle-core/     # Dependency traits + DTOs (LlmProvider, VectorStore, BlobStore, EmbeddingProvider)
│   ├── chronacle-db/       # SurrealQL schema + run_migrations
│   ├── chronacle-providers/# Concrete provider impls (SurrealDB, fastembed, OpenAI/Anthropic/Ollama LLM)
│   ├── chronacle-ingestion/# PDF extraction, chunking, ingestion pipeline
│   ├── chronacle-extraction/# Entity extraction, wikilink resolution
│   ├── chronacle-retrieval/ # RAG agent service (chat + cited answers)
│   └── chronacle-domain/   # Campaign, session, collection, custom-provider CRUD
├── apps/
│   ├── desktop/            # Svelte frontend + Tauri shell
│   │   ├── src/            # Svelte 5 + TypeScript frontend
│   │   ├── src-tauri/      # Rust Tauri backend (commands, AppState, settings_service)
│   │   │   └── tests/      # Rust integration tests + fixtures
│   │   ├── tests/e2e/      # Playwright backend E2E + tauri-driver UI E2E
│   │   ├── package.json
│   │   └── vite.config.ts
│   └── website/            # Static landing page + bilingual searchable manual
│       ├── src/content/manual/{en,de}/ # Canonical user-manual sources
│       ├── tests/e2e/      # Public-site Playwright tests
│       └── build/          # Generated static output (not committed)
├── docs/                   # Architecture docs and ADRs
└── package.json            # pnpm workspace root (minimal)
```

---

## Tech Stack

| Layer      | Technology                                                       |
| ---------- | ---------------------------------------------------------------- |
| Shell      | [Tauri 2](https://v2.tauri.app) (Rust + WebView)                 |
| Frontend   | [Svelte 5](https://svelte.dev) + TypeScript + Vite               |
| Backend    | Rust                                                             |
| Storage    | [SurrealDB](https://surrealdb.com) (embedded, RocksDB)           |
| Embeddings | [fastembed-rs](https://github.com/Anush008/fastembed-rs) (local) |
| LLM        | OpenAI / Anthropic / Ollama (configurable via traits)            |
| PDF        | pdfium-render                                                    |

---

## License

Copyright © 2026-present Nico Nußbaum.

**Code:** Licensed under the **GNU Affero General Public License v3.0** — see [`LICENSE`](./LICENSE).

**Brand Assets:** The project name "Chronacle", its logos, icons, and trade dress are **not** covered by the AGPL. They may not be used in modified or redistributed versions without explicit permission. See [`LICENSE-EXCEPTION.md`](./LICENSE-EXCEPTION.md).

Forks must remove or replace all Brand Assets.
