# Phase 2 Finalization Plan

**Date:** 2026-06-13
**Status:** Proposed

## Context

Phase 2 ("Campaign & Notes") is ~70% complete. Campaign/entity/session CRUD, all 8
entity types with their temporal and player-character fields, and the markdown notes
editor (with `[[wikilink]]` autocomplete) are implemented and tested on both backend
(`entity_service`, `campaign_service`, `session_service`) and frontend
(`EntityManager.svelte`, `EntityForm.svelte`, `CampaignView.svelte`).

Four threads remain before the milestone — *"take notes on NPCs and events, ask a lore
question, get cited answers from both the sourcebook and your own notes"* — is real:

1. **`is_gm_only`** does not exist anywhere (schema, services, UI). Architecture
   explicitly defers it to Phase 2.
2. **Notes indexing is partial.** Entities carry an `embedding` field (migration
   `006_collection_entities.surql`) and `extraction_service` embeds LLM-extracted
   entities, and `agent_service::fetch_entity_context` already does MTREE KNN over
   them. But manual `entity_service::create`/`update` never (re-)embeds, so
   hand-written/edited notes are invisible to retrieval. Sessions have no embedding
   field at all.
3. **Source scoping** — *resolved by decision, no code work.* Sources are
   collection-scoped (campaigns `subscribes_to` collections); the original
   `campaign = NULL` global-source model is dropped. Architecture doc updated to
   match. Nothing to build.
4. **Test gaps:** event ordering, notes→retrieval integration, `is_gm_only`
   propagation, and the campaign→NPC+event→query backend E2E.

This plan closes them. The milestone-critical items are #2 (notes retrieval) and the
related tests; #1 is a smaller self-contained addition; #3 is already done (doc only).

## Work items

### 1. Notes indexing on manual entity create/update (milestone-critical) — DONE (2026-06-13)

**Shipped:** `entity_service::embed_node` (single source of truth; embeds name +
summary + notes) is called by manual `create_entity`/`update_entity` handlers and by
`extraction_service`. `session_service::embed_session` + migration
`007_session_embedding.surql` make session notes retrievable.
`agent_service::fetch_entity_context` now selects and includes entity **and** session
note excerpts (collapsed/truncated to `NOTES_EXCERPT_LEN`) and queries sessions for the
campaign. Tests: `embed_text`/`embed_node`/`embed_session` unit+integration,
`fetch_entity_context_includes_entity_notes`, `fetch_entity_context_includes_session_notes`,
`notes_excerpt_collapses_and_truncates`. All lib + integration tests green; clippy/fmt clean.

Original detail follows:


Make hand-edited entity notes searchable, matching what `extraction_service` already
does for LLM-extracted entities.

- **Reuse**, don't reinvent: `extraction_service.rs:241` already runs
  `UPDATE $rec SET embedding = $vec, embed_model = $model` after embedding an entity's
  text. Extract that into a shared helper (e.g. `entity_service::embed_entity` or a
  small `embedding` util) and call it from both `extraction_service` and
  `entity_service::create` / `entity_service::update`.
- The embedding input text should match extraction's (name + summary + notes) so
  manually-created and extracted entities are embedded consistently.
- `entity_service::create`/`update` currently take no embedding provider. Thread
  `Arc<dyn EmbeddingProvider>` through (the command handlers already hold it for
  ingestion). On update, re-embed only when `summary`/`notes`/`name` changed.
- **Sessions are in scope and must be retrievable.** Add `embedding`/`embed_model`
  fields to `session` (new migration, mirroring the entity tables) plus an MTREE index,
  and embed session notes on `session_service::create`/`update` using the same shared
  helper. Extend `agent_service::fetch_entity_context` (or add a session path) so
  session note embeddings are KNN-searched alongside entities at query time.

**Files:** `src-tauri/src/services/entity_service.rs`,
`src-tauri/src/services/extraction_service.rs`, `src-tauri/src/services/session_service.rs`,
`src-tauri/src/services/agent_service.rs`, new migration `007_session_embedding.surql`,
`src-tauri/src/commands/entity_commands.rs` / `session_commands.rs`.

### 2. `is_gm_only` end-to-end — DONE (2026-06-13)

**Chat shield (final piece) shipped:** migration `009_message_gm_only.surql`;
`stream_response` returns `StreamHandle { rx, drew_from_gm_only }` (true when any
retrieved chunk is GM-only — retrieval still never filters); the flag is persisted on
the assistant message and threaded to the frontend via both the `chat-token` done event
and `get_chat_history`; `OracleView` renders a "GM only" badge on flagged answers.
Tests: message flag round-trip (`chat_history_test`), badge shown/hidden
(`OracleView.test.ts`). Source-upload toggle UI remains a small follow-up (backend
`upload_source` already accepts the param).

Earlier (data layer + forms) detail:

**Shipped:** migration `008_is_gm_only.surql` (field on source, chunk, session, 8 entity
tables); chunk inheritance from source at index time (`SourceInfo` → `embed_chunks` →
`IndexedChunk` → vector-store upsert); `SearchResult.is_gm_only` (retrieval tags, never
filters); entity & session CRUD persist/return the flag; `upload_source` param;
`EntityForm`/`SessionRow` toggles; TS types. A SurrealDB quirk (falsy bound bool dropped
from a SET that also assigns fields undefined on the table) was fixed by writing
`is_gm_only` in a dedicated UPDATE statement. Tests: vector-store propagation round-trip,
entity/session persistence+toggle, EntityForm toggle component tests — all green.

**Remaining (chat shield indicator):** citations are parsed from the LLM's text response
(`parse_citations`) and persisted as `{source_name, page, text_excerpt}` — they do not
carry `is_gm_only`. To flag an answer as drawing on GM-only material, thread the flag from
the retrieved `SearchResult`s onto the persisted assistant `message` (new field on
`message`, set when any contributing chunk/entity is GM-only), expose it on the chat
message DTO, and render a shield/`EyeMark` indicator in `OracleView`. Source-upload toggle
UI is also pending (backend already accepts the param). This is a separate vertical slice.

Original detail follows:


- **Migration:** new `.surql` adding `is_gm_only TYPE bool DEFAULT false` to `source`,
  `session`, the 8 entity tables, and `chunk`. Follow the additive pattern of existing
  migrations.
- **Propagation:** chunks inherit `is_gm_only` from their source at index time
  (`ingestion_service`); entity embeddings carry the entity's flag. Retrieval does
  **not** filter it out (single-user app) — it only tags results.
- **Backend:** add `is_gm_only` to `EntityInput`/`GraphNode`, session input, source
  upload; persist and return it.
- **Frontend:** toggle in `EntityForm.svelte` and session form; wire the existing
  unused `EyeMark.svelte` (or shield) indicator into chat results / citation rendering
  when a contributing chunk/entity is GM-only.

**Files:** new migration, `entity_service.rs`, `session_service.rs`,
`ingestion_service.rs`, `EntityForm.svelte`, `SessionRow.svelte`, chat/citation
components, `EyeMark.svelte`.

### 3. Source scoping — DONE (no code)

Resolved by decision: sources are collection-scoped (campaigns `subscribes_to`
collections); the `campaign = NULL` global-source model is dropped. The architecture
doc's Multi-Campaign section and Phase 2 checklist have been updated to match. No
implementation work remains.

### 4. Keyboard-first shortcuts — DONE (2026-06-13)

**Shipped (Vim-style g-chords, per user choice):** `lib/shortcuts.ts` (pure, unit-tested
g-chord map + editable-target suppression + help rows); `Shell.svelte` global handler with
a leader-key state machine — `g` then `o/p/n/l/f/c/i/e/s/m/,` navigates, `c` = new entity,
`/` = focus chat, `?` = help overlay, Esc = close; suppressed while typing or when a
modal/picker is open. Cross-view signals (`focusNonce`/`createNonce`) drive chat focus and
the entity create form. Tests: resolver/suppression unit + Shell integration (nav, create,
help toggle, typing suppression). Only follow-up: a source-upload GM-only toggle UI.

Original detail:


- Add a global shortcut layer: quick-search (`/`), new entity (e.g. Ctrl/Cmd+N),
  view navigation. Build on `src/lib/actions/modal.ts` focus handling.
- Can be deferred to a follow-up if Phase 2 needs to close on the data/retrieval
  features first.

**Files:** new `src/lib/actions/shortcuts.ts` (or store), `Shell.svelte`, view components.

### 5. Tests (ship with each item above — TDD) — DONE (2026-06-13)

**Shipped:** event timeline ordering — `order_events_for_timeline` (pure unit test) +
`get_events_timeline` (integration test, `entity_service_test.rs`); notes→retrieval and
`is_gm_only` propagation integration tests (delivered with items 1–2); backend E2E
`tests/e2e_campaign_notes_query.rs` (campaign → NPC + event → `fetch_entity_context` →
both surface, unrelated campaign sees nothing); GM-secret toggle + chat-badge component
tests (delivered with item 2). Original detail:


- **Unit (Rust):** event `sequence_index` ordering / timeline retrieval in
  `entity_service_test.rs`.
- **Integration (Rust):** manual entity note → embed → `fetch_entity_context` returns
  it; session note → embed → retrieved at query time; `is_gm_only` propagation from
  source into chunk and from entity into its embedding tag.
- **Backend E2E:** create campaign → add NPC + event → run agent query → assert both
  surface in retrieved context. New `tests/` file.
- **Component (Vitest):** `is_gm_only` toggle in `EntityForm.test.ts`; GM-secret
  indicator render in chat.

## Verification

- `cargo test` (unit + integration) and `cargo test --test '*'` green, including the
  new notes-retrieval and `is_gm_only` integration tests.
- `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --check`.
- `pnpm test --run` green including new component tests.
- Manual: `cargo tauri dev` → create a campaign, add an NPC with notes and an event,
  ask a lore question that should hit the NPC's notes, confirm the answer cites the
  entity and (if GM-only set) shows the shield indicator.
- Run the existing fixture/retrieval evals to confirm entity-notes embedding didn't
  regress PDF retrieval (`tests/retrieval_recall.rs`).

## Suggested sequencing

1. ~~Item 1 (notes indexing — entities **and** sessions) + its tests~~ — DONE.
2. ~~Item 2 (`is_gm_only`) + its tests~~ — DONE.
3. ~~Item 5 remaining tests (event ordering, backend E2E)~~ — DONE.
4. Item 4 (keyboard shortcuts) — last remaining; a source-upload GM-only toggle is a
   small adjacent follow-up.

(Item 3, source scoping, is already resolved — doc-only, no work.)
