# Unresolved Wikilink Create-Article Flow

**Date:** 2026-07-17
**Status:** Approved (design)
**Area:** `apps/desktop` frontend (`WikiText`, `EntityManager`, `MaintenanceView`),
`chronacle-extraction` wikilink/lint services, Tauri command layer.

## Problem

Chronacle currently treats every unresolved `[[wikilink]]` as a broken link. That
collapses two different situations:

1. **Possible name mismatch.** The target article/entity probably already exists,
   but the link text does not exactly match its name or confirmed aliases. This
   already has suggestions through the `broken_wikilink.payload.candidates` list.
2. **Missing article.** The link is an intentional forward reference to an article
   the GM has not created yet. This is a valid writing workflow, not an error.

The current UI only handles the first case well. Maintenance labels both cases
as "Broken wikilink", and the article renderer (`WikiText.svelte`) renders
unmatched links as inert literal text. The GM cannot click `[[Moon Gate]]` and
quickly create the missing target article.

There is also a related correctness gap: backend wikilink resolution honors names,
aliases, and normalized name matches, while `WikiText.svelte` only receives a
frontend map keyed by primary names and performs case-insensitive exact matching.
A link can therefore be backend-valid but still look unresolved in the article UI.

## Goals

- Let a GM click an unresolved wikilink in an article or notes preview and create
  the missing article/entity with the link text prefilled as the name.
- When a suggestion exists, offer both valid resolutions: accept the suggestion or
  create a new article anyway.
- Update Maintenance so missing-article findings no longer read as errors.
- Reuse the existing `broken_wikilink` lint kind; distinguish the UI state from
  `payload.candidates.length`.
- Keep scope and alias behavior aligned with the existing wikilink resolver.

## Non-goals

- Do not introduce a new lint kind or database migration.
- Do not change fuzzy matching thresholds or candidate ranking.
- Do not auto-create entities from lint or article rendering. Creation is always
  user-confirmed through the existing entity form.
- Do not add an LLM classifier to guess the target entity kind.

## Design

### Finding semantics

The stored lint kind remains `broken_wikilink`. The frontend renders it according
to its payload:

| Payload state             | User-facing label      | Meaning                                                                  |
| ------------------------- | ---------------------- | ------------------------------------------------------------------------ |
| `candidates.length > 0`   | Possible name mismatch | The link might refer to an existing article under another name.          |
| `candidates.length === 0` | Missing article        | No plausible existing target was found; this may be a forward reference. |

The payload shape is unchanged:

```json
{
  "entity": "npc:mira",
  "entity_name": "Mira",
  "link_text": "Moon Gate",
  "candidates": []
}
```

`list_lint_findings` should continue enriching `entity_name` for single-entity
findings. Existing idempotence and deduplication rules stay in place.

### Shared create-from-wikilink flow

Both article clicks and Maintenance use the same user flow:

1. User activates an unresolved wikilink.
2. Chronacle opens a compact "Create article" chooser.
3. The link text is prefilled as the new entity `name`.
4. The user chooses an entity kind (`NPC`, `Location`, `Faction`, `Creature`,
   `Item`, `Event`, `PC`, `Misc`).
5. `Shell.svelte` routes to the matching notebook category and opens the
   existing `EntityForm` in create mode with that kind and prefilled name.
6. On save, the normal `createEntity(campaignId, kind, input)` path creates the
   entity and runs existing inbound wikilink sync for the new entity.

The chooser does not guess kind from text. The current entity kind may be
highlighted as the default selection, but the full choice set remains visible.
The shell carries this as a pending create request:

```ts
type PendingCreate = {
  kind: EntityKind;
  name: string;
  sourceFindingId?: string;
};
```

`EntityManager` receives `pendingCreate` when its category matches
`PendingCreate.kind`, consumes it once, and preloads the create form with
`EntityInput.name = pendingCreate.name`.

### Article and notes rendering

`WikiText.svelte` gains an unresolved-link callback:

```ts
onMissingLinkClick?: (name: string) => void;
```

Matched links keep their current behavior:

```ts
onEntityClick?.(id, entityKind);
```

Unmatched links become buttons styled differently from resolved entity badges.
They preserve the visible wikilink text, but are no longer inert:

- accessible name: `Create article for Moon Gate`
- click: `onMissingLinkClick?.('Moon Gate')`
- if no callback is provided, render as non-interactive text as today

`EntityManager.svelte` passes `onMissingLinkClick` to both notes preview and
codex article `WikiText` instances. It owns the chooser and existing create form
handoff because it already owns `kind`, `campaignId`, `openCreate`, `handleSave`,
and `entityMap`.

### Frontend resolution map

`EntityManager.buildEntityMap()` indexes each entity by:

- primary `node.name`
- every `node.aliases[]`
- a normalized frontend key compatible with backend `naming::normalize`

This uses a new shared frontend utility in `apps/desktop/src/lib/wikilinks.ts`
that exports:

```ts
normalizeWikiLinkKey(name: string): string;
buildWikiLinkEntityMap(nodes: GraphNode[]): Map<string, { id: string; kind: string }>;
```

The utility mirrors the backend rules already documented in the Entity Identity
spec: trim/case-fold, leading `the`, possessive stripping, conservative plural
singularization, and whitespace / punctuation collapse. It is isolated and
tested so any later backend resolver-preview command can replace it cleanly.

Collision behavior must remain deterministic and conservative: if two entities
claim the same frontend key, do not pick either for article rendering. Leave the
link unresolved so the GM can resolve the underlying alias collision in
Maintenance.

### Maintenance card behavior

The `broken_wikilink` branch in `MaintenanceView.svelte` is redesigned.

For `candidates.length > 0`:

- Heading/group label: `Possible name mismatch`
- Detail: `[[Moon Gate]] in Mira`
- Suggestion line: `Did you mean Moon Gate of Elturel?`
- Actions:
  - `Use suggestion` -> existing `confirmAliasSuggestion(candidate.id, link_text)`,
    then `resolveLintFinding(f.id)`, refresh counts
  - `Create article` -> shared create-from-wikilink flow with name `link_text`
  - `Open source` -> existing `onOpenEntity` for `payload.entity`
  - `Dismiss` -> existing `resolveLintFinding(f.id)`

For `candidates.length === 0`:

- Heading/group label: `Missing article`
- Detail: `[[Moon Gate]] in Mira`
- Helper copy: `Create it now or leave it as a forward reference.`
- Actions:
  - `Create article` -> shared create-from-wikilink flow with name `link_text`
  - `Open source` -> existing `onOpenEntity` for `payload.entity`
  - `Dismiss` -> existing `resolveLintFinding(f.id)`

When the user creates from a Maintenance finding, `MaintenanceView` passes the
finding id through `PendingCreate.sourceFindingId`. When creation succeeds,
`EntityManager` calls an optional `onPendingCreateSaved(sourceFindingId)` prop;
`Shell.svelte` resolves the lint finding and refreshes Maintenance counts. This
is correct because creating the target article satisfies the unresolved
reference. If save fails with a validation error such as a name collision, keep
the finding unresolved and show the normal form error.

### Backend and command contract

No new backend command or lint kind is needed for the first implementation.
Creation uses the existing `createEntity(campaignId, kind, input)` Tauri command.
Resolving a Maintenance finding after successful creation uses the existing
`resolveLintFinding(id)` command.

The backend behavior that matters is already present: creating a new entity runs
the existing inbound wikilink sync path, so prior forward references can become
`mentioned` relations without a manual re-save of the source article.

### Error handling

- Create save errors use existing `EntityForm` validation handling.
- Alias suggestion confirmation keeps existing behavior: if
  `confirmAliasSuggestion` fails because the alias now collides, show the error
  and keep the finding unresolved.
- If a Maintenance finding references a deleted source entity, disable `Open
source` and still allow `Create article` and `Dismiss`.
- If a missing link is clicked while another create/edit form is dirty, prompt
  before replacing the form state. The first implementation can use the existing
  form's cancel/close behavior if no dirty-state tracking exists, but it must not
  silently discard typed form fields.

## Testing

### Frontend unit tests

- `WikiText` renders unmatched links as clickable buttons when
  `onMissingLinkClick` is provided and inert text when it is not.
- `WikiText` calls `onMissingLinkClick('Moon Gate')` for `[[Moon Gate]]`.
- `WikiText` resolves links through aliases and normalized keys supplied by the
  entity map.
- `EntityManager` opens the create chooser/form from an unresolved article link
  with the name prefilled.
- `MaintenanceView` renders candidate-backed findings as `Possible name mismatch`
  with `Use suggestion` and `Create article`.
- `MaintenanceView` renders no-candidate findings as `Missing article` with
  `Create article`, not `Use suggestion`.
- Creating from a Maintenance finding resolves the finding only after
  `createEntity` succeeds.

### Backend tests

- Creating the target entity triggers the existing inbound wikilink sync path so
  previous forward references become `mentioned` relations.
- Existing lint behavior still records a no-candidate `broken_wikilink` payload
  with an empty `candidates` array.

### Acceptance Scenarios

Add Gherkin scenarios under `apps/desktop/tests/e2e/features/`:

```gherkin
Scenario: Create a missing article from a clicked wikilink
  Given an NPC article contains the unresolved link "[[Moon Gate]]"
  When the GM clicks "[[Moon Gate]]"
  And creates a Location named "Moon Gate"
  Then the article link resolves to the new Location
  And the relationship graph includes a mentioned edge to "Moon Gate"

Scenario: Choose between a suggestion and a new article in Maintenance
  Given Maintenance has a wikilink finding for "[[Moon Gat]]" with a suggestion "Moon Gate"
  When the GM opens the finding
  Then they can use the suggestion
  And they can instead create a new article named "Moon Gat"

Scenario: Treat no-candidate wikilinks as missing articles
  Given Maintenance has a wikilink finding for "[[Ashen Ferry]]" with no candidates
  When the GM opens the finding
  Then the finding is labeled "Missing article"
  And the primary action is "Create article"
```
