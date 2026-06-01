# Chronacle

<p align="center">
  <img src="src-tauri/app-icon.png" alt="Chronacle app icon" width="128" height="128">
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
┌─────────────────────────────────────┐
│  Tauri Shell (Desktop UI)           │
│  ┌──────────────┐  ┌──────────────┐ │
│  │ Svelte 5     │◄─┤ Tauri IPC    │ │
│  │ Frontend     │  │ (commands +   │ │
│  │              │  │  events)      │ │
│  └──────────────┘  └──────┬───────┘ │
├────────────────────────────┼────────┤
│  Rust Backend              │        │
│  ┌─────────────────────────▼──────┐ │
│  │ Service Layer                  │ │
│  │  LlmProvider · VectorStore ·  │ │
│  │  BlobStore (all via traits)   │ │
│  └─────────────────┬─────────────┘ │
│  ┌─────────────────▼─────────────┐ │
│  │ SurrealDB (relational +       │ │
│  │  vector + graph, embedded)    │ │
│  └───────────────────────────────┘ │
└─────────────────────────────────────┘
```

All external dependencies are behind Rust traits (`Arc<dyn LlmProvider>`, `Arc<dyn VectorStore>`, `Arc<dyn BlobStore>`), making the service layer testable and portable to a server deployment later.

---

## Prerequisites

| Tool    | Version  | Note                                            |
|---------|----------|-------------------------------------------------|
| Rust    | ≥ 1.81   | Install via [rustup](https://rustup.rs)          |
| Node.js | ≥ 20     | Use [nvm](https://github.com/nvm-sh/nvm) or fnm  |
| pnpm    | ≥ 9      | `npm install -g pnpm`                            |
| Tauri   | —        | CLI bundled via pnpm; system deps listed below   |

### System Dependencies (Tauri 2)

- **macOS:** Xcode Command Line Tools (`xcode-select --install`)
- **Linux:** `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`
- **Windows:** WebView2 (included in Windows 11 / 10 with latest updates)

For full details, see the [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/).

---

## Build & Run

```bash
# Clone the repository
git clone https://github.com/nunico/chronacle.git
cd chronacle

# Install frontend dependencies
pnpm install

# Run in development mode (hot-reload frontend + debug backend)
pnpm tauri dev

# Build a production release
pnpm tauri build
```

---

## Development Commands

### Rust Backend

```bash
cargo build                          # Build all workspace crates
cargo fmt && cargo fmt --check       # Format / check formatting
cargo clippy --all-targets --all-features -- -D warnings
cargo test                           # Unit + integration tests
cargo test -- --nocapture <name>     # Single test with output
cargo llvm-cov --html                # Coverage report (HTML)
cargo audit                          # Security audit
cargo deny check                     # License + dependency check
```

### Frontend

```bash
pnpm dev                 # Vite dev server (standalone)
pnpm typecheck           # TypeScript type checking
pnpm lint                # ESLint
pnpm test:run            # Vitest (CI mode)
pnpm test:coverage       # With coverage
pnpm playwright test tests/e2e/backend/   # E2E backend tests
```

### Full App

```bash
pnpm tauri dev           # Dev with hot-reload
pnpm tauri build         # Production bundle
```

---

## Project Structure

```
chronacle/
├── src/                 # Frontend (Svelte 5 + TypeScript)
│   ├── App.svelte
│   ├── main.ts
│   ├── lib/             # Shared UI components and utilities
│   ├── ChatPage.svelte
│   ├── CampaignsPage.svelte
│   ├── SettingsPage.svelte
│   ├── UploadProgress.svelte
│   ├── ModelDownload.svelte
│   └── App.test.ts
├── src-tauri/           # Rust backend (Tauri 2)
│   ├── src/
│   │   ├── main.rs      # Binary entrypoint
│   │   ├── lib.rs       # Library root
│   │   ├── commands/    # Tauri IPC command handlers
│   │   ├── services/    # Business logic (rag, ingestion, etc.)
│   │   ├── providers/   # LLM, vector, blob store impls
│   │   └── schema/      # SurrealQL schema definitions
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                # Architecture docs and ADRs
├── tests/               # Integration tests and fixtures
├── package.json
├── Cargo.toml           # Workspace root
└── vite.config.ts
```

---

## Tech Stack

| Layer        | Technology                                     |
|-------------|------------------------------------------------|
| Shell       | [Tauri 2](https://v2.tauri.app) (Rust + WebView) |
| Frontend    | [Svelte 5](https://svelte.dev) + TypeScript + Vite |
| Backend     | Rust                                           |
| Storage     | [SurrealDB](https://surrealdb.com) (embedded, RocksDB) |
| Embeddings  | [fastembed-rs](https://github.com/Anush008/fastembed-rs) (local) |
| LLM         | OpenAI / Anthropic / Ollama (configurable via traits) |
| PDF         | pdfium-render                                  |

---

## License

Copyright © 2026-present Nico Nußbaum.

**Code:** Licensed under the **GNU Affero General Public License v3.0** — see [`LICENSE`](./LICENSE).

**Brand Assets:** The project name "Chronacle", its logos, icons, and trade dress are **not** covered by the AGPL. They may not be used in modified or redistributed versions without explicit permission. See [`LICENSE-EXCEPTION.md`](./LICENSE-EXCEPTION.md).

Forks must remove or replace all Brand Assets.
