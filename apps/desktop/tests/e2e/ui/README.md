# UI E2E (tauri-driver)

End-to-end tests that drive the **real built Chronacle app** through
[`tauri-driver`](https://v2.tauri.app/develop/tests/webdriver/) — a WebDriver
proxy that launches the native binary and bridges to the platform webview
driver. Unlike the mocked Playwright tests in `../backend/`, these exercise the
actual Rust backend, SurrealDB, PDF extraction, and embeddings.

## ⚠️ Linux only — does not run on macOS

`tauri-driver` supports **Linux (WebKitWebDriver)** and **Windows (Edge
driver)** only. macOS's WKWebView exposes no WebDriver, so these tests **cannot
run on a Mac**. They run in CI on Ubuntu (`.github/workflows/e2e-ui.yml`) under
Xvfb. To run locally, use a Linux machine or container.

## What's here

| File | Purpose |
|------|---------|
| `enrichment-flow.e2e.mjs` | Full flow: index a lore PDF, extract an entity, assert the related-entity summary is rewritten by the second-pass enrichment. |
| `settings-toggle.e2e.mjs` | UI-driven: clicks the "Enrich related entities" checkbox in Settings and confirms it persists through IPC + reload. |
| `stub-llm.mjs` | Deterministic OpenAI-compatible SSE server. Returns canned extraction/profile JSON, branching on the prompt. No API key, fully reproducible. |
| `driver.mjs` | tauri-driver + selenium-webdriver lifecycle and an `invoke()` bridge over the live webview IPC. |
| `fixtures/lore-iron-fist.pdf` | Lore PDF naming a seed NPC ("Commander Varn") and a related faction ("The Iron Fist"). Regenerate with `node fixtures/make-pdf.mjs`. |

## How the enrichment test proves the feature

1. Points `llm_base_url` at the local stub and sets
   `extraction_enrich_neighbors = true`.
2. Indexes the lore PDF (real chunking + embeddings) via `upload_source`.
3. Runs `extract_entity_by_name("Commander Varn")`. The stub's **first** pass
   returns the faction with a *relational* summary ("The militia that Commander
   Varn commands."); the **second** (profile) pass returns an *entity-centric*
   summary ("A militant guild controlling the eastern docks of Varrowmoor.").
4. Asserts the persisted faction ends up with the entity-centric summary — i.e.
   the second pass overwrote the relational one. With the setting off, it
   wouldn't.

## Running (on Linux)

Run from the `apps/desktop/` directory (or use `pnpm -C apps/desktop <cmd>`
from the repo root). Build via the Tauri CLI — plain `cargo build --release`
does NOT embed the frontend (`frontendDist`) and the SPA will not serve.

```bash
cargo install tauri-driver --locked       # once
sudo apt-get install -y webkit2gtk-driver xvfb   # WebKitWebDriver + headless X

pnpm install
pnpm exec tauri build --no-bundle --features rocksdb # embeds dist/ + persistent database
xvfb-run -a pnpm e2e:ui                  # or omit xvfb-run on a real display
```

The first run downloads the embedding model (`nomic-embed-text-v1.5`), so allow
extra time.

## Notes / limitations

- Setup uses `invoke()` through the webview rather than clicking through PDF
  upload, because native file dialogs aren't WebDriver-addressable.
  `settings-toggle.e2e.mjs` does drive real DOM clicks for the part that has a
  dialog-free UI path.
- The specs run sequentially (each spawns its own `tauri-driver` on port 4444).
- This harness has only been exercised in Linux CI; it is not runnable on the
  maintainer's macOS dev machine (see above).
