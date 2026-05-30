# Implementation Plan

## Current Implementation Status

After a thorough codebase exploration, here is the real status of each Phase 1 checklist item:

| Item | Status | Evidence |
|------|--------|----------|
| Tauri scaffold + IPC commands | ✅ **Done** | `lib.rs` wires Tauri with `#[tauri::command]` handlers in `commands/mod.rs`; 7 IPC commands registered |
| `LlmProvider` trait + 3 providers | ✅ **Done** | `providers/llm_provider.rs` — trait + `OpenAIProvider`, `AnthropicProvider`, `OllamaProvider` with SSE/NDJSON streaming parsing; unit tests present |
| `VectorStore` trait + `SurrealDbVector` | ✅ **Done** | `providers/vector_store.rs` — search/upsert/delete backed by SurrealDB MTREE index; unit tests present |
| `BlobStore` trait + `LocalFileStore` | ✅ **Done** | `providers/blob_store.rs` — filesystem-backed store/retrieve/delete; unit tests present |
| SurrealDB embedded (RocksDB) + schema | ✅ **Done** | `lib.rs` initializes RocksDB; `schema/mod.rs` runs migrations; `001_initial.surql` defines all 7 Phase 1 tables + indexes |
| Settings screen: LLM provider | ✅ **Done** | `SettingsPage.svelte` — Full provider selection + API key + model + base URL + Save & Connect |
| fastembed integration | ✅ **Done** | `providers/embedding.rs` — `FastEmbedProvider` with `try_new()` and `try_new_small()`; graceful fallback to `MockEmbeddingProvider` on cache miss; unit tests |
| Chunker with section detection | ✅ **Done** | `services/chunker.rs` — sliding window (~400 tokens, ~80 overlap); regex-based heading detection (Chapter, numbered, ALL-CAPS); page-range tracking; 20+ unit tests |
| Chat history (`message` table) | ✅ **Done** | SurQL schema has `message` table; `persist_message()` / `persist_assistant_message()` in agent service; `get_chat_history` IPC command; integration tests |
| CI pipeline | ✅ **Done** | `.github/workflows/ci.yml` — rust-check (fmt + clippy + audit + test), frontend-check (typecheck + lint), coverage + build on main |
| Integration tests | ✅ **Done** | `tests/integration/mod.rs` — schema migration + CRUD tests against in-memory SurrealDB |
| PDF upload UI | ❌ **Missing** | Backend `upload_source` command exists, but frontend has **no upload button, file picker, or progress UI** |
| PDF text extraction | ⚠️ **Stub** | `ingestion_service.rs` `extract_text()` returns empty `ExtractedDoc` — no real PDF parsing |
| Citation rendering in chat UI | ❌ **Missing** | Citations are parsed and stored in DB but the frontend chat displays raw `msg.content` only — no citation highlighting or links |
| Ingestion error recovery | ❌ **Missing** | No checkpoint/resume logic |
| Frontend tests | ❌ **Missing** | No `.spec.ts` or `.test.ts` files anywhere in `src/` |

**Conclusion:** The RAG pipeline is fully wired on the backend (embed → search → prompt → stream → persist). What's missing is the **frontend PDF upload** and **real PDF text extraction** — without these, users cannot provide source material for the RAG system, making the chat UI a "smart but empty" assistant. This is the single most impactful next step.

---

## Goal

Wire up the end-to-end PDF ingestion pipeline: frontend upload UI → file picker → backend text extraction (using `pdf-extract`) → chunking → embedding → storage — so a user can select a PDF, see ingestion progress, and then ask questions with cited answers from that PDF.

## Tasks

### Task 1: Add real PDF text extraction

Replace the empty `extract_text()` stub with a real PDF parser using the `pdf-extract` crate.

- **File:** `src-tauri/Cargo.toml`
  - Add `pdf-extract = "0.7"` to `[dependencies]`
- **File:** `src-tauri/src/services/ingestion_service.rs`
  - Replace the `extract_text()` function body with real PDF extraction using `pdf-extract::extract_text(&data)`.
  - Add `use pdf_extract::extract_text_from_mem;`.
  - Preserve page-level text: use `output_pages` feature or iterate pages by splitting on form feeds (`\x0C`) — a heuristic that works well with `pdf-extract`.
  - Construct `ExtractedDoc` with real `page_count`, full `text`, and `pages` list.
  - Update `get_source_filename()` return type to work with the new flow (it already exists, but verify it's called correctly before extraction).
- **Acceptance:** `cargo test` passes. A Rust unit test ingests a known PDF fixture and returns non-empty text.

### Task 2: Wire ingestion pipeline after upload

Connect `upload_source` command to actually run the ingestion pipeline after storing the blob.

- **File:** `src-tauri/src/commands/mod.rs`
  - In `upload_source()`: after blob store succeeds and source record is created, call `ingestion_service::ingest_source(&state, &source_id).await` in the same request (synchronous for now — the file is small enough for Phase 1).
  - Emit progress events via `app_handle.emit("ingestion-progress", payload)` where payload has `{ source_id: string, status: string, page: number, total_pages: number }` so the frontend can show progress.
  - If ingestion fails, update `index_status` to `'error'` and emit a `"ingestion-error"` event with the error message.
- **Acceptance:** After `upload_source` returns, querying the `chunk` table returns >0 rows for that source.

### Task 3: Add PDF upload UI to the frontend

Create an upload area in the chat view — not just the settings page — so users can drag/select a PDF before asking questions.

- **File:** `src/App.svelte`
  - Add an "Upload PDF" button in the header / nav area that opens a file picker via Tauri's dialog plugin (`@tauri-apps/plugin-dialog`).
  - Show an upload area with file name, progress bar, and status text when an upload is in progress.
  - Listen for `ingestion-progress` and `ingestion-error` Tauri events.
  - On successful upload, show a brief success notification or status indicator.
- **File:** `src/lib/commands.ts`
  - Add `selectPdfFile(): Promise<string>` helper that opens the Tauri file dialog (filtered to `.pdf`).
  - The `uploadSource` function already exists; it may need a minor signature adjustment if `sourceType` and `displayName` should be auto-derived from the chosen file.
- **Acceptance:** User clicks "Upload", picks a PDF, sees progress bar, sees success. Chunks are queryable.

### Task 4: Add citation rendering in chat

Make citations visible and interactive in the streaming chat output.

- **File:** `src/App.svelte` (or create `src/lib/Citation.svelte` if preferred)
  - Parse citation markers `[Source: "name", p.N]` in the message text and render them as styled clickable badges (e.g., a small blue tag with source name and page).
  - The raw text should keep the citation marker in content, but the UI should transform it to a visual element.
  - Keep it simple: regex in the Svelte template or a utility function.
  - Style: `.citation-badge { background: var(--accent); color: #fff; padding: 0.15rem 0.4rem; border-radius: 3px; font-size: 0.8rem; cursor: pointer; }`.
- **Acceptance:** A message with `[Source: "PHB", p.72]` renders as a badge, not raw text.

### Task 5: Add a PDF ingestion test fixture and integration test

- **File:** Create `tests/fixtures/pdfs/single-column.pdf` — generate a 3-page single-column PDF programmatically or commit a small real PDF (~30 KB, simple text).
  - Use Python script or commit a known test PDF. A script using `fpdf2` or similar could work; simpler: commit a small hand-crafted PDF with known content.
  - Alternative: add `pdf-extract` tests in `ingestion_service.rs` that read from a test fixture path.
- **File:** `tests/integration/mod.rs`
  - Add `test_pdf_ingest_and_query_cycle`: create source, upload real PDF bytes via blob store, run `ingest_source`, then search vector store for a known term — assert results have non-empty text and correct page numbers.
  - Mark test as `#[ignore]` if fastembed model not cached, or use `MockEmbeddingProvider` with a small dimension to avoid downloading the real model.
- **Acceptance:** `cargo test --test '*' test_pdf_ingest_and_query_cycle` passes.

## Files to Modify

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `pdf-extract = "0.7"` dependency |
| `src-tauri/src/services/ingestion_service.rs` | Implement real `extract_text()`, wire progress events |
| `src-tauri/src/commands/mod.rs` | Call `ingest_source()` after upload, emit progress events |
| `src/App.svelte` | Add upload button, progress bar, citation rendering |
| `src/lib/commands.ts` | Add `selectPdfFile()` helper |
| `tests/integration/mod.rs` | Add PDF ingest → query cycle integration test |

## New Files

| File | Purpose |
|------|---------|
| `tests/fixtures/pdfs/single-column.pdf` | A small real PDF fixture for integration testing |
| *(optional)* `src/lib/Citation.svelte` | Reusable citation badge component |

## Dependencies

- **Task 1** must be done before **Task 2** (ingestion pipe needs real extraction).
- **Task 2** must be done before **Task 5** (integration test needs working pipeline).
- **Task 3** (upload UI) depends on **Task 2** (backend upload+ingest working), but they can be done in parallel if the frontend stubs the progress listener and wires it after the backend is ready.
- **Task 4** (citation rendering) is independent — can be done anytime, but is most impactful after a PDF has been successfully ingested and a query returns citations.

**Recommended order:** Task 1 → Task 2 → Task 3 → Task 4 → Task 5

## Risks

1. **`pdf-extract` crate compatibility:** The `pdf-extract` crate wraps C++ `pdfium`. It may need `libpdfium` shared library or a build-time download. Check if `pdf-extract` v0.7 works with the existing `Cargo.lock` without pulling conflicting deps. If it causes issues, use `pdf4me` or `lopdf` with manual text extraction instead (more work but no native dep).
2. **Multi-column / stat-block PDFs:** The simple `\x0C` page splitting heuristic won't handle multi-column layouts well. Phase 1 scope is single-column rulebook PDFs; document this limitation.
3. **fastembed model cache on CI:** The integration test should use `MockEmbeddingProvider` (already exists) to avoid downloading the 80 MB embedding model on every CI run. The real model is only needed for the dedicated `test_fastembed_real_model` test.
4. **Progress events threading:** The `ingestion_service::ingest_source()` currently only takes `&Arc<AppState>`. To emit progress events, it needs `&tauri::AppHandle`. Consider passing an `EventEmitter` trait or a `mpsc::Sender<ProgressEvent>` to keep the service layer testable without Tauri.
5. **LLM provider lock contention:** The progress event pattern should not hold the `llm_provider` RwLock during ingestion (ingestion doesn't use the LLM, so this is low-risk).

## Testing Strategy

| Test type | What | Fixture |
|-----------|------|---------|
| Unit (Rust) | `extract_text()` returns non-empty text for a real PDF | `tests/fixtures/pdfs/single-column.pdf` |
| Unit (Rust) | `ingest_source()` stores chunks in SurrealDB | In-memory DB + `MockEmbeddingProvider` |
| Integration (Rust) | Full ingest → vector search → verify chunk text contains expected words | In-memory DB + small PDF fixture + MockEmbedding |
| Unit (Frontend) | Citation regex rendering; upload button click → file dialog | Vitest + `@testing-library/svelte` (no existing frontend tests — start small) |

## Scope Estimate

**Medium** — approximately 5 files to modify, 1-2 new files, 2 crate dependency additions (`pdf-extract` + maybe `tauri-plugin-dialog`). Estimated 6-10 hours of implementation work including testing.