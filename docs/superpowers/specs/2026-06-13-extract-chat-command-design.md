# `/extract` chat command with live progress — Design

**Date:** 2026-06-13
**Status:** Approved, ready for implementation planning
**Phase:** 2 (Campaign & Notes — entity manager)

## Problem

Entity extraction today is a per-collection "Extract entities" button in
`CampaignView.svelte`. It has two problems:

1. **No real feedback.** Clicking the button does a full-collection LLM sweep.
   Progress is shown only as text inside the button label (`Batch x/y · n
   found`). The user is left unsure what is happening or whether anything is
   happening at all.
2. **Wrong granularity for everyday use.** It only does an all-or-nothing sweep
   of an entire collection. There is no way to say "build out *this* entity."

We are replacing the button with chat commands that give the user a targeted
operation and a clear, always-legible view of what the system is doing.

## Goals

- A `/extract <entity name>` chat command that builds one named entity plus its
  immediate relation neighborhood (**seed-anchored** extraction).
- A live, in-thread progress card so the user is **never unsure what is
  happening right now**.
- Move the existing full-collection sweep off the button and into an explicit,
  opt-in `/extract-all` chat command — and make it cancellable.
- No accidental triggering of the expensive full sweep.

## Non-goals (YAGNI)

- Queuing multiple concurrent extractions (one at a time, like chat).
- A general plugin-style command registry. Only `/extract`, `/extract-all`,
  and `/help` are recognised.
- Reworking the breadth of the full sweep itself — see *Future work*.

## Command surface

A slash-command parser intercepts chat input in `OracleView.sendMessage`
**before** it reaches `chatSend`. Parsing lives in a pure, testable module
`src/lib/chat-commands.ts`:

```ts
parseCommand(input): 
  | { kind: 'extract'; name: string }
  | { kind: 'extract-all' }
  | { kind: 'help' }
  | { kind: 'extract-usage' }   // bare /extract with no name
  | { kind: 'chat'; text: string }
```

| Input | Behaviour |
|-------|-----------|
| `/extract <name>` | Seed-anchored extraction of `<name>` + its relation neighborhood. |
| `/extract` (no name) | **Usage hint only — never starts work.** Inline message: *"Usage: `/extract <entity name>`. To extract everything from all books, use `/extract-all` (this can take a while)."* |
| `/extract-all` | Explicit, opt-in full sweep across all linked collections. Cancellable. |
| `/help` | Inline list of available commands. |
| `/somethingelse` | Treated like `/help` (unknown command) so a typo is never silently sent to the LLM. |
| anything else | Normal chat — unchanged path through `chatSend`. |

Rationale: a bare `/extract` produces a hint rather than a sweep, removing the
accidental-sweep footgun the full sweep would otherwise create.

## Live status card (frontend)

A new message role — `extraction` — renders via a dedicated
`ExtractionCard.svelte` component in the chat thread.

- **Phase checklist** that fills in as events arrive, e.g.
  `✓ Resolved "Commander Varn"` · `✓ Found 12 passages` · `⟳ Building entity…`
  · `✓ Discovered 4 relations` · `✓ Embedded`.
- **Current phase line with a spinner**, so there is always a legible "what is
  happening right now" indicator.
- **Cancel button** while a run is in flight (most relevant for `/extract-all`).
- **Completion summary**: counts plus clickable links to the created entities
  (reusing the entity-link pattern already used in chat replies).
- **Terminal states** rendered in the same card:
  - Success → summary as above.
  - Empty → *"No passages found for "{name}"."* with a retry affordance.
  - Failure → error text with retry.
  - Cancelled → *"Cancelled — kept N entities / M relations created so far."*

### Progress event

The current batch-only `extract-progress` payload is replaced with a phased
payload:

```ts
interface ExtractionProgress {
  phase: 'resolving' | 'searching' | 'extracting' | 'relating'
       | 'embedding' | 'done' | 'empty';
  detail: string;            // human-readable, e.g. 'Found 12 passages'
  entities_found: number;    // running total
  relations_found: number;   // running total
}
```

Emitted by both backend commands via the existing Tauri event channel
(`extract-progress`). The frontend listens, matches by phase, and updates the
card in place.

## Backend

Two new Tauri commands in `extraction_commands.rs`, both emitting the phased
`extract-progress` event and both runnable as cancellable spawned tasks.

### `extract_entity_by_name(campaign_id, name)`

New `extraction_service::extract_seed_anchored`:

1. `agent_service::resolve_collection_ids(campaign)` → collection IDs linked to
   the active campaign.
2. Embed `name`; gather candidate passages by the **union** of:
   - `vector_store.search(query_vector, collection_ids, k)` (semantic), and
   - a SurrealQL text `CONTAINS` scan on `chunk.text` for `name` (lexical recall).
   Deduplicate the passage set.
3. LLM extraction with a **seed-anchored prompt**: "Build a complete profile of
   *{name}* and any entities directly related to it from this text."
4. Create / dedup entities **collection-scoped** to the collection each
   supporting chunk came from (reuses `entity_service::find_by_name_and_
   collection`), embed each new entity (ADR-003 pattern), and relate — the same
   persistence path used by the existing sweep.

If step 2 yields no passages, emit `phase: 'empty'` and return a zero result.

### `extract_all_from_campaign(campaign_id)`

Loops the existing `extraction_service::extract_from_collection` over every
linked collection, forwarding phase progress with per-collection detail. The
core sweep logic is reused unchanged.

### Cancellation (reuses the chat pattern)

The existing chat cancellation is the model: the chat task runs spawned, its
`AbortHandle` is stored in `state.chat_task`, and `cancel_chat_task` aborts it
at the next `.await`.

- App state gains `extract_task: Mutex<Option<AbortHandle>>` alongside
  `chat_task`.
- Both extraction commands spawn their work and register the abort handle.
- A new **`cancel_extraction`** command aborts at the next `.await`, mirroring
  `chat_cancel`.
- Partial results are intentionally preserved: extraction commits incrementally,
  and name+kind+collection dedup makes a re-run safe.

Scope: one state slot, one command, one button — all modeled on existing,
already-tested code.

## Data flow

```
User types "/extract Commander Varn"
  → OracleView.sendMessage → parseCommand → { kind: 'extract', name }
  → push ExtractionCard (role: 'extraction') into thread
  → invoke('extract_entity_by_name', { campaignId, name })
       backend spawns task, registers AbortHandle in state.extract_task
       emits extract-progress: resolving → searching → extracting
                               → relating → embedding → done
  → OracleView listens on 'extract-progress', updates card in place
  → command resolves with summary → card shows result + entity links
(Cancel button → invoke('cancel_extraction') → task aborts → card: "Cancelled…")
```

## Testing (ships with the feature)

**Rust**
- Unit: seed-anchored prompt builder contains the seed name and JSON schema.
- Integration: `extract_seed_anchored` round-trip with `MockLlm` /
  `MockEmbeddingProvider` — creates the seed entity + its relations, dedups on
  re-run, stays collection-scoped to the supporting chunk's collection.
- Integration: empty-passage path returns a zero result / `empty` phase.
- `cancel_extraction` aborts a registered task and empties the slot (mirrors the
  existing `cancel_chat_task` tests).

**Frontend**
- `parseCommand` table tests for every row in the command table, including
  whitespace and bare `/extract`.
- `ExtractionCard` renders each phase and every terminal state (success, empty,
  failure, cancelled); Cancel button visible only while running.
- `OracleView` routes `/extract …` to the card and **not** to `chatSend`; bare
  `/extract` shows the usage hint and starts no work.

## Removed

- The per-collection "Extract entities" button and its progress UI in
  `CampaignView.svelte`.
- The old batch-only `extract-progress` payload and
  `extractEntitiesFromCollection` frontend binding (superseded by the two new
  commands).

## Future work

- **Revisit `/extract-all`.** The broad, read-everything full sweep is retained
  for now to preserve the bulk capability, but its approach is not settled — we
  want to revisit it in a later phase and find something better (e.g. scoped /
  incremental / smarter-than-brute-force coverage) rather than re-extracting an
  entire collection every time. Treat the current `/extract-all` as a
  placeholder for that better design, not a final answer.
