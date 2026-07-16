# Maintenance: scroll fix + naming-conflict resolution redesign

**Date:** 2026-07-16
**Status:** Approved (design)
**Area:** `apps/desktop` frontend (`MaintenanceView`), `chronacle-extraction` codex/lint service, Tauri command layer.

## Problem

Two issues in the Maintenance → Findings view:

1. **The findings list does not scroll.** `Shell.svelte`'s `<main class="main">` is
   `overflow: hidden` with `min-height: 0` by contract — every view is expected to be
   its own scroll container. `MaintenanceView`'s `.maintenance` root never opts in
   (`height`/`overflow` unset), so once findings exceed the viewport they are clipped
   and become unreachable. Resizing the window is the only way to see the rest.

2. **The "Naming conflict" (`alias_collision`) card is unusable.**
   - It renders raw SurrealDB record IDs (`entityRef(f.payload.a)?.id`), which look
     like opaque hashes — a GM cannot tell the two entities apart.
   - It offers no resolution mechanism. "Open A", "Open B", and a vague "Mark
     resolved" do not convey what is being resolved or let the GM actually settle the
     conflict.

   The `duplicate_entity` ("Possible duplicate") card has the same raw-ID display
   problem and is fixed in the same change.

## Background (current behaviour)

- `alias_collision` is emitted by `lint_alias_collisions` in
  `crates/chronacle-extraction/src/codex_service/lint.rs`. Two entities in the same
  resolution scope must never share a normalized name/alias key, or tier-2 wikilink
  resolution becomes non-deterministic. The finding payload is `{ alias, a, b }`
  where `alias` is the **normalized key** and `a`/`b` are full record strings
  (`kind:id`).
- Backend `naming::normalize` (in `crates/chronacle-extraction/src/naming.rs`) is
  nontrivial: lowercasing, possessive stripping, leading-"the" removal, and
  singularization. Replicating it in TypeScript would drift, so **name-vs-alias
  detection and alias removal must stay backend-side.**
- Existing primitives to reuse:
  - `entity_service::remove_alias(db, entity_id, alias)` — removes one alias.
  - `MergeDialog.svelte` — already used by the `duplicate_entity` card.
  - `resolveLintFinding(id)` — marks a finding resolved.
  - `getEntity(id, kind)` → `GraphNode` with `name`, `aliases`, `kind`, `summary`.

## Design

### Part 1 — Scrollable Maintenance view

CSS-only change to `MaintenanceView.svelte`'s `.maintenance` root: fill the parent
and own its overflow.

```css
.maintenance {
  height: 100%;
  overflow-y: auto;
  box-sizing: border-box;
  /* existing padding / flex column / gap unchanged */
}
```

No logic change. A regression guard in `MaintenanceView.test.ts` asserts the root
element carries a scroll affordance (`overflow-y: auto`), so the invariant — not just
the current markup — is protected.

### Part 2 — Read-time finding enrichment (both cards)

Names and validity flags are computed **at read time**, on the backend, in
`list_lint_findings` (`codex_service/lint.rs`). No schema migration; enrichment is
always fresh and never stored.

For each finding of kind `alias_collision` or `duplicate_entity`, resolve `a` and `b`
(parsing `kind:id` from the payload) and inject into the payload:

| Field | Kinds | Meaning |
|-------|-------|---------|
| `a_name`, `b_name` | both | primary display name |
| `a_kind`, `b_kind` | both | entity kind (for the type tag) |
| `a_summary`, `b_summary` | both | short snippet for disambiguation (may be null) |
| `a_is_name` | `alias_collision` | disputed term equals entity A's **primary name** (normalized) |
| `b_is_name` | `alias_collision` | disputed term equals entity B's **primary name** (normalized) |

Resolution rules:
- Name lookup uses a `SELECT name, aliases, summary FROM type::thing(kind, id)`
  helper that **excludes soft-deleted entities** (consistent with existing
  wikilink name resolution).
- If an entity is missing / soft-deleted, its `*_name` is left absent; the frontend
  falls back to the record ID and offers only Dismiss (+ Merge/Open where the other
  side still exists).
- `*_is_name` is computed with backend `naming::normalize`: the disputed key equals
  `normalize(entity.name)`.

Enrichment lives in a dedicated helper (e.g. `enrich_finding_display`) invoked from
`list_lint_findings`, so the shared list function stays readable and other kinds pass
through untouched.

### Part 3 — Naming-conflict card redesign

The `alias_collision` branch in `MaintenanceView.svelte` is rewritten:

**Layout**
- Heading: the disputed term (`f.payload.alias`) shown prominently.
- Two entity rows, each: **name · kind tag · snippet**, plus a tag showing whether
  this entity holds the term as its **name** or an **alias**.
- Fallback to the record ID when `*_name` is absent (deleted entity).

**Actions**

| Action | Shown when | Backend call |
|--------|-----------|--------------|
| **Keep on «A»** | B holds the term as an *alias* (`b_is_name` false) | `resolveAliasCollision(finding_id, keep=A, drop=B)` |
| **Keep on «B»** | A holds the term as an *alias* (`a_is_name` false) | `resolveAliasCollision(finding_id, keep=B, drop=A)` |
| **Merge…** | always | existing `MergeDialog` (`openMerge`) |
| **Open A / Open B** | always | existing `onOpenEntity` |
| **Dismiss** | always (renamed from "Mark resolved") | existing `resolveLintFinding` |

If **both** sides hold the term as their primary name (both `*_is_name` true — e.g.
two entities literally named "Legion"), neither Keep button appears; only Merge /
Open / Dismiss are offered. This is correct: a primary name cannot be stripped, so the
GM must merge, rename manually, or accept the two as distinct.

The `duplicate_entity` card reuses the same name/kind/snippet display (Part 2 fields);
its actions (Merge / Open A / Open B / Dismiss) are unchanged in behaviour, only the
labels now show real names.

### New backend command: `resolve_alias_collision`

`resolve_alias_collision(finding_id, keep_id, drop_id)` (Tauri command +
`codex_service`/`entity_service` logic):

1. Load `drop_id`; compute the disputed normalized key from the finding payload.
2. **Re-validate server-side** that the key is one of `drop_id`'s aliases and **not**
   its primary name. If it is the primary name → return an error (defence in depth;
   the UI already hides the button, but the command must not strip a name).
3. Find the original-cased alias on `drop_id` whose `normalize()` equals the key, and
   call `remove_alias(db, drop_id, that_alias)`.
4. `resolve_lint_finding(finding_id)`.

`keep_id` is accepted for clarity/validation (it must be the other party in the
finding) but requires no mutation — the term simply stays on it.

Frontend wrapper in `apps/desktop/src/lib/commands.ts`:
`resolveAliasCollision(findingId, keepId, dropId): Promise<void>`.

## Testing

**Rust (unit / integration, in-crate `#[cfg(test)]` + Tauri integration)**
- `resolve_alias_collision`: strips the disputed alias from the drop target and
  resolves the finding; the alias remains on the keep target.
- `resolve_alias_collision` **rejects** an attempt to strip the term from the side
  where it is the primary name (returns an error, mutates nothing).
- Enrichment: `list_lint_findings` injects `a_name`/`b_name`/`*_kind`/`*_is_name` for
  `alias_collision` and `duplicate_entity`; `*_is_name` is correct when the term is a
  name vs an alias; a soft-deleted party leaves the name absent (ID fallback).

**Frontend (Vitest + `@testing-library/svelte`, backend mocked)**
- Naming-conflict card renders real names, not record IDs.
- "Keep on «X»" is hidden for the side whose term is its primary name; both hidden
  when both are names.
- Merge / Dismiss / Open wired to the right calls.
- Scroll guard: `.maintenance` root exposes `overflow-y: auto`.

**Acceptance (BDD — ADR-011, mandatory for user-visible behaviour)**
- `.feature` scenario: a GM opens a naming conflict, assigns the term to one entity,
  and the finding leaves the inbox with the alias removed from the other entity.

## Out of scope

- No change to `alias_collision` detection logic or when findings are emitted.
- No change to the `stale_article`, `broken_wikilink`, `scope_violation`,
  `orphaned_edge`, or `auto_alias` cards.
- No new dependencies.
