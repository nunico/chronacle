# Design — Phase 3 items 1 & 2: Session timeline view + Entity relationship graph

**Date:** 2026-06-22
**Status:** Approved (design); implementation plan pending
**Phase:** 3 — Polish & Power Features
**Roadmap:** `docs/architecture.md` → Phase 3 items "Session log timeline view" and "Entity relationship graph"

## Summary

Two visualisation features over data that already exists in the campaign graph:

1. **Timeline view** — a single view with two toggle modes over the campaign's
   `event` entities: **Chronicle** (in-world order by `sequence_index`, grouped by
   `era`) and **Sessions** (swimlanes by real play session).
2. **Entity relationship graph** — an **ego-first, expandable** graph of
   `relates_to` edges, centered on a chosen entity, rendered with `d3-force` for
   layout and Svelte/SVG for drawing.

Both are read-only over existing data. No schema migrations are required.

## Context — what already exists

- **Events** live in the `event` table (migration `004_graph_entities.surql`) with
  `sequence_index`, `era`, `date_start`, `date_end`, `is_ongoing`, `duration_label`,
  and a `session` record link. Per the data-model rules, `sequence_index` is the
  canonical ordering key; `date_*` are opaque strings and are **never parsed**.
- `entity_service::order_events_for_timeline` (`entity_service.rs:470`) sorts events
  by `sequence_index` with nulls last, tie-broken by name. It is already unit-tested.
  `get_events_timeline(campaign)` (`entity_service.rs:482`) wraps it. **Neither is
  exposed as a Tauri command yet.**
- **Edges** live in `relates_to` (directional `FROM ... TO ...`, with `rel_type` and
  optional `notes`). They are populated automatically by syncing `[[wikilinks]]` in
  entity notes (`entity_service.rs:377`) and by the manual `relate_entities` command
  (`entity_commands.rs:103`). There is **no read query** to fetch a subgraph yet.
- Frontend: `SessionLogView.svelte` lists sessions; `EntityManager.svelte` /
  `EntityForm.svelte` manage entities; `Shell.svelte` + `lib/shortcuts.ts` own the
  Vim-style `g`-chord navigation. No JS graph/viz dependency is installed.

## Non-goals

- No parsing of `date_start` / `date_end` — they render verbatim as labels only.
- No editing of events or relations from these views (open the entity to edit).
- No new entity or edge data; both features are read-only projections.
- Not a top-level "graph" nav entry (see Decision N1).

---

## Feature 1 — Timeline view

### Backend

- New Tauri command `get_events_timeline(campaign_id) -> Vec<GraphNode>` that calls
  the existing service fn. The returned events already carry `era`, `date_start/end`,
  `is_ongoing`, `duration_label`, `sequence_index`, and the `session` link — the full
  field set both modes need. Register in `lib.rs` alongside the other entity commands.
- **Sessions mode needs no new query**: the frontend joins these events against the
  existing `getSessions(campaign_id)` by the `session` link.

### Frontend — `TimelineView.svelte`

- Reachable as a **top-level campaign nav entry** with a `g`-chord (extend
  `lib/shortcuts.ts` + `Shell.svelte`, following the existing pattern; pick a free
  chord letter, e.g. `g t`).
- A mode toggle: **Chronicle** | **Sessions**. Mode is component-local state.
- **Chronicle mode:** vertical spine. Group events under `era` headers (in first-seen
  order along the sorted list). Each event renders name + a date/duration label
  (`date_start`–`date_end` or `duration_label`, verbatim; an "ongoing" marker when
  `is_ongoing`) + a session tag when linked. Events with `sequence_index = NULL` collect
  in a trailing **"Unordered"** group (mirrors the nulls-last service behaviour).
- **Sessions mode:** one lane per session in session order (as returned by
  `getSessions`), events nested under the session they link to; a final lane for events
  with no `session` link ("Unscheduled").
- Clicking an event opens it in the entity manager (reuse existing entity routing).
- Empty state when the campaign has no events.

---

## Feature 2 — Entity relationship graph

### Backend

- New service fn + Tauri command
  `get_entity_graph(entity_id, depth) -> EntityGraph` where
  `EntityGraph { nodes: Vec<GraphNodeRef>, edges: Vec<GraphEdge> }`,
  `GraphNodeRef { id, kind, name }`, `GraphEdge { from, to, rel_type, notes }`.
  - Fetch `relates_to` edges in **both** directions for `entity_id`, plus the neighbor
    nodes across all 8 entity tables. `depth` defaults to 1.
  - Dedupe nodes and edges (an entity may relate to the same neighbor via multiple
    edges; keep distinct edges but never duplicate a node).
  - Scope to the entity's campaign (neighbors are same-campaign by construction of the
    edges, but assert it).
- **Expand uses the same command**: calling `get_entity_graph(neighbor_id, 1)` and
  merging client-side grows the graph outward — no separate endpoint.

### Frontend — `EntityGraph.svelte`

- Opened **from an entity**: a "View graph" action in `EntityManager` / `EntityForm`.
  Centers on that entity. No top-level nav entry (Decision N1).
- `d3-force` (new dependency, layout only — headless force simulation) computes node
  positions; Svelte renders the SVG. Nodes are colored by entity kind (palette aligned
  with the entity manager / mockup colors). Edges are drawn directed (arrowhead
  `from → to`) and labelled by `rel_type`.
- Interactions (all in scope, none deferred):
  - **Click a node** → re-center: fetch its ego graph and reset the view to it.
  - **Expand affordance per node** → fetch that node's neighbors (depth 1) and merge
    into the current graph without recentering.
  - **Pan & zoom** the canvas.
  - **Drag to pin** a node (fix its simulation position; release to unpin or keep
    pinned — pinned by default after drag, matching common graph-tool behaviour).
  - **Click-through** to open an entity in the manager.
- Empty/lonely state when the centered entity has no relations.

---

## Decisions

- **N1 — Graph navigation:** graph is reachable only *from* an entity, not as a
  top-level nav entry. A top-level entry would require choosing a default entity to
  center on; "most-connected" is arbitrary from the user's perspective and not worth
  the surprise. In-graph re-center-on-click covers navigation once opened.
- **N2 — Timeline is a single view, two modes**, not two views — same data fetch,
  toggled rendering. Keeps it one nav destination and one data path.
- **N3 — `d3-force` dependency:** added for layout physics only; Svelte owns
  rendering so the look stays on-brand and themable. CLAUDE.md's "approved crates"
  rule governs Rust `Cargo.toml`; it does not gate frontend packages, so no ADR is
  required, but the addition is recorded here for the dependency-auditor.

## Testing

Per CLAUDE.md, tests ship with the feature.

- **Rust unit:** `order_events_for_timeline` is already covered. Add coverage for the
  graph query shaping: both-directions edge fetch, node/edge dedupe, campaign scoping —
  against an in-memory SurrealDB (`mem::Db`) with a seeded campaign.
- **Rust integration (`tests/`):** seed campaign → events with mixed/NULL
  `sequence_index` and `era` → assert `get_events_timeline` order and grouping inputs;
  seed entities + `relates_to` edges → assert `get_entity_graph` membership at depth 1
  and after a simulated expand.
- **Frontend (Vitest + `@testing-library/svelte`, `msw`-mocked backend):**
  - `TimelineView`: mode toggle; Chronicle era grouping + Unordered trailing group;
    Sessions lane grouping + Unscheduled lane; empty state.
  - `EntityGraph`: nodes/edges render from a mocked graph; re-center on node click;
    expand merges neighbors; empty state. (Force layout positions are non-deterministic —
    assert on node/edge presence and interactions, not coordinates.)

## Build order

1. **Timeline** first — backend is mostly done (expose the command); lower risk.
2. **Entity graph** second — new query + new dependency + richer interactions.

Each lands with its tests in the same change, following TDD.
