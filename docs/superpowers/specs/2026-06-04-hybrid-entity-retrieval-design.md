# Hybrid Entity Retrieval for the Oracle — Design Spec

**Goal:** Inject campaign entity records (NPCs, PCs, locations, etc.) into the Oracle's LLM context so that queries like "who are Nico's characters?" return correct answers from the GM's own notes alongside PDF-sourced rules.

**Architecture:** At query time, fetch all entities for the active campaign from SurrealDB and inject them as a structured `CAMPAIGN NOTES:` block in the system prompt. No embedding at write time. The existing PDF vector-search path is unchanged. Entity-derived LLM claims are cited with `[Entity: "name", kind: "kind"]` badges rendered inline.

**Tech Stack:** Rust (`agent_service.rs`), SurrealDB multi-statement query, TypeScript (`ruling-parse.ts`), Svelte 5 (`OracleView.svelte`), Vitest.

---

## Backend — `agent_service.rs`

### New function: `fetch_entity_context`

```rust
pub async fn fetch_entity_context<C: Connection>(
    db: &surrealdb::Surreal<C>,
    campaign_id: &str,
) -> Result<String, AgentError>
```

Queries all 8 entity tables in a single round-trip using chained `.query()` calls, each bound to the same `campaign_id`:

```sql
SELECT name, summary FROM npc WHERE campaign = type::thing('campaign', $cid) ORDER BY name ASC
SELECT name, summary FROM location WHERE ...
SELECT name, summary FROM faction WHERE ...
SELECT name, summary FROM creature WHERE ...
SELECT name, summary FROM item WHERE ...
SELECT name, summary, date_start, date_end, is_ongoing FROM event WHERE ...
SELECT name, summary, player_name, character_class, character_level, status FROM player_character WHERE ...
SELECT name, summary FROM misc WHERE ...
```

Results are taken from indices 0–7. The function returns an empty `String` if all tables are empty (caller uses this to skip the section).

**Output format:**

```
Campaign notes (your GM records):

[player_character] Nazirdijan · Player: Nico · Class: Wizard · Level: 5 · Status: active
[player_character] Torben Ashveil · Player: Jan · Class: Barbarian · Level: 5

[npc] Aldric the Smith · village blacksmith with a secret
[npc] Lady Mirova Stane

[location] Blackrock Keep · fortress on the northern cliffs

[faction] The Iron Circle · mercenary company allied with the Baron

[event] Fall of the Keep · 4th Age, Year 312 → 313
```

Rules:
- Each entity is one line: `[kind] Name · summary` (summary omitted when null/empty).
- `player_character` additionally shows `Player:`, `Class:`, `Level:`, `Status:` for non-null fields only.
- `event` additionally shows dates when non-null (`date_start → date_end` or just `date_start`).
- Sections are omitted when empty.
- Entities within a section are ordered by name (ORDER BY name ASC from the query).

### Updated function: `build_system_prompt`

Replaces `build_rag_system_prompt(context: &str)` with:

```rust
fn build_system_prompt(rag_context: &str, entity_context: &str) -> String
```

The function has four cases:

| rag_context | entity_context | behaviour |
|-------------|----------------|-----------|
| empty | empty | Fallback: "Answer to best of your ability; say if unknown" |
| non-empty | empty | Existing behaviour — REFERENCE MATERIAL section only |
| empty | non-empty | CAMPAIGN NOTES section only, no RAG instructions |
| non-empty | non-empty | Both sections |

**Added prompt instructions (when entity_context is non-empty):**

```
CAMPAIGN NOTES (GM's own records):
<entity_context>

INSTRUCTIONS (additions to the existing list):
- Facts derived from CAMPAIGN NOTES must cite with: [Entity: "<name>", kind: "<kind>"]
  where kind is the bracketed prefix on that line (e.g. player_character, npc, location).
  No verbatim quote is needed — entity records are the GM's own data.
  Example: [Entity: "Nazirdijan", kind: "player_character"]
- Facts derived from REFERENCE MATERIAL still require [Source: "...", p.X, quote: "..."].
- When a fact appears in both, prefer the PDF citation.
- Entity names in CAMPAIGN NOTES are exact — use them verbatim in citations.
```

### Updated: `stream_response`

After `resolve_collection_ids`, call:

```rust
let entity_context = match campaign_id {
    Some(cid) => fetch_entity_context(&state.db, cid).await
        .unwrap_or_else(|e| { eprintln!("entity context fetch failed: {e}"); String::new() }),
    None => String::new(),
};
```

Pass both `context` (existing RAG) and `entity_context` to `build_system_prompt`.

Entity context fetch failures are logged but do not abort the pipeline — the Oracle falls back to PDF-only context.

---

## Frontend — `ruling-parse.ts`

### Updated: `renderContent`

Add a second regex alongside `SOURCE_RE`:

```typescript
const ENTITY_RE = /\[Entity:\s*"([^"]+)",\s*kind:\s*"([^"]+)"\s*\]/g;
```

Apply it after `SOURCE_RE` replacement:

```typescript
.replace(ENTITY_RE, (_, name: string, kind: string) =>
  `<span class="entity-badge" title="${escapeAttr(kind)}">${escapeAttr(name)}</span>`
)
```

Entity badges are `<span>` (not `<button>`) — no click handler needed.

No changes to `hasCitation`, `parseRuling`, or `findVerdictBoundary`. Responses with only entity citations render via the plain `{@html plainHtml(...)}` path, which already calls `renderContent`. Responses with both PDF and entity citations render via `RulingCard`, with entity badges appearing inside the `why` HTML block.

### Updated: `OracleView.svelte`

Add a global style rule for `.entity-badge`:

```css
:global(.entity-badge) {
  display: inline-flex;
  align-items: baseline;
  padding: 1px 8px;
  border-radius: var(--r-full);
  border: 1px solid var(--line);
  color: var(--violet-300);
  background: rgba(184, 166, 255, 0.08);
  font-family: var(--font-mono);
  font-size: 12px;
  margin: 0 2px;
}
```

Violet colour (`--violet-300`) distinguishes entity badges from PDF citation badges (arcane blue, `--arcane-300`).

---

## Files changed

| File | Change |
|------|--------|
| `src-tauri/src/services/agent_service.rs` | Add `fetch_entity_context`; rename + update `build_rag_system_prompt` → `build_system_prompt`; update `stream_response` |
| `src/views/ruling-parse.ts` | Add `ENTITY_RE`; update `renderContent` |
| `src/views/OracleView.svelte` | Add `:global(.entity-badge)` style |

No schema changes. No new crates. No new Tauri commands. No changes to `message.citations` persistence (entity badge rendering happens at display time from the raw stored content).

---

## Testing

### Rust unit tests (in `agent_service.rs`)

- `fetch_entity_context_empty_campaign` — returns empty string when no entities exist for a campaign.
- `fetch_entity_context_player_character` — creates a `player_character` record, verifies the output contains `[player_character]`, the entity name, and the `Player:` field.
- `fetch_entity_context_omits_empty_sections` — creates only an NPC; verifies the output does NOT contain `[player_character]` or `[location]` section headers.
- `fetch_entity_context_event_dates` — creates an event with `date_start`/`date_end`; verifies dates appear in the output.
- `build_system_prompt_both_contexts` — verifies both `REFERENCE MATERIAL` and `CAMPAIGN NOTES` sections appear.
- `build_system_prompt_entity_only` — verifies `CAMPAIGN NOTES` appears and `REFERENCE MATERIAL` does not when RAG context is empty.
- `build_system_prompt_rag_only` — verifies existing behaviour unchanged when entity context is empty (regression).
- `build_system_prompt_neither` — verifies fallback prompt when both are empty.

### Vitest frontend tests (in `ruling-parse.test.ts`)

- `renderContent replaces [Entity] with entity-badge span` — verifies `<span class="entity-badge">` is present and the name/kind are rendered correctly.
- `renderContent escapes malicious entity name` — `[Entity: "<script>", kind: "npc"]` must not produce raw `<script>`.
- `renderContent handles both Source and Entity markers in one string` — verifies both badge types render in a single `renderContent` call.

---

## Verification

1. `cargo test` — all unit + integration tests pass.
2. `pnpm test --run` — Vitest tests pass.
3. `cargo tauri dev` — create a player character "Nazirdijan" with player name "Nico", then ask the Oracle "who are Nico's characters?" — verify the answer includes the entity badge.
4. Ask a PDF question — verify `[Source: ...]` citations are unaffected.
5. Ask a mixed question (e.g. "what class is Nazirdijan and what does that class do?") — verify both entity and source badges appear.
6. Verify entity fetch failure is non-fatal: with `CHRONACLE_RAG_DEBUG=1`, a log line appears but the Oracle still responds.
