# Defined Entity Kinds + Canonical Relationship Vocabulary

**Date:** 2026-06-24
**Status:** Approved (pending implementation plan)
**Area:** `src-tauri/src/services/extraction_service.rs`, `src-tauri/src/services/entity_service.rs`

## Problem

Two related quality issues in LLM entity extraction:

1. **Entity kinds are undefined in the prompts.** The extraction prompts list the
   8 kinds as bare names (`npc|location|faction|...`) with no definitions. The LLM
   therefore mis-classifies — e.g. a *family* (a house/clan, which is a group) gets
   labelled as an `npc` rather than recognised as a single `faction` entity.

2. **`rel_type` is a free-form string.** `LlmRelation::rel_type` is an unconstrained
   `String`, stored verbatim by `entity_service::relate`. There is no known set, so a
   later translation/i18n layer has no finite vocabulary to translate, and the same
   relationship can be expressed many ways.

## Goals

- Families and other organised groups classify as `faction`.
- `rel_type` is drawn from a known, finite, canonical set so translations key off a
  closed vocabulary.
- No relationships are silently dropped, including inverse-direction phrasings.
- No database migration (the `rel_type` column stays a string).

## Non-goals

- Building the translation/i18n layer itself (later stage).
- Adding new entity kinds — the existing 8 are sufficient once defined.
- Changing the `EntityKind` enum or any table schema.

## Part A — Entity-kind definitions (prompts only)

The 8 `EntityKind` variants stay unchanged. The two **classifying** prompts —
`build_extraction_prompt` and `build_seed_prompt` — gain a one-line definition per
kind where they currently list bare names. `build_profile_prompt` does not classify
kinds and is untouched.

Definitions:

- **npc** — a single named individual (person, named monster, deity).
- **location** — a place: region, settlement, building, room, plane.
- **faction** — any organised group of people: guild, cult, government, military
  order, crime ring, **noble house, family, or clan**.
- **creature** — a kind/species of being, not a named individual (e.g. "goblin",
  "owlbear").
- **item** — an object, artifact, weapon, or substance.
- **event** — something that happens at a point or span in time.
- **player_character** — a PC controlled by a player.
- **misc** — anything that fits none of the above.

The `faction` definition is the fix for the families bug. No new kind is introduced —
adding a `group` kind would make `faction` a redundant fuzzy sibling and *worsen*
classification consistency. One well-defined bucket beats two.

## Part B — Canonical relationship vocabulary

### `RelType` enum

A new `RelType` enum in `entity_service` carries the known vocabulary plus an
`Other(String)` catch-all. The DB `rel_type` column remains a string; "unknown" is
**derived** (`matches!(_, RelType::Other(_))`), so there is **no migration**.

```text
// Directional inverse pairs (canonical ⇄ inverse)
leads        ⇄ led_by
member_of    ⇄ has_member
located_in   ⇄ contains
owns         ⇄ owned_by
serves       ⇄ served_by
created      ⇄ created_by
parent_of    ⇄ child_of

// Symmetric (self-inverse)
allied_with
enemy_of
related_to
knows

// Catch-all
Other(String)
```

API surface:

- `RelType::from_str(&str) -> RelType` — parses LLM output; unrecognised → `Other(raw)`.
- `RelType::as_str(&self) -> Cow<str>` — stable snake_case key for known variants; raw
  verbatim for `Other`.
- `RelType::canonical(&self) -> (RelType, bool)` — returns the canonical variant and a
  `flip` flag. For an inverse member (e.g. `LedBy`) returns `(Leads, true)`; for a
  canonical or symmetric variant returns `(self, false)`.
- `RelType::is_canonical(&self) -> bool` — false for `Other`.

### Both directions exposed, canonical on store

The vocabulary exposes **both directions** of each directional relationship so the LLM
always has a fitting type for whichever direction the source sentence describes — `Other`
becomes a genuine last resort, not the landing spot for every inverse phrasing.

On persist, `persist_batch` normalises to the canonical direction:

1. Parse `rel.rel_type` → `RelType`.
2. Call `.canonical()` → `(canonical, flip)`.
3. If `flip`, swap `in`/`out` (the relation `B led_by A` is stored as `A -> leads -> B`).
4. Store `canonical.as_str()` as the edge's `rel_type`.

Net effect:

- **No dropout** — every direction has a named type.
- **No duplicate edges** — storage only ever holds the 7 canonical directional + 4
  symmetric keys (or `Other(raw)`).
- **Translation table stays minimal** — covers canonical keys only; inverse display
  labels are derived by the i18n layer if/when needed, never stored.

`entity_service::relate` keeps its string-based signature; normalisation happens at the
`persist_batch` boundary before calling it.

### Prompt change

In both `build_extraction_prompt` and `build_seed_prompt`, replace the free-form
`rel_type: "string (e.g. leads, commands, located_in, allied_with)"` with the
enumerated vocabulary, one-line definitions, and direction notes, plus the instruction:
*"Choose the closest type from this list, in whichever direction matches the sentence;
only if none fits, emit a short snake_case verb."*

### Frontend

No change required now. `EntityGraph.svelte:349` already renders the raw `rel_type`
string as the edge label, and `commands.ts` types it as a bare `string`. `Other(raw)`
therefore renders exactly as today (the status quo). The future i18n layer is a single
lookup at that render site, keyed on the now-closed canonical set, with the raw string
as the fallback.

## Testing

- **Prompt unit tests:** each kind definition appears in both classifying prompts; the
  canonical `rel_type` vocabulary appears in both classifying prompts.
- **`RelType` round-trip:** every known variant parses and serialises to a stable
  snake_case key; unknown input → `Other(raw)`; `is_canonical()` correct; `canonical()`
  returns the right `(variant, flip)` for canonical, inverse, and symmetric inputs.
- **`persist_batch` normalisation:** an inverse-direction input (`B led_by A`) produces
  a stored `A -> leads -> B` edge (flip verified); an off-list `rel_type` is stored
  verbatim as `Other`.

## Impact summary

- Two prompt edits (`build_extraction_prompt`, `build_seed_prompt`).
- One new `RelType` enum + normalisation call in `persist_batch`.
- All backend; **no schema migration**, no `EntityKind` change, no required frontend
  change.
