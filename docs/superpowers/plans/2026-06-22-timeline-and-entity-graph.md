# Timeline View + Entity Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a campaign **timeline view** (Chronicle + Sessions toggle modes) and an **ego-first, expandable entity relationship graph**, both read-only over existing data.

**Architecture:** The timeline reuses the already-tested `order_events_for_timeline` service fn, newly exposed as a Tauri command; the frontend renders two modes from one fetch (events + sessions). The graph adds one new backend query that reads `relates_to` edges in both directions plus neighbor node names, and a Svelte component that lays out nodes with `d3-force` and draws them as SVG.

**Tech Stack:** Rust + SurrealDB (embedded), Tauri IPC, Svelte 5 (runes), Vitest + `@testing-library/svelte` + `msw`, new frontend dep `d3-force`.

**Spec:** `docs/superpowers/specs/2026-06-22-timeline-and-entity-graph-design.md`

**Conventions in this codebase (read before starting):**
- Tauri commands live in `src-tauri/src/commands/*.rs`, registered in `src-tauri/src/lib.rs` `invoke_handler!`. Frontend wrappers live in `src/lib/commands.ts`.
- Service fns are `pub async fn f<C: surrealdb::Connection>(db: &Surreal<C>, …)`. Integration tests use an in-memory `mem::Db` with schema applied; see `src-tauri/tests/entity_service_test.rs` for the harness pattern.
- Components mock the backend with `msw`; see `src/components/EntityForm.test.ts`.
- Run Rust tests: `cargo test`. Run a single Rust test: `cargo test -- --nocapture <name>`. Run frontend tests: `pnpm test --run`. Lint gates: `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check`, `pnpm typecheck`, `pnpm lint`.

---

## File Structure

**Feature 1 — Timeline:**
- Modify `src-tauri/src/commands/entity_commands.rs` — add `get_events_timeline` command.
- Modify `src-tauri/src/lib.rs` — register the command.
- Modify `src/lib/commands.ts` — add `getEventsTimeline` wrapper.
- Create `src/views/TimelineView.svelte` — the view, both modes, mode toggle.
- Create `src/views/TimelineView.test.ts` — component tests.
- Modify `src/lib/shortcuts.ts` — add `t` → timeline chord + help row.
- Modify `src/lib/shortcuts.test.ts` — cover the new chord.
- Modify `src/shell/CampaignRail.svelte` — `View` type + Timeline rail button.
- Modify `src/shell/Shell.svelte` — route the `'timeline'` view + `navTo`.

**Feature 2 — Entity graph:**
- Modify `src-tauri/src/services/entity_service.rs` — `EntityGraph`/`GraphNodeRef`/`GraphEdge` types + `get_entity_graph` fn + unit tests.
- Modify `src-tauri/tests/entity_service_test.rs` — integration test for `get_entity_graph`.
- Modify `src-tauri/src/commands/entity_commands.rs` — `get_entity_graph` command.
- Modify `src-tauri/src/lib.rs` — register it.
- Modify `package.json` — add `d3-force` + `@types/d3-force`.
- Modify `src/lib/commands.ts` — `getEntityGraph` wrapper + TS types.
- Create `src/lib/graph-merge.ts` — pure node/edge merge helper (unit-testable).
- Create `src/lib/graph-merge.test.ts` — merge unit tests.
- Create `src/components/EntityGraph.svelte` — the graph component.
- Create `src/components/EntityGraph.test.ts` — component tests.
- Modify `src/components/EntityManager.svelte` — per-row "View graph" affordance (the entity list is the natural entry point; opening the graph mid-edit from `EntityForm` is intentionally out of scope).
- Modify `src/shell/Shell.svelte` — open the graph overlay from an entity.

---

# Feature 1 — Timeline View

## Task 1: Expose `get_events_timeline` as a command + frontend wrapper

**Files:**
- Modify: `src-tauri/src/commands/entity_commands.rs`
- Modify: `src-tauri/src/lib.rs:247` (the `relate_entities` registration line — add adjacent)
- Modify: `src/lib/commands.ts`

- [ ] **Step 1: Add the command** in `src-tauri/src/commands/entity_commands.rs`, after `get_entities` (around line 44):

```rust
/// Campaign events in canonical timeline order (`sequence_index`, nulls last).
#[tauri::command]
pub async fn get_events_timeline(
    state: State<'_, Arc<AppState>>,
    campaign_id: String,
) -> Result<Vec<GraphNode>, EntityError> {
    entity_service::get_events_timeline(&state.db, &campaign_id).await
}
```

- [ ] **Step 2: Register it** in `src-tauri/src/lib.rs`. Find the line `commands::relate_entities,` (around line 247) and add directly below it:

```rust
            commands::get_events_timeline,
```

- [ ] **Step 3: Confirm the command is re-exported.** Check `src-tauri/src/commands/mod.rs` — if it uses an explicit `pub use entity_commands::{…}` list, add `get_events_timeline` to it. (If it does `pub use entity_commands::*;`, no change needed.)

Run: `grep -n "relate_entities\|get_entities" src-tauri/src/commands/mod.rs`
If `relate_entities` is listed explicitly, add `get_events_timeline` to the same list.

- [ ] **Step 4: Build the backend** to verify wiring.

Run: `cargo build`
Expected: compiles with no errors.

- [ ] **Step 5: Add the frontend wrapper** in `src/lib/commands.ts`, directly after `getEntities` (around line 443):

```ts
/** Campaign events in canonical timeline order (sequence_index, nulls last). */
export async function getEventsTimeline(campaignId: string): Promise<GraphNode[]> {
  return invoke<GraphNode[]>('get_events_timeline', { campaignId });
}
```

- [ ] **Step 6: Typecheck.**

Run: `pnpm typecheck`
Expected: no errors.

- [ ] **Step 7: Commit.**

```bash
git add src-tauri/src/commands/entity_commands.rs src-tauri/src/lib.rs src-tauri/src/commands/mod.rs src/lib/commands.ts
git commit -m "feat(timeline): expose get_events_timeline command + wrapper"
```

---

## Task 2: TimelineView — Chronicle mode

Chronicle mode groups timeline-ordered events under `era` headers, with a trailing "Unordered" group for `sequence_index === null` events. Because grouping logic should be unit-testable without a DOM, extract it into a pure helper first.

**Files:**
- Create: `src/lib/timeline-groups.ts`
- Create: `src/lib/timeline-groups.test.ts`
- Create: `src/views/TimelineView.svelte`
- Create: `src/views/TimelineView.test.ts`

- [ ] **Step 1: Write the failing test** for the grouping helper in `src/lib/timeline-groups.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { groupByEra } from './timeline-groups';
import type { GraphNode } from './commands';

function ev(name: string, sequence_index: number | null, era: string | null): GraphNode {
  return {
    id: name, kind: 'event', campaign_id: null, name,
    summary: null, notes: null, created_at: null, updated_at: null,
    date_start: null, date_end: null, is_ongoing: null,
    sequence_index, era, duration_label: null, session_id: null,
    player_name: null, character_class: null, character_level: null, status: null,
  };
}

describe('groupByEra', () => {
  it('groups consecutive events by era in first-seen order, nulls last', () => {
    // Caller passes events already in timeline order.
    const ordered = [
      ev('A', 1, 'Dawn'),
      ev('B', 2, 'Dawn'),
      ev('C', 3, 'Dusk'),
      ev('D', null, 'Dusk'),
      ev('E', null, null),
    ];
    const groups = groupByEra(ordered);
    expect(groups.map((g) => g.era)).toEqual(['Dawn', 'Dusk', null]);
    expect(groups[0].events.map((e) => e.name)).toEqual(['A', 'B']);
    expect(groups[1].events.map((e) => e.name)).toEqual(['C', 'D']);
    expect(groups[2].events.map((e) => e.name)).toEqual(['E']);
  });

  it('returns an empty array for no events', () => {
    expect(groupByEra([])).toEqual([]);
  });
});
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `pnpm test --run src/lib/timeline-groups.test.ts`
Expected: FAIL — cannot find module `./timeline-groups`.

- [ ] **Step 3: Implement the helper** in `src/lib/timeline-groups.ts`:

```ts
import type { GraphNode } from './commands';

export interface EraGroup {
  era: string | null; // null = the "Unordered / no era" trailing bucket
  events: GraphNode[];
}

/**
 * Group already-timeline-ordered events into runs by `era`, preserving order.
 * Events keep the order the backend returned (sequence_index, nulls last); this
 * only buckets consecutive same-era events. Events with `era == null` form a
 * trailing `era: null` group.
 */
export function groupByEra(ordered: GraphNode[]): EraGroup[] {
  const groups: EraGroup[] = [];
  for (const e of ordered) {
    const era = e.era ?? null;
    const last = groups[groups.length - 1];
    if (last && last.era === era) last.events.push(e);
    else groups.push({ era, events: [e] });
  }
  return groups;
}
```

- [ ] **Step 4: Run it to verify it passes.**

Run: `pnpm test --run src/lib/timeline-groups.test.ts`
Expected: PASS (both tests).

- [ ] **Step 5: Create the view** `src/views/TimelineView.svelte` with Chronicle mode only (Sessions mode added in Task 3). The `mode` toggle markup is included now so Task 3 only adds the Sessions branch:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { getEventsTimeline, type GraphNode } from '../lib/commands';
  import { groupByEra, type EraGroup } from '../lib/timeline-groups';

  interface Props {
    campaignId: string;
    onOpenEntity?: (e: GraphNode) => void; // deep-link: open this event in its notebook
  }
  const { campaignId, onOpenEntity }: Props = $props();

  let mode = $state<'chronicle' | 'sessions'>('chronicle');
  let events = $state<GraphNode[]>([]);
  let loading = $state(true);

  const eraGroups = $derived<EraGroup[]>(groupByEra(events));

  onMount(load);

  async function load() {
    loading = true;
    try {
      events = await getEventsTimeline(campaignId);
    } catch (e) {
      console.error('Failed to load timeline:', e);
      events = [];
    } finally {
      loading = false;
    }
  }

  function dateLabel(e: GraphNode): string {
    if (e.duration_label) return e.duration_label;
    if (e.date_start && e.date_end) return `${e.date_start} – ${e.date_end}`;
    return e.date_start ?? e.date_end ?? '';
  }
</script>

<div class="timeline">
  <div class="toolbar" role="tablist" aria-label="Timeline mode">
    <button role="tab" aria-selected={mode === 'chronicle'} class:active={mode === 'chronicle'}
      onclick={() => (mode = 'chronicle')}>Chronicle</button>
    <button role="tab" aria-selected={mode === 'sessions'} class:active={mode === 'sessions'}
      onclick={() => (mode = 'sessions')}>Sessions</button>
  </div>

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if events.length === 0}
    <p class="muted">No events yet. Add events in the Events notebook to build your timeline.</p>
  {:else if mode === 'chronicle'}
    <ol class="spine">
      {#each eraGroups as group (group.era ?? '__none__')}
        <li class="era">
          <h3 class="era-head">{group.era ?? 'Unordered'}</h3>
          <ol class="events">
            {#each group.events as e (e.id)}
              <li class="event">
                <button class="name" onclick={() => onOpenEntity?.(e)}>{e.name}</button>
                {#if dateLabel(e)}<span class="when">{dateLabel(e)}</span>{/if}
                {#if e.is_ongoing}<span class="ongoing">ongoing</span>{/if}
              </li>
            {/each}
          </ol>
        </li>
      {/each}
    </ol>
  {/if}
</div>

<style>
  .timeline { padding: 20px; }
  .toolbar { display: flex; gap: 6px; margin-bottom: 16px; }
  .toolbar button { padding: 6px 14px; border-radius: var(--r-md); background: var(--bg-panel); border: 1px solid var(--line); color: var(--fg-2); cursor: pointer; }
  .toolbar button.active { color: var(--fg-1); border-color: var(--violet-400); }
  .spine { list-style: none; margin: 0; padding: 0; }
  .era-head { font-family: var(--font-display); color: var(--violet-400); margin: 18px 0 8px; }
  .events { list-style: none; margin: 0; padding-left: 16px; border-left: 2px solid var(--line); }
  .event { padding: 6px 0; display: flex; gap: 10px; align-items: baseline; }
  .event .name { background: none; border: none; padding: 0; color: var(--fg-1); cursor: pointer; font: inherit; text-align: left; }
  .event .name:hover { color: var(--violet-400); }
  .event .when { color: var(--fg-3); font-size: 12px; }
  .event .ongoing { color: var(--violet-400); font-size: 11px; text-transform: uppercase; }
  .muted { color: var(--fg-3); }
</style>
```

- [ ] **Step 6: Write the component test** `src/views/TimelineView.test.ts`. Mock the Tauri `invoke` so the component sees fixture events (follow the mocking style already used in `src/components/EntityForm.test.ts` — check whether it mocks `@tauri-apps/api/core` `invoke` or uses `msw`, and match it). Example using a direct `invoke` mock:

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import TimelineView from './TimelineView.svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

function ev(name: string, sequence_index: number | null, era: string | null) {
  return {
    id: name, kind: 'event', campaign_id: null, name, summary: null, notes: null,
    created_at: null, updated_at: null, date_start: null, date_end: null,
    is_ongoing: false, sequence_index, era, duration_label: null, session_id: null,
    player_name: null, character_class: null, character_level: null, status: null,
  };
}

beforeEach(() => invoke.mockReset());

describe('TimelineView — chronicle mode', () => {
  it('renders era headers and events in order, with an Unordered group last', async () => {
    invoke.mockResolvedValueOnce([ev('Siege', 1, 'Dawn'), ev('Pact', 2, 'Dawn'), ev('Lost', null, null)]);
    render(TimelineView, { campaignId: 'c1' });
    expect(await screen.findByText('Siege')).toBeInTheDocument();
    expect(screen.getByText('Dawn')).toBeInTheDocument();
    expect(screen.getByText('Unordered')).toBeInTheDocument();
    expect(screen.getByText('Lost')).toBeInTheDocument();
  });

  it('shows an empty state when there are no events', async () => {
    invoke.mockResolvedValueOnce([]);
    render(TimelineView, { campaignId: 'c1' });
    expect(await screen.findByText(/No events yet/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 7: Run the tests.**

Run: `pnpm test --run src/views/TimelineView.test.ts src/lib/timeline-groups.test.ts`
Expected: PASS. If the mock style is wrong, adjust to match `EntityForm.test.ts` exactly, then re-run.

- [ ] **Step 8: Commit.**

```bash
git add src/lib/timeline-groups.ts src/lib/timeline-groups.test.ts src/views/TimelineView.svelte src/views/TimelineView.test.ts
git commit -m "feat(timeline): TimelineView chronicle mode + era grouping"
```

---

## Task 3: TimelineView — Sessions mode

Sessions mode lays out one lane per session (in the order `getSessions` returns), with events nested under the session they link to via `session_id`, plus a trailing "Unscheduled" lane for events with no `session_id`. Extract the join into a pure helper.

**Files:**
- Create: `src/lib/timeline-sessions.ts`
- Create: `src/lib/timeline-sessions.test.ts`
- Modify: `src/views/TimelineView.svelte`
- Modify: `src/views/TimelineView.test.ts`

- [ ] **Step 1: Confirm the Session type shape.** Check `src/lib/commands.ts` for the `Session` interface (id field name) and the `getSessions` signature.

Run: `grep -n "interface Session\|export async function getSessions" src/lib/commands.ts`
Expected: note the `Session` `id` field and that `getSessions(campaignId)` returns `Session[]`. Use the real field names below.

- [ ] **Step 2: Write the failing test** `src/lib/timeline-sessions.test.ts` (adjust the `sess()` factory to the real `Session` fields found in Step 1):

```ts
import { describe, it, expect } from 'vitest';
import { groupBySession } from './timeline-sessions';
import type { GraphNode, Session } from './commands';

function ev(name: string, session_id: string | null): GraphNode {
  return {
    id: name, kind: 'event', campaign_id: null, name, summary: null, notes: null,
    created_at: null, updated_at: null, date_start: null, date_end: null,
    is_ongoing: false, sequence_index: null, era: null, duration_label: null,
    session_id, player_name: null, character_class: null, character_level: null, status: null,
  };
}
function sess(id: string, title: string): Session {
  return { id, title } as unknown as Session; // fill remaining required fields per Step 1
}

describe('groupBySession', () => {
  it('nests events under their session in session order, with Unscheduled last', () => {
    const sessions = [sess('s1', 'Session 1'), sess('s2', 'Session 2')];
    const events = [ev('A', 's1'), ev('B', 's2'), ev('C', 's1'), ev('D', null)];
    const lanes = groupBySession(sessions, events);
    expect(lanes.map((l) => l.session?.id ?? null)).toEqual(['s1', 's2', null]);
    expect(lanes[0].events.map((e) => e.name)).toEqual(['A', 'C']);
    expect(lanes[2].events.map((e) => e.name)).toEqual(['D']);
  });

  it('omits the Unscheduled lane when every event has a session', () => {
    const lanes = groupBySession([sess('s1', 'S1')], [ev('A', 's1')]);
    expect(lanes.map((l) => l.session?.id ?? null)).toEqual(['s1']);
  });
});
```

- [ ] **Step 3: Run it to verify it fails.**

Run: `pnpm test --run src/lib/timeline-sessions.test.ts`
Expected: FAIL — cannot find module `./timeline-sessions`.

- [ ] **Step 4: Implement** `src/lib/timeline-sessions.ts` (use the real `Session.id` field name):

```ts
import type { GraphNode, Session } from './commands';

export interface SessionLane {
  session: Session | null; // null = the trailing "Unscheduled" lane
  events: GraphNode[];
}

/**
 * Lay out events into one lane per session (in the given session order),
 * matching `event.session_id` to `session.id`. Events with no `session_id`
 * (or a dangling one) collect into a trailing `session: null` lane, which is
 * omitted when empty.
 */
export function groupBySession(sessions: Session[], events: GraphNode[]): SessionLane[] {
  const lanes: SessionLane[] = sessions.map((s) => ({ session: s, events: [] }));
  const byId = new Map(lanes.map((l) => [l.session!.id, l]));
  const unscheduled: GraphNode[] = [];
  for (const e of events) {
    const lane = e.session_id ? byId.get(e.session_id) : undefined;
    if (lane) lane.events.push(e);
    else unscheduled.push(e);
  }
  if (unscheduled.length > 0) lanes.push({ session: null, events: unscheduled });
  return lanes;
}
```

- [ ] **Step 5: Run it to verify it passes.**

Run: `pnpm test --run src/lib/timeline-sessions.test.ts`
Expected: PASS.

- [ ] **Step 6: Wire Sessions mode into the view.** In `src/views/TimelineView.svelte`:

In the `<script>`, add imports and state:

```ts
  import { getSessions, type Session } from '../lib/commands';
  import { groupBySession, type SessionLane } from '../lib/timeline-sessions';
```

Add `let sessions = $state<Session[]>([]);` next to `events`, load both in `load()`:

```ts
    [events, sessions] = await Promise.all([
      getEventsTimeline(campaignId),
      getSessions(campaignId),
    ]);
```

Add the derived lanes: `const sessionLanes = $derived<SessionLane[]>(groupBySession(sessions, events));`

Add the Sessions branch to the markup, after the `{:else if mode === 'chronicle'}` block:

```svelte
  {:else}
    <ol class="lanes">
      {#each sessionLanes as lane (lane.session?.id ?? '__none__')}
        <li class="lane">
          <h3 class="lane-head">{lane.session?.title ?? 'Unscheduled'}</h3>
          <ol class="events">
            {#each lane.events as e (e.id)}
              <li class="event"><button class="name" onclick={() => onOpenEntity?.(e)}>{e.name}</button></li>
            {/each}
          </ol>
        </li>
      {/each}
    </ol>
  {/if}
```

(Replace the bare closing `{/if}` of the chronicle branch accordingly so the new `{:else}` belongs to the `mode` chain. Use `lane.session?.title` only if `title` is the real field from Step 1; otherwise use the correct label field.)

- [ ] **Step 7: Add a component test** for Sessions mode in `src/views/TimelineView.test.ts`. The component now makes two `invoke` calls (`get_events_timeline`, then `get_sessions`) — mock by command name rather than call order:

```ts
function mockByCommand(map: Record<string, unknown>) {
  invoke.mockImplementation((cmd: string) => Promise.resolve(map[cmd] ?? []));
}
```

Add a test that switches to Sessions mode and asserts lane headers + nesting + the "Unscheduled" lane. Use `fireEvent.click` on the `Sessions` tab (import `fireEvent` from `@testing-library/svelte`). Update the existing chronicle tests to use `mockByCommand({ get_events_timeline: [...] })`.

- [ ] **Step 8: Run all timeline tests.**

Run: `pnpm test --run src/views/TimelineView.test.ts src/lib/timeline-sessions.test.ts`
Expected: PASS.

- [ ] **Step 9: Commit.**

```bash
git add src/lib/timeline-sessions.ts src/lib/timeline-sessions.test.ts src/views/TimelineView.svelte src/views/TimelineView.test.ts
git commit -m "feat(timeline): sessions swimlane mode"
```

---

## Task 4: Wire Timeline into navigation (`g t` + rail button)

**Files:**
- Modify: `src/lib/shortcuts.ts`
- Modify: `src/lib/shortcuts.test.ts`
- Modify: `src/shell/CampaignRail.svelte`
- Modify: `src/shell/Shell.svelte`

- [ ] **Step 1: Write the failing test** in `src/lib/shortcuts.test.ts` — add to the existing `resolveNavChord` describe block:

```ts
  it('maps t to the timeline', () => {
    expect(resolveNavChord('t')).toBe('timeline');
  });
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `pnpm test --run src/lib/shortcuts.test.ts`
Expected: FAIL — received `null`.

- [ ] **Step 3: Implement the chord** in `src/lib/shortcuts.ts`:
  - Extend the type: `export type NavTarget = 'oracle' | 'settings' | 'timeline' | { category: NoteCategoryId };`
  - Add to `NAV_CHORDS`: `t: 'timeline',`
  - Add a help row to `SHORTCUT_HELP` (place it sensibly, e.g. after the `g s` row): `{ keys: 'g t', label: 'Timeline' },`

- [ ] **Step 4: Run it to verify it passes.**

Run: `pnpm test --run src/lib/shortcuts.test.ts`
Expected: PASS.

- [ ] **Step 5: Extend the `View` type** in `src/shell/CampaignRail.svelte` (line 7):

```ts
  export type View =
    | 'oracle'
    | 'campaign'
    | 'settings'
    | 'timeline'
    | { kind: 'notebook'; category: NoteCategoryId };
```

- [ ] **Step 6: Add the Timeline rail button** in `src/shell/CampaignRail.svelte`, inside `<nav class="nav primary">` right after the Oracle button (around line 63):

```svelte
    <button
      class="nav-item"
      class:active={view === 'timeline'}
      onclick={() => setView('timeline')}
    >
      <Icon name="milestone" size={18} className="ic" />
      Timeline
    </button>
```

- [ ] **Step 7: Route the view** in `src/shell/Shell.svelte`:
  - Import the view at the top with the others: `import TimelineView from '../views/TimelineView.svelte';`
  - In `navTo` (line 108), extend the scalar branch: `if (target === 'oracle' || target === 'settings' || target === 'timeline') view = target;`
  - In the title helper (around line 310), add: `if (view === 'timeline') return { title: 'Timeline', sub: 'Your campaign in chronological and session order' };`
  - In the view-render block, add a branch alongside the others (it needs `activeCampaignId`):

```svelte
    {:else if view === 'timeline' && activeCampaignId}
      <TimelineView campaignId={activeCampaignId} />
    {:else if view === 'timeline'}
      <p class="empty">Create or select a campaign to see its timeline.</p>
```

  (Match the exact empty-state markup the `sessions` branch uses at line 575 for consistency.)

- [ ] **Step 8: Add a shared "open entity" deep-link** so timeline events (and later, graph nodes) can open an entity in its notebook. This is one Shell-level primitive plus an `openId` prop on `EntityManager`.

  In `src/components/EntityManager.svelte`:
  - Add `openId?: string | null` to `Props` (around line 21) and `openId = null` to the destructure (line 26).
  - Add an effect that opens the matching node's edit form once entities are loaded:

    ```ts
    // Deep-link: when asked to open a specific entity, open its edit form once
    // it's present in the loaded list.
    $effect(() => {
      if (!openId) return;
      const node = entities.find((n) => n.id === openId);
      if (node) openEdit(node);
    });
    ```

  In `src/shell/Shell.svelte`:
  - Add the inverse map (kind → category) and an `openEntity` helper near `ENTITY_KIND_MAP` (line 43):

    ```ts
    const KIND_TO_CATEGORY = Object.fromEntries(
      Object.entries(ENTITY_KIND_MAP).map(([cat, kind]) => [kind, cat]),
    ) as Record<EntityKind, NoteCategoryId>;

    let pendingOpen = $state<{ id: string; kind: EntityKind } | null>(null);

    function openEntity(id: string, kind: string) {
      const cat = KIND_TO_CATEGORY[kind as EntityKind];
      if (!cat) return;
      pendingOpen = { id, kind: kind as EntityKind };
      view = { kind: 'notebook', category: cat };
    }
    ```

  - Pass `openId` to the `EntityManager` render (the `ENTITY_KIND_MAP` branch around line 581): add
    `openId={pendingOpen && pendingOpen.kind === ENTITY_KIND_MAP[view.category] ? pendingOpen.id : null}`.
  - Pass the callback to the timeline branch added in Step 7:
    `<TimelineView campaignId={activeCampaignId} onOpenEntity={(e) => openEntity(e.id, e.kind)} />`

- [ ] **Step 9: Add a test** for the deep-link in `src/components/EntityManager.test.ts` — render with `openId` set to a loaded entity's id and assert its edit form opens (e.g. the form's name input shows that entity's name). Match the file's existing mock setup.

Run: `pnpm test --run src/components/EntityManager.test.ts`
Expected: PASS (new test + existing tests).

- [ ] **Step 10: Typecheck, lint, and run the frontend suite.**

Run: `pnpm typecheck && pnpm lint && pnpm test --run`
Expected: no type/lint errors; all tests pass.

- [ ] **Step 11: Commit.**

```bash
git add src/lib/shortcuts.ts src/lib/shortcuts.test.ts src/shell/CampaignRail.svelte src/shell/Shell.svelte src/components/EntityManager.svelte src/components/EntityManager.test.ts
git commit -m "feat(timeline): nav entry + g t chord + open-entity deep link"
```

---

# Feature 2 — Entity Relationship Graph

## Task 5: Backend `get_entity_graph` service fn

Reads `relates_to` edges touching the center entity (both directions), then resolves neighbor + center names across the eight node tables. Returns nodes (deduped) + edges (distinct).

**Files:**
- Modify: `src-tauri/src/services/entity_service.rs`
- Modify: `src-tauri/tests/entity_service_test.rs`

- [ ] **Step 1: Add the DTO types** in `src-tauri/src/services/entity_service.rs`, near the other DTOs (after `GraphNode`, around line 170):

```rust
/// A node as it appears in a relationship graph — identity + display only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphNodeRef {
    pub id: String,
    pub kind: String, // table name: npc, location, …
    pub name: String,
}

/// A directed `relates_to` edge between two nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphEdge {
    pub from_id: String,
    pub from_kind: String,
    pub to_id: String,
    pub to_kind: String,
    pub rel_type: String,
    pub notes: Option<String>,
}

/// An ego graph: the center entity, its neighbors, and the edges among them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntityGraph {
    pub nodes: Vec<GraphNodeRef>,
    pub edges: Vec<GraphEdge>,
}
```

- [ ] **Step 2: Write the failing integration test** in `src-tauri/tests/entity_service_test.rs`. Use the existing harness helpers in that file for spinning up `mem::Db` + schema and creating entities/relations (read the top of the file first to reuse its `setup`/`create` helpers; the names below assume `setup_db()` and `entity_service::create`/`relate` — adjust to the real helpers):

```rust
#[tokio::test]
async fn get_entity_graph_returns_center_neighbors_and_edges() {
    let db = setup_db().await; // existing harness in this file
    let campaign = "c1";

    // center NPC + two neighbors
    let varin = entity_service::create(&db, Some(campaign), None, EntityKind::Npc,
        EntityInput { name: "Varin".into(), ..Default::default() }).await.unwrap();
    let keep = entity_service::create(&db, Some(campaign), None, EntityKind::Location,
        EntityInput { name: "The Keep".into(), ..Default::default() }).await.unwrap();
    let pact = entity_service::create(&db, Some(campaign), None, EntityKind::Faction,
        EntityInput { name: "The Pact".into(), ..Default::default() }).await.unwrap();

    // outbound: Varin -> Keep ; inbound: Pact -> Varin
    entity_service::relate(&db, &varin.id, "npc", &keep.id, "location", "resides_in", None).await.unwrap();
    entity_service::relate(&db, &pact.id, "faction", &varin.id, "npc", "controls", None).await.unwrap();

    let graph = entity_service::get_entity_graph(&db, &varin.id, "npc", 1).await.unwrap();

    // nodes: center + 2 neighbors, deduped
    let mut names: Vec<&str> = graph.nodes.iter().map(|n| n.name.as_str()).collect();
    names.sort();
    assert_eq!(names, vec!["The Keep", "The Pact", "Varin"]);

    // edges: both directions present
    assert_eq!(graph.edges.len(), 2);
    assert!(graph.edges.iter().any(|e| e.from_id == varin.id && e.to_id == keep.id && e.rel_type == "resides_in"));
    assert!(graph.edges.iter().any(|e| e.from_id == pact.id && e.to_id == varin.id && e.rel_type == "controls"));
}

#[tokio::test]
async fn get_entity_graph_isolated_entity_returns_just_itself() {
    let db = setup_db().await;
    let lonely = entity_service::create(&db, Some("c1"), None, EntityKind::Npc,
        EntityInput { name: "Hermit".into(), ..Default::default() }).await.unwrap();
    let graph = entity_service::get_entity_graph(&db, &lonely.id, "npc", 1).await.unwrap();
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].name, "Hermit");
    assert!(graph.edges.is_empty());
}
```

- [ ] **Step 3: Run it to verify it fails.**

Run: `cargo test --test entity_service_test get_entity_graph`
Expected: FAIL — `get_entity_graph` not found (won't compile).

- [ ] **Step 4: Implement the service fn** in `src-tauri/src/services/entity_service.rs`, near `relate` (around line 822):

```rust
/// Fetch the ego graph around an entity: the center, its `relates_to` neighbors
/// (one hop), and the edges among them. `depth` is currently always treated as
/// one hop; deeper walks are produced client-side by re-calling on a neighbor.
pub async fn get_entity_graph<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    id: &str,
    kind: &str,
    _depth: u32,
) -> Result<EntityGraph, EntityError> {
    #[derive(Deserialize)]
    struct EdgeRow {
        #[serde(rename = "in")]
        in_: Thing,
        out: Thing,
        rel_type: String,
        notes: Option<String>,
    }

    // 1. Edges touching the center, both directions. Build the center Thing in
    //    the query string (RELATE-style) since type::thing in WHERE on edge
    //    endpoints is unreliable on some SurrealDB versions.
    let edge_sql = format!(
        "SELECT in, out, rel_type, notes FROM relates_to \
         WHERE in = {kind}:{id} OR out = {kind}:{id}"
    );
    let mut resp = db.query(edge_sql).await.map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    let rows: Vec<EdgeRow> = resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;

    let edges: Vec<GraphEdge> = rows
        .iter()
        .map(|r| GraphEdge {
            from_id: r.in_.id.to_raw(),
            from_kind: r.in_.tb.clone(),
            to_id: r.out.id.to_raw(),
            to_kind: r.out.tb.clone(),
            rel_type: r.rel_type.clone(),
            notes: r.notes.clone(),
        })
        .collect();

    // 2. Collect distinct (kind, id) node keys: the center plus every endpoint.
    let mut keys: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
    keys.insert((kind.to_string(), id.to_string()));
    for r in &rows {
        keys.insert((r.in_.tb.clone(), r.in_.id.to_raw()));
        keys.insert((r.out.tb.clone(), r.out.id.to_raw()));
    }

    // 3. Resolve names. Group ids by table and query each table once.
    #[derive(Deserialize)]
    struct NameRow {
        id: Thing,
        name: String,
    }
    let mut by_table: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for (k, i) in &keys {
        by_table.entry(k.clone()).or_default().push(i.clone());
    }

    let mut nodes: Vec<GraphNodeRef> = Vec::new();
    for (table, ids) in by_table {
        // Build the id list as `Thing`s in Rust and bind as an array — robust
        // across SurrealDB versions, unlike type::thing() inside the query.
        let things: Vec<Thing> = ids
            .iter()
            .map(|i| Thing::from((table.as_str(), i.as_str())))
            .collect();
        let mut r = db
            .query("SELECT id, name FROM type::table($table) WHERE id IN $ids")
            .bind(("table", table.clone()))
            .bind(("ids", things))
            .await
            .map_err(|e| EntityError::Database {
                message: e.to_string(),
            })?;
        let found: Vec<NameRow> = r.take(0).map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
        for nr in found {
            nodes.push(GraphNodeRef {
                id: nr.id.id.to_raw(),
                kind: nr.id.tb.clone(),
                name: nr.name,
            });
        }
    }

    Ok(EntityGraph { nodes, edges })
}
```

- [ ] **Step 5: Run the tests to verify they pass.**

Run: `cargo test --test entity_service_test get_entity_graph -- --nocapture`
Expected: PASS for both. If the `relates_to` `in`/`out` deserialization fails, print the raw rows with `--nocapture` and adjust field renames; the SurrealQL is verified at runtime per project rules.

- [ ] **Step 6: Lint.**

Run: `cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: clean. Fix any unused-import or unused-variable warnings.

- [ ] **Step 7: Commit.**

```bash
git add src-tauri/src/services/entity_service.rs src-tauri/tests/entity_service_test.rs
git commit -m "feat(graph): get_entity_graph service fn (ego graph, both directions)"
```

---

## Task 6: Expose `get_entity_graph` command + frontend wrapper

**Files:**
- Modify: `src-tauri/src/commands/entity_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands/mod.rs` (if it lists commands explicitly)
- Modify: `src/lib/commands.ts`

- [ ] **Step 1: Add the command** in `src-tauri/src/commands/entity_commands.rs`. Add `EntityGraph` to the `use` import at line 4, then:

```rust
/// Ego graph (one hop) around an entity: center, neighbors, and edges.
#[tauri::command]
pub async fn get_entity_graph(
    state: State<'_, Arc<AppState>>,
    id: String,
    kind: String,
    depth: u32,
) -> Result<EntityGraph, EntityError> {
    let k = parse_kind(&kind)?;
    entity_service::get_entity_graph(&state.db, &id, k.table_name(), depth).await
}
```

- [ ] **Step 2: Register it** in `src-tauri/src/lib.rs` next to `get_events_timeline`:

```rust
            commands::get_entity_graph,
```

  And add to `src-tauri/src/commands/mod.rs` if commands are re-exported explicitly.

- [ ] **Step 3: Build.**

Run: `cargo build`
Expected: compiles.

- [ ] **Step 4: Add TS types + wrapper** in `src/lib/commands.ts`, near the entity types (after the `GraphNode` interface):

```ts
export interface GraphNodeRef {
  id: string;
  kind: string;
  name: string;
}
export interface GraphEdge {
  from_id: string;
  from_kind: string;
  to_id: string;
  to_kind: string;
  rel_type: string;
  notes: string | null;
}
export interface EntityGraph {
  nodes: GraphNodeRef[];
  edges: GraphEdge[];
}

/** Ego graph (one hop) around an entity. Re-call on a neighbor to expand. */
export async function getEntityGraph(
  id: string,
  kind: EntityKind,
  depth = 1,
): Promise<EntityGraph> {
  return invoke<EntityGraph>('get_entity_graph', { id, kind, depth });
}
```

- [ ] **Step 5: Typecheck.**

Run: `pnpm typecheck`
Expected: no errors.

- [ ] **Step 6: Commit.**

```bash
git add src-tauri/src/commands/entity_commands.rs src-tauri/src/lib.rs src-tauri/src/commands/mod.rs src/lib/commands.ts
git commit -m "feat(graph): expose get_entity_graph command + wrapper"
```

---

## Task 7: Add the `d3-force` dependency

**Files:**
- Modify: `package.json` (via pnpm)

- [ ] **Step 1: Add the runtime dep and types.**

Run: `pnpm add d3-force && pnpm add -D @types/d3-force`
Expected: both added; `pnpm-lock.yaml` updated.

- [ ] **Step 2: Verify it imports under the test/build toolchain.** Create a temporary scratch check:

Run: `pnpm typecheck`
Expected: no errors (the dep resolves with types).

- [ ] **Step 3: Commit.**

```bash
git add package.json pnpm-lock.yaml
git commit -m "build(graph): add d3-force for graph layout"
```

---

## Task 8: `EntityGraph.svelte` — render nodes + edges

Renders a fetched `EntityGraph` as SVG, positioned by a `d3-force` simulation. This task covers static render; interactions come in Task 9.

**Files:**
- Create: `src/lib/graph-colors.ts`
- Create: `src/components/EntityGraph.svelte`
- Create: `src/components/EntityGraph.test.ts`

- [ ] **Step 1: Create the kind→color map** in `src/lib/graph-colors.ts`:

```ts
import type { EntityKind } from './commands';

/** Node fill per entity kind (aligned with the graph mockup palette). */
export const KIND_COLOR: Record<EntityKind, string> = {
  npc: '#6699cc',
  location: '#99aa88',
  faction: '#cc6699',
  creature: '#cc9966',
  item: '#cc66cc',
  event: '#66cc99',
  player_character: '#ffcc66',
  misc: '#8899aa',
};

export function kindColor(kind: string): string {
  return (KIND_COLOR as Record<string, string>)[kind] ?? '#8899aa';
}
```

- [ ] **Step 2: Create the component** `src/components/EntityGraph.svelte`. The simulation runs on mount; Svelte renders positions reactively. Re-center/expand are added in Task 9 — keep the `onSelect`/`onExpand` props as no-op-capable callbacks now so the markup is stable.

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    forceSimulation, forceManyBody, forceLink, forceCenter, forceCollide,
    type Simulation, type SimulationNodeDatum,
  } from 'd3-force';
  import { getEntityGraph, type EntityGraph, type GraphNodeRef, type EntityKind } from '../lib/commands';
  import { kindColor } from '../lib/graph-colors';

  interface Props {
    entityId: string;
    entityKind: string;
    width?: number;
    height?: number;
    onClose?: () => void;
    onOpenEntity?: (node: GraphNodeRef) => void; // click-through: open in the manager
  }
  const { entityId, entityKind, width = 720, height = 520, onClose, onOpenEntity }: Props = $props();

  type SimNode = GraphNodeRef & SimulationNodeDatum;
  interface SimLink { source: string; target: string; rel_type: string; }

  let centerId = $state(entityId);
  let centerKind = $state(entityKind);
  let nodes = $state<SimNode[]>([]);
  let links = $state<SimLink[]>([]);
  let loading = $state(true);
  let sim: Simulation<SimNode, undefined> | null = null;
  let tick = $state(0); // bumped each simulation tick to force re-render

  onMount(() => void recenter(entityId, entityKind));
  onDestroy(() => sim?.stop());

  async function recenter(id: string, kind: string) {
    loading = true;
    centerId = id;
    centerKind = kind;
    try {
      const g: EntityGraph = await getEntityGraph(id, kind as EntityKind, 1);
      buildSimulation(g);
    } catch (e) {
      console.error('Failed to load graph:', e);
      nodes = []; links = [];
    } finally {
      loading = false;
    }
  }

  function buildSimulation(g: EntityGraph) {
    sim?.stop();
    nodes = g.nodes.map((n) => ({ ...n }));
    links = g.edges.map((e) => ({ source: e.from_id, target: e.to_id, rel_type: e.rel_type }));
    sim = forceSimulation<SimNode>(nodes)
      .force('charge', forceManyBody().strength(-280))
      .force('center', forceCenter(width / 2, height / 2))
      .force('collide', forceCollide(28))
      .force('link', forceLink<SimNode, SimLink>(links as never)
        .id((d) => (d as SimNode).id).distance(110))
      .on('tick', () => (tick += 1));
  }

  function nodeById(id: string): SimNode | undefined {
    void tick; // reactive dependency
    return nodes.find((n) => n.id === id);
  }
</script>

<div class="graph-wrap" data-testid="entity-graph">
  {#if onClose}
    <button class="close" onclick={onClose} aria-label="Close graph">✕</button>
  {/if}
  {#if loading}
    <p class="muted">Loading graph…</p>
  {:else if nodes.length <= 1}
    <p class="muted" data-testid="graph-empty">No relationships yet for this entity.</p>
  {/if}
  <svg {width} {height} role="img" aria-label="Entity relationship graph">
    {#each links as l (l.source + '->' + l.target)}
      {@const a = nodeById(l.source)}
      {@const b = nodeById(l.target)}
      {#if a && b}
        <line x1={a.x} y1={a.y} x2={b.x} y2={b.y} class="edge" />
        <text x={((a.x ?? 0) + (b.x ?? 0)) / 2} y={((a.y ?? 0) + (b.y ?? 0)) / 2}
          class="edge-label">{l.rel_type}</text>
      {/if}
    {/each}
    {#each nodes as n (n.id)}
      <g class="node" data-id={n.id} data-name={n.name}>
        <circle cx={n.x} cy={n.y} r={n.id === centerId ? 16 : 10} fill={kindColor(n.kind)} />
        <text x={n.x} y={(n.y ?? 0) + 26} class="node-label" text-anchor="middle">{n.name}</text>
      </g>
    {/each}
  </svg>
</div>

<style>
  .graph-wrap { position: relative; }
  .close { position: absolute; top: 8px; right: 8px; }
  .edge { stroke: var(--line); stroke-width: 1.5; }
  .edge-label { fill: var(--fg-3); font-size: 10px; text-anchor: middle; }
  .node-label { fill: var(--fg-2); font-size: 11px; }
  .muted { color: var(--fg-3); }
</style>
```

- [ ] **Step 3: Write the component test** `src/components/EntityGraph.test.ts`. Force positions are non-deterministic, so assert on node/edge **presence**, not coordinates:

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import EntityGraph from './EntityGraph.svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

beforeEach(() => invoke.mockReset());

const graph = {
  nodes: [
    { id: 'varin', kind: 'npc', name: 'Varin' },
    { id: 'keep', kind: 'location', name: 'The Keep' },
  ],
  edges: [{ from_id: 'varin', from_kind: 'npc', to_id: 'keep', to_kind: 'location', rel_type: 'resides_in', notes: null }],
};

describe('EntityGraph', () => {
  it('renders center + neighbor nodes and the edge label', async () => {
    invoke.mockResolvedValueOnce(graph);
    render(EntityGraph, { entityId: 'varin', entityKind: 'npc' });
    expect(await screen.findByText('Varin')).toBeInTheDocument();
    expect(screen.getByText('The Keep')).toBeInTheDocument();
    expect(screen.getByText('resides_in')).toBeInTheDocument();
  });

  it('shows an empty state for a lonely entity', async () => {
    invoke.mockResolvedValueOnce({ nodes: [{ id: 'h', kind: 'npc', name: 'Hermit' }], edges: [] });
    render(EntityGraph, { entityId: 'h', entityKind: 'npc' });
    expect(await screen.findByTestId('graph-empty')).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Run the tests.**

Run: `pnpm test --run src/components/EntityGraph.test.ts`
Expected: PASS. If `d3-force` ticks asynchronously and labels aren't found, the text nodes still render on first paint (positions default to `undefined`→`0`); assert text presence via `findByText` which waits.

- [ ] **Step 5: Commit.**

```bash
git add src/lib/graph-colors.ts src/components/EntityGraph.svelte src/components/EntityGraph.test.ts
git commit -m "feat(graph): EntityGraph component — d3-force layout + SVG render"
```

---

## Task 9: Graph interactions — re-center, expand, pan/zoom, drag-to-pin

Re-center and expand share node/edge merge logic; extract it as a pure helper for unit testing. Interaction wiring (pan/zoom/drag) is verified by the component test for re-center + a unit test for merge; pan/zoom/drag are exercised manually (see Step 8).

**Files:**
- Create: `src/lib/graph-merge.ts`
- Create: `src/lib/graph-merge.test.ts`
- Modify: `src/components/EntityGraph.svelte`
- Modify: `src/components/EntityGraph.test.ts`

- [ ] **Step 1: Write the failing merge test** `src/lib/graph-merge.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { mergeGraph } from './graph-merge';
import type { EntityGraph } from './commands';

const base: EntityGraph = {
  nodes: [{ id: 'a', kind: 'npc', name: 'A' }, { id: 'b', kind: 'npc', name: 'B' }],
  edges: [{ from_id: 'a', from_kind: 'npc', to_id: 'b', to_kind: 'npc', rel_type: 'knows', notes: null }],
};

describe('mergeGraph', () => {
  it('adds new nodes/edges and dedupes existing ones', () => {
    const extra: EntityGraph = {
      nodes: [{ id: 'b', kind: 'npc', name: 'B' }, { id: 'c', kind: 'npc', name: 'C' }],
      edges: [
        { from_id: 'a', from_kind: 'npc', to_id: 'b', to_kind: 'npc', rel_type: 'knows', notes: null }, // dup
        { from_id: 'b', from_kind: 'npc', to_id: 'c', to_kind: 'npc', rel_type: 'rivals', notes: null }, // new
      ],
    };
    const merged = mergeGraph(base, extra);
    expect(merged.nodes.map((n) => n.id).sort()).toEqual(['a', 'b', 'c']);
    expect(merged.edges.length).toBe(2);
  });
});
```

- [ ] **Step 2: Run it to verify it fails.**

Run: `pnpm test --run src/lib/graph-merge.test.ts`
Expected: FAIL — cannot find module `./graph-merge`.

- [ ] **Step 3: Implement** `src/lib/graph-merge.ts`:

```ts
import type { EntityGraph } from './commands';

const edgeKey = (e: EntityGraph['edges'][number]) =>
  `${e.from_id}->${e.to_id}:${e.rel_type}`;

/** Union two graphs, deduping nodes by id and edges by (from,to,rel_type). */
export function mergeGraph(a: EntityGraph, b: EntityGraph): EntityGraph {
  const nodes = new Map(a.nodes.map((n) => [n.id, n]));
  for (const n of b.nodes) if (!nodes.has(n.id)) nodes.set(n.id, n);
  const edges = new Map(a.edges.map((e) => [edgeKey(e), e]));
  for (const e of b.edges) if (!edges.has(edgeKey(e))) edges.set(edgeKey(e), e);
  return { nodes: [...nodes.values()], edges: [...edges.values()] };
}
```

- [ ] **Step 4: Run it to verify it passes.**

Run: `pnpm test --run src/lib/graph-merge.test.ts`
Expected: PASS.

- [ ] **Step 5: Wire interactions into `EntityGraph.svelte`:**
  - **Re-center on node click:** add `onclick={() => recenter(n.id, n.kind)}` to the node `<g>` and `style="cursor:pointer"`.
  - **Open the entity:** make the node's name label a click-through — `onclick|stopPropagation={() => onOpenEntity?.(n)}` on the `<text class="node-label">` (so it opens the manager rather than re-centering). Add `cursor: pointer` to `.node-label`.
  - **Expand:** import `mergeGraph`; add a small `＋` affordance per node (a `<text>` or `<circle>` next to it) with `onclick|stopPropagation={() => expand(n.id, n.kind)}` where:

    ```ts
    import { mergeGraph } from '../lib/graph-merge';
    async function expand(id: string, kind: string) {
      try {
        const g = await getEntityGraph(id, kind as EntityKind, 1);
        const current = { nodes, edges: links.map((l) => ({
          from_id: l.source, from_kind: '', to_id: l.target, to_kind: '', rel_type: l.rel_type, notes: null,
        })) };
        buildSimulation(mergeGraph(current as never, g));
      } catch (e) { console.error('expand failed', e); }
    }
    ```

  - **Pan/zoom:** wrap the `<svg>` children in a `<g transform={`translate(${pan.x},${pan.y}) scale(${zoom})`}>`; add `onwheel` on the svg to adjust `zoom` (clamp 0.4–2.5) and pointer-drag on empty canvas to adjust `pan`. Keep `pan`/`zoom` as `$state`.
  - **Drag-to-pin:** on node `pointerdown`, set `n.fx = n.x; n.fy = n.y;` and follow the pointer updating `n.fx/n.fy`; `sim.alphaTarget(0.3).restart()` while dragging, `sim.alphaTarget(0)` on release. Leave `fx/fy` set after release (pinned). Reset pins on re-center (the new simulation replaces `nodes`).

  Keep each piece small; the simulation already re-renders via the `tick` bump.

- [ ] **Step 6: Add a re-center component test** to `src/components/EntityGraph.test.ts`:

```ts
import { fireEvent } from '@testing-library/svelte';

it('re-centers when a neighbor node is clicked', async () => {
  invoke
    .mockResolvedValueOnce(graph) // initial: varin-centered
    .mockResolvedValueOnce({ // after clicking The Keep
      nodes: [{ id: 'keep', kind: 'location', name: 'The Keep' }, { id: 'town', kind: 'location', name: 'Town' }],
      edges: [{ from_id: 'keep', from_kind: 'location', to_id: 'town', to_kind: 'location', rel_type: 'near', notes: null }],
    });
  render(EntityGraph, { entityId: 'varin', entityKind: 'npc' });
  const keep = await screen.findByText('The Keep');
  await fireEvent.click(keep);
  expect(await screen.findByText('Town')).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith('get_entity_graph', { id: 'keep', kind: 'location', depth: 1 });
});
```

- [ ] **Step 7: Run all graph tests.**

Run: `pnpm test --run src/components/EntityGraph.test.ts src/lib/graph-merge.test.ts`
Expected: PASS.

- [ ] **Step 8: Manual interaction check (no automated test for pan/zoom/drag).** Build and exercise drag-to-pin, wheel-zoom, and canvas-pan by hand.

Run: `cargo tauri dev`
Expected: open an entity's graph (after Task 10), drag a node (stays pinned), wheel zooms, dragging empty canvas pans. Note any issues and fix before committing.

- [ ] **Step 9: Commit.**

```bash
git add src/lib/graph-merge.ts src/lib/graph-merge.test.ts src/components/EntityGraph.svelte src/components/EntityGraph.test.ts
git commit -m "feat(graph): re-center, expand-merge, pan/zoom, drag-to-pin"
```

---

## Task 10: "View graph" affordance + Shell wiring

Open the graph from an entity. The graph shows as a modal overlay in the Shell so it floats over any notebook view.

**Files:**
- Modify: `src/components/EntityManager.svelte`
- Modify: `src/shell/Shell.svelte`
- Modify: `src/components/EntityManager.test.ts`

- [ ] **Step 1: Add an "open graph" event from `EntityManager.svelte`.** Add an `onViewGraph?: (node: GraphNode) => void` prop to `Props` (line 21 area) and a "Graph" button in each entity row (near the existing edit button at line 181):

```svelte
              {#if onViewGraph}
                <button class="entity-graph-btn" title="View relationships"
                  onclick={() => onViewGraph(node)}>Graph</button>
              {/if}
```

- [ ] **Step 2: Write the failing test** in `src/components/EntityManager.test.ts` — assert the callback fires with the node. Follow the file's existing render/mock setup; add:

```ts
it('calls onViewGraph with the entity when the Graph button is clicked', async () => {
  // ...render EntityManager with a mocked entity list + onViewGraph spy...
  const onViewGraph = vi.fn();
  // render(EntityManager, { campaignId: 'c1', kind: 'npc', onViewGraph });
  // const btn = await screen.findByTitle('View relationships');
  // await fireEvent.click(btn);
  // expect(onViewGraph).toHaveBeenCalledWith(expect.objectContaining({ id: expect.any(String) }));
});
```

Fill in the render/mock lines to match the rest of `EntityManager.test.ts` (it already mocks `getEntities`). Assert `onViewGraph` is called with the clicked node.

- [ ] **Step 3: Run it to verify it fails.**

Run: `pnpm test --run src/components/EntityManager.test.ts`
Expected: FAIL — button not found.

- [ ] **Step 4: Confirm Step 1 satisfies it; run again.**

Run: `pnpm test --run src/components/EntityManager.test.ts`
Expected: PASS.

- [ ] **Step 5: Wire the overlay in `src/shell/Shell.svelte`:**
  - Import: `import EntityGraph from '../components/EntityGraph.svelte';`
  - State: `let graphFor = $state<{ id: string; kind: string } | null>(null);`
  - Pass the callback where `EntityManager` is rendered (the `ENTITY_KIND_MAP` branch around line 581): add `onViewGraph={(n) => (graphFor = { id: n.id, kind: n.kind })}`.
  - Add the overlay near the other modals (e.g. by the help overlay around line 476):

```svelte
  {#if graphFor}
    <div class="graph-overlay" role="dialog" aria-label="Entity relationships">
      <div class="graph-panel" use:modalBehavior={{ onClose: () => (graphFor = null) }}>
        <EntityGraph entityId={graphFor.id} entityKind={graphFor.kind}
          onClose={() => (graphFor = null)}
          onOpenEntity={(n) => { graphFor = null; openEntity(n.id, n.kind); }} />
      </div>
    </div>
  {/if}
```

  Add minimal CSS for `.graph-overlay` (fixed, centered, dim backdrop) mirroring the existing help/picker overlay styles in this file.

- [ ] **Step 6: Typecheck, lint, full suite.**

Run: `pnpm typecheck && pnpm lint && pnpm test --run`
Expected: clean; all tests pass.

- [ ] **Step 7: Manual smoke test.**

Run: `cargo tauri dev`
Expected: in an entity notebook, click **Graph** on a row → overlay opens centered on that entity; click a neighbor to re-center; ＋ expands; Esc / ✕ closes.

- [ ] **Step 8: Commit.**

```bash
git add src/components/EntityManager.svelte src/components/EntityManager.test.ts src/shell/Shell.svelte
git commit -m "feat(graph): View graph affordance + Shell overlay"
```

---

## Final verification

- [ ] **Backend gates:**

Run: `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`
Expected: all pass.

- [ ] **Frontend gates:**

Run: `pnpm typecheck && pnpm lint && pnpm test --run`
Expected: all pass.

- [ ] **Update the roadmap.** In `docs/architecture.md`, tick the two Phase 3 items:
  - `- [x] Session log timeline view`
  - `- [x] Entity relationship graph (...)`

```bash
git add docs/architecture.md
git commit -m "docs: tick Phase 3 timeline + entity graph as complete"
```

---

## Notes for the implementer

- **SurrealQL is runtime-tested only** (project rule). If the `relates_to` edge query in Task 5 returns unexpected shapes, run the test with `--nocapture` and inspect; the `in`/`out` fields on an edge row are `Thing`s with `.tb` (kind) and `.id`.
- **`sequence_index` is canonical; `date_*` strings are never parsed** — the timeline only displays them.
- **Force-layout coordinates are non-deterministic** — never assert on `x`/`y` in tests; assert on presence and interaction outcomes.
- **Match existing test-mock style** — before writing the first component test, open `src/components/EntityForm.test.ts` and copy whichever `invoke`-mocking approach it uses, rather than the illustrative `vi.mock` above if they differ.
