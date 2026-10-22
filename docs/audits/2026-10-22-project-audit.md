# Project audit — 2026-10-22

**Baseline:** `f194470` · **Scope:** desktop, Rust services, website/manual, plans, CI.
**Disposition:** Open findings; no fixes implemented. [Roadmap](../roadmap.md) · [Stories](../superpowers/specs/2026-10-22-reliability-user-stories.md).

## How to read this

- **Priority:** P1 = evidence/ownership/false completion; P2 = blocked/stale/unrecoverable workflow; P3 = content/accessibility/planning debt.
- **Confidence:** Source = code-traced, native reproduction outstanding; Browser = reproduced on the local dev site, not the static production build.
- IDs are stable; source lines describe the baseline and will drift.

## Answer reliability and ownership

### F01 · P1 · Source — wrong source model identity → REL-01

Upload hard-codes Nomic; ingestion never replaces the source label with the actual indexing provider. Cloud/multilingual chunks disagree with source metadata; reindex cannot reliably clear mismatch warnings.
**Evidence:** `apps/desktop/src-tauri/src/commands/source_commands/upload.rs:35`; `crates/chronacle-ingestion/src/ingestion_service/mod.rs:164`.
**Action:** stamp the provider identity used for each completed index generation.

### F02 · P1 · Source — incomplete model migration → REL-02

Consistency checks read sources; reindex visits PDFs only. Entity/rule searches can mix old vectors with the new query model.
**Evidence:** `crates/chronacle-providers/src/embedding/consistency.rs:47`; `apps/desktop/src-tauri/src/commands/source_commands/reindex.rs:19`; `crates/chronacle-retrieval/src/agent_service/context/entity.rs:96`; `rules_block.rs:117` in the same service.
**Action:** inventory, migrate, and gate all searchable embeddings by model identity.

### F03 · P1 · Source — chat stream loses campaign ownership → REL-03

Oracle remains mounted across campaign changes and accepts every token event. Events have no request/campaign ID. Switching A → B during generation can display A's answer in B; Save to Codex uses B. Remounting can permit overlapping streams.
**Evidence:** `apps/desktop/src/views/OracleView.svelte:58,194`; `apps/desktop/src-tauri/src/commands/chat_commands.rs:65`; `apps/desktop/src/shell/Shell.svelte:813`.
**Action:** retain request ownership across navigation and correlate every event and downstream action.

### F04 · P1 · Source — Maintenance creates in the active scope → REL-04

Findings are global; missing-article creation carries only name/finding ID and uses the active campaign. A finding from A can create in B and be dismissed.
**Evidence:** `apps/desktop/src/views/MaintenanceView.svelte:131,456`; `apps/desktop/src/shell/Shell.svelte:855,868`; `apps/desktop/src-tauri/src/commands/codex_commands.rs:311`.
**Action:** carry the finding's owning campaign/collection through creation and navigation.

### F05 · P1 · Source — deleted books retain derived influence → REL-05

PDF deletion removes blob/chunks/source, but leaves compiled knowledge available. Rule retrieval does not check source existence or staleness.
**Evidence:** `apps/desktop/src-tauri/src/commands/source_commands/query.rs:89`; `crates/chronacle-retrieval/src/agent_service/rules_block.rs:117`.
**Action:** invalidate derived evidence on deletion; retain independent GM notes.

### F06 · P1 · Source — stream failures appear complete → REL-06

SSE error payloads are ignored and EOF does not require a completion marker. Oracle recognizes an error only when the whole response is an error marker; partial content plus error lacks the normal Retry state.
**Evidence:** `crates/chronacle-providers/src/llm_provider/sse.rs:45,76,128`; `apps/desktop/src/views/OracleView.svelte:369`.
**Action:** separate content from explicit terminal status end to end.

### F07 · P1 · Source — extraction retry leaves missing vectors → REL-07

Embedding failure is logged as best effort. Re-extraction finds the entity and skips its embedding, leaving it undiscoverable in collection semantic search.
**Evidence:** `crates/chronacle-extraction/src/extraction_service/persist.rs:52,80`.
**Action:** record and repair missing/stale embeddings without duplicate entities.

### F08 · P2 · Source — visible chat has no model memory → REL-09

The model receives one user message, with no preceding exchange. Follow-up references cannot use the displayed conversation.
**Evidence:** `crates/chronacle-retrieval/src/agent_service/mod.rs:140`.
**Action:** include bounded history from the request's campaign.

### F09 · P2 · Source — only leading note excerpts reach answers → REL-10

Notes are truncated to 280 leading characters; sessions are fetched wholesale rather than as relevant passages. A fact found only later in notes is absent through this path.
**Evidence:** `crates/chronacle-retrieval/src/agent_service/context/format.rs:6`; `context/entity.rs:34` in the same service.
**Action:** retrieve relevant passages from long entity/session notes.

### F10 · P1 · Source — citations do not reliably verify evidence → REL-08

Ruling cards display model-written quotations without stored-passage verification. Quote-less cards show a generic failure. Separate fallback lookup uses filename plus `LIMIT 1`, ambiguous across collections and chunks on a page.
**Evidence:** `apps/desktop/src/components/RulingCard.svelte:33`; `apps/desktop/src-tauri/src/commands/source_commands/citation.rs:28,45`.
**Action:** use stable evidence references and one verified-passage interaction.

## Daily desktop workflows

### F11 · P2 · Source — timeline stays on the old campaign → REL-11

Timeline loads on mount only; Shell updates its campaign prop without remounting.
**Evidence:** `apps/desktop/src/views/TimelineView.svelte:23`; `apps/desktop/src/shell/Shell.svelte:832`.
**Action:** reload on scope changes and reject obsolete results.

### F12 · P2 · Source — delayed reads overwrite current scope → REL-11

History/subscription reads assign results without request-generation checks. An old A response can overwrite B or a later A visit.
**Evidence:** `apps/desktop/src/views/OracleView.svelte:133`; `apps/desktop/src/views/CampaignView.svelte:109`; `apps/desktop/src/components/EntityManager.svelte:391`.
**Action:** fence all affected reads; use SessionLogView's generation checks as a reference.

### F13 · P2 · Source — local download blocks first-run alternatives → REL-12

App gates the shell on model readiness; an uncached supported local runtime offers only Download/Retry. Listener setup sits outside error handling and can strand downloading state.
**Evidence:** `apps/desktop/src/App.svelte:18`; `apps/desktop/src/ModelDownload.svelte:47,75,138`.
**Action:** expose cloud selection and ordinary notes without local download; recover listener failure.

### F14 · P2 · Source — Connect hides failed configuration saves → REL-13

Save catches errors and returns; Connect continues and can announce success. Independent setting writes can partially update configuration.
**Evidence:** `apps/desktop/src/views/SettingsView.svelte:287,340`.
**Action:** atomically validate/apply settings and stop on failure.

### F15 · P2 · Source — upload success leaves stale lists/counts → REL-14

Cached collection lists and Oracle's mount-only source check are not refreshed after upload. Book totals count only expanded/lazily loaded collections.
**Evidence:** `apps/desktop/src/views/CampaignView.svelte:174,330`; `apps/desktop/src/views/OracleView.svelte:187`; `apps/desktop/src/shell/Shell.svelte:672`.
**Action:** invalidate affected library state and use authoritative totals.

### F16 · P2 · Source — edit/wiki actions have no visible result → REL-15

Edit details does not open the collapsed editor. Session and entity note previews render entity buttons without navigation callbacks.
**Evidence:** `apps/desktop/src/views/CampaignView.svelte:214,406`; `apps/desktop/src/components/SessionRow.svelte:330`; `EntityManager.svelte:1102` in the same components directory.
**Action:** reveal/focus the editor and wire entity destinations without losing drafts.

### F17 · P2 · Source — failures appear empty → REL-16, REL-17

Session load failures select the empty state; failed entity loads are marked complete. Extraction parse failures become empty results; zero new entities can be reported as no passages despite deduplication or new relations.
**Evidence:** `apps/desktop/src/views/SessionLogView.svelte:248`; `apps/desktop/src/components/SessionRow.svelte:135,240`; `crates/chronacle-extraction/src/extraction_service/parse.rs:66`; `apps/desktop/src-tauri/src/commands/extraction_commands.rs:29`.
**Action:** expose separate empty/existing/partial/failed outcomes and targeted retry.

### F18 · P2 · Source — campaign deletion leaves content behind → REL-18

Cascade deletion leaves PDF blobs; the caller does not clean them. Collection cleanup omits rules; campaign cleanup omits sessions/messages. These become unreachable through normal parent views.
**Evidence:** `crates/chronacle-domain/src/collection_service/crud.rs:205,225`; `crates/chronacle-domain/src/campaign_service.rs:234`; `apps/desktop/src-tauri/src/commands/campaign_commands.rs:90`.
**Action:** define complete cascade/convert semantics with retryable file cleanup.

### F22 · P3 · Source — citation accessibility is incomplete → REL-22

Expanded controls omit `aria-expanded`/`aria-controls`; the floating passage has no explicit focus handoff/return.
**Evidence:** `apps/desktop/src/components/RulingCard.svelte:33`; `apps/desktop/src/views/OracleView.svelte:603`.
**Action:** verify keyboard and screen-reader passage access.

### F23 · P3 · Source — Enter ignores IME composition → REL-22

Oracle submits Enter without checking `isComposing`.
**Evidence:** `apps/desktop/src/views/OracleView.svelte:385`.
**Action:** composition confirmation must not send the question.

## Website and manual

### F19 · P2 · Browser — navigation leaves overlays open → REL-19

At 390px: quick-start article → Contents → Install changes the article while the drawer remains open. Install → Search → fallback Troubleshooting likewise leaves search open over the destination. Rechecked after destination rendering.
**Evidence:** `apps/website/src/lib/manual/ManualDrawer.svelte:122`; `apps/website/src/lib/search/SearchDialog.svelte:393`.
**Action:** dismiss after navigation and move focus to the destination.

### F20 · P2 · Source — search permanently caches initial failure → REL-20

Dialog initialization and Pagefind module loading retain rejected promises; reopening cannot recover without reload. The dev server's missing built Pagefind index is expected and is not a production-search defect.
**Evidence:** `apps/website/src/lib/search/SearchDialog.svelte:142`; `apps/website/src/lib/search/pagefind.ts:54`.
**Action:** clear failed initialization and provide Retry.

### F21 · P3 · Source/Browser — installation, editorial, touch gaps → REL-21, REL-22

Manual installation omits actionable architecture/package/Flatpak instructions already present in README. Thirty-eight German articles retain proofreading markers. Mobile landing Manual/Source links measured 16×16px.
**Evidence:** `apps/website/src/content/manual/en/getting-started/install.md:28`; `README.md:97`; `apps/website/src/lib/components/SiteHeader.svelte:172`; German manual proofreading markers.
**Action:** add package selection, review paired German copy, enlarge touch targets.

## Verification and planning gaps

- **Q01 · P1 → REL-23:** recall harness is feature-gated and ignored; missing model/PDFium returns success. Documented command omits `rocksdb`; normal CI does not run it. Historical 100% covers twelve English fixture queries. Evidence: `apps/desktop/src-tauri/tests/retrieval_recall.rs:1,80`; `docs/phase-1-retrieval-eval.md:94`; `scripts/ci/backend-quality.sh:10`.
- **Q02 · P2 → REL-24:** website built-index/browser suites are omitted from configured gates. Evidence: `scripts/ci/frontend-quality.sh:7`; `apps/website/package.json:17`; `apps/website/vite.config.ts:14`.
- **Q03 · P1 → REL-25:** ADR-013 requires native macOS Quit initiators plus Cancel/confirm; release validation only checks process survival. This is missing verification, not proven draft loss. Evidence: `docs/architecture.md:1723`; `scripts/ci/release/validate-macos-packages.sh:25`.
- **D01 · P3 → REL-26:** Phase-1/pre-implementation status and unchecked vault/rename items are stale. Timeline, graph, vault sync, campaign editing, and substantial draft recovery exist. Historical handovers are not live backlogs. Evidence: `AGENTS.md:65`; `docs/architecture.md:1092,1121,1158`; `crates/chronacle-vault/src/reconcile.rs`; `apps/desktop/src/views/CampaignView.svelte:214`.

## Confirmed unfinished work

- **R01 → REL-27:** incremental extraction; current sweep reloads every chunk and calls the LLM per batch (`extraction_service/collection.rs:36,75`).
- **R02 → REL-28:** per-query source selection (`docs/architecture.md:1155`).
- **R03 → REL-29:** searchable history; clear/new-thread controls also absent, with retention semantics undecided (`docs/architecture.md:1156`; manual `notes-and-sessions/chat-history.md`).
- **R04 → REL-30:** session Markdown/PDF export (`docs/architecture.md:1157`); plan Markdown first.
- **R05 → REL-31:** soft-delete restore; explicitly absent (`docs/architecture.md:696`).
- **R06 · Parked:** ingestion checkpoint/resume; cleanup/retry was deliberate (`docs/architecture.md:1103`); correct contradictory checkpoint wording before extending it.
- **R07 · Parked:** passage-level GM secrecy and player-safe export; do not restore the reverted whole-record flag (`docs/architecture.md:1039`).
- **R08 · Parked:** cloud/server/mobile (`docs/architecture.md:1163`).
- **R09 · Conditional:** reranking requires an expanded quality measurement, not automatic implementation (`docs/architecture.md:1160`).

## Unconfirmed probes

- **H01 — cancellation atomicity:** pause between entity-row creation and scope membership; cancel extraction; assert no unscoped or permanently unindexed entity. Evidence: `crates/chronacle-extraction/src/entity_service/crud/write.rs:27,83`; `apps/desktop/src-tauri/src/commands/extraction_commands.rs:239`.
- **H02 — KNN scope/filter ordering:** seed more than K closer excluded rows and farther subscribed rows; assert in-scope recall. Evidence: `crates/chronacle-providers/src/vector_store.rs:140`; `crates/chronacle-retrieval/src/agent_service/rules_block.rs:107`.
- **Decision:** record actual results first. A passing probe closes the hypothesis; a failing probe becomes a scoped story. Neither is a confirmed defect today.

## Validation recorded during the audit

- `mise exec -- pnpm -C apps/desktop test:run`: 46 files, 622 tests passed.
- `mise exec -- pnpm -C apps/desktop typecheck`: passed.
- `mise exec -- pnpm -C apps/desktop exec svelte-check --tsconfig ./tsconfig.json`: zero errors/warnings.
- `mise exec -- pnpm -C apps/website test:run`: 15 files, 141 tests passed; four static-preview tests passed.
- `mise exec -- pnpm -C apps/website typecheck`: zero errors/warnings.
- Browser: local landing/manual, mobile dimensions, F19 journeys. Development server stopped afterward.
- Connected GitHub query `repo:nunico/chronacle is:issue is:open`: returned no issues; not an independently verified tracker inventory.
- Not run: Rust suite, Docker PR gate, release build, built Pagefind/browser suites, native desktop E2E, or macOS packaged Quit journeys.
- Source tracing used; no source files changed during the audit.
