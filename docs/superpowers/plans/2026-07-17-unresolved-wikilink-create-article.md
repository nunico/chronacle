# Unresolved Wikilink Create-Article Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let unresolved wikilinks in text, Maintenance, and the relationship graph
drive the same user-confirmed create-article workflow.

**Architecture:** Keep `broken_wikilink` as the stored lint kind and change
presentation/action semantics in the frontend. Use a shared pending-create request
owned by `Shell.svelte`, consumed by `EntityManager`, and triggered from
`WikiText`, `MaintenanceView`, or missing nodes in `EntityGraph`. Extend graph
backend output with synthetic `missing_wikilink` nodes and `unresolved` edges for
unresolved text links.

**Tech Stack:** Svelte 5 runes, TypeScript, Vitest + Testing Library, Tauri IPC
wrappers, Rust service tests with SurrealDB in-memory engine.

## Global Constraints

- Tauri IPC only; no HTTP or WebSocket.
- No new backend command or lint kind for the first implementation.
- Reuse existing `createEntity(campaignId, kind, input)` and `resolveLintFinding(id)`.
- Creation is always user-confirmed through the existing entity form.
- Do not guess entity kind from link text; the user chooses kind.
- `broken_wikilink.payload.candidates.length > 0` renders as `Possible name mismatch`.
- `broken_wikilink.payload.candidates.length === 0` renders as `Missing article`.
- Unresolved graph nodes use `kind: "missing_wikilink"` and `rel_type: "unresolved"`.
- Do not silently discard typed form fields when launching create-from-link.
- No new dependencies.

---

## File Map

- Modify `apps/desktop/src/lib/commands.ts`
  - Add `missing?: boolean`, `source_id?: string`, `source_kind?: string` to
    `GraphNodeRef`.
  - Allow `GraphEdge.to_kind` / `from_kind` to carry `missing_wikilink`.
- Create `apps/desktop/src/lib/wikilinks.ts`
  - Shared frontend normalization and entity-map builder.
- Create `apps/desktop/src/lib/wikilinks.test.ts`
  - Unit coverage for alias/normalization map behavior and collision handling.
- Modify `apps/desktop/src/components/WikiText.svelte`
  - Resolve links via the shared map keys.
  - Render unresolved links as create buttons when `onMissingLinkClick` is
    provided and inert text otherwise.
- Modify `apps/desktop/src/components/WikiText.test.ts`
  - Cover unresolved-link click behavior and alias/normalized matches.
- Modify `apps/desktop/src/components/EntityForm.svelte`
  - Accept `initialName` for create mode.
  - Report dirty state to the parent so pending create does not overwrite edits.
- Modify `apps/desktop/src/components/EntityManager.svelte`
  - Build entity maps with aliases/normalized keys.
  - Consume pending create requests once and pass `initialName` to `EntityForm`.
  - Notify shell when a source Maintenance finding should resolve after save.
- Modify `apps/desktop/src/components/EntityManager.test.ts`
  - Cover pending create, prefilled name, dirty-form guard, and post-save callback.
- Modify `apps/desktop/src/shell/Shell.svelte`
  - Own the create-kind chooser, create `PendingCreate`, route to the right
    notebook category, and resolve lint finding after a Maintenance-origin create
    succeeds.
- Modify `apps/desktop/src/views/MaintenanceView.svelte`
  - Split `broken_wikilink` display into `Possible name mismatch` and
    `Missing article`.
  - Add `Create article` action via shell callback.
- Modify `apps/desktop/src/views/MaintenanceView.test.ts`
  - Cover relabeling, create action, suggestion action, and no-candidate action.
- Modify `crates/chronacle-extraction/src/entity_service/types.rs`
  - Add missing-node metadata fields to `GraphNodeRef`, omitted from JSON when
    absent.
- Modify `crates/chronacle-extraction/src/entity_service/relations/graph.rs`
  - Add synthetic missing-link nodes and unresolved edges.
- Modify `crates/chronacle-extraction/src/entity_service/relations/relations_tests.rs`
  - Cover graph missing nodes and no synthetic node for resolvable links.
- Modify `apps/desktop/src/components/EntityGraph.svelte`
  - Render missing nodes distinctly and route clicks to create flow.
- Modify `apps/desktop/src/components/EntityGraph.test.ts`
  - Cover missing-node rendering and click behavior.
- Add or update `apps/desktop/tests/e2e/features/*.feature`
  - Add BDD scenarios from the design spec.

---

### Task 1: Shared Wikilink Resolution Map

**Files:**

- Create: `apps/desktop/src/lib/wikilinks.ts`
- Create: `apps/desktop/src/lib/wikilinks.test.ts`
- Modify: `apps/desktop/src/lib/commands.ts`
- Modify: `apps/desktop/src/components/WikiText.svelte`
- Modify: `apps/desktop/src/components/WikiText.test.ts`

**Interfaces:**

- Consumes: `GraphNode` and `GraphNodeRef` from `apps/desktop/src/lib/commands.ts`
- Produces:
  - `normalizeWikiLinkKey(name: string): string`
  - `buildWikiLinkEntityMap(nodes: Array<Pick<GraphNode, "id" | "kind" | "name" | "aliases">>): Map<string, { id: string; kind: string }>`
  - `WikiText` prop `onMissingLinkClick?: (name: string) => void`

- [ ] **Step 1: Extend frontend graph node typing**

  In `apps/desktop/src/lib/commands.ts`, extend `GraphNodeRef`:

  ```ts
  export interface GraphNodeRef {
    id: string;
    kind: string;
    name: string;
    missing?: boolean;
    source_id?: string;
    source_kind?: string;
  }
  ```

- [ ] **Step 2: Write failing tests for normalization and collisions**

  Create `apps/desktop/src/lib/wikilinks.test.ts`:

  ```ts
  import { describe, expect, it } from "vitest";
  import { buildWikiLinkEntityMap, normalizeWikiLinkKey } from "./wikilinks";

  const node = (overrides: Partial<any>) => ({
    id: "id1",
    kind: "npc",
    name: "The Moon Gates",
    aliases: [],
    ...overrides,
  });

  describe("wikilinks", () => {
    it("normalizes case, leading the, possessives, plurals, and punctuation", () => {
      expect(normalizeWikiLinkKey(" The Moon Gate's ")).toBe("moon gate");
      expect(normalizeWikiLinkKey("The Moon Gates")).toBe("moon gate");
      expect(normalizeWikiLinkKey("Moon--Gate")).toBe("moon gate");
    });

    it("indexes primary names and aliases by exact and normalized keys", () => {
      const map = buildWikiLinkEntityMap([
        node({
          id: "loc1",
          kind: "location",
          name: "The Moon Gates",
          aliases: ["Selene Door"],
        }),
      ]);

      expect(map.get("the moon gates")).toEqual({
        id: "loc1",
        kind: "location",
      });
      expect(map.get("selene door")).toEqual({ id: "loc1", kind: "location" });
      expect(map.get("moon gate")).toEqual({ id: "loc1", kind: "location" });
    });

    it("drops colliding keys instead of picking a winner", () => {
      const map = buildWikiLinkEntityMap([
        node({ id: "a", name: "The Free League" }),
        node({ id: "b", name: "Free Leagues" }),
      ]);

      expect(map.has("free league")).toBe(false);
    });
  });
  ```

- [ ] **Step 3: Implement `wikilinks.ts`**

  ```ts
  import type { GraphNode } from "./commands";

  export interface WikiLinkTarget {
    id: string;
    kind: string;
  }

  type NodeLike = Pick<GraphNode, "id" | "kind" | "name" | "aliases">;

  export function normalizeWikiLinkKey(name: string): string {
    let key = name
      .trim()
      .toLowerCase()
      .replace(/['’]s\b/g, "")
      .replace(/^the\s+/u, "")
      .replace(/[^\p{L}\p{N}]+/gu, " ")
      .trim()
      .replace(/\s+/g, " ");

    key = key
      .split(" ")
      .map((part) => singularize(part))
      .join(" ");
    return key;
  }

  function singularize(part: string): string {
    if (part.endsWith("ss") || part.endsWith("us") || part.length <= 3)
      return part;
    if (part.endsWith("es")) return part.slice(0, -2);
    if (part.endsWith("s")) return part.slice(0, -1);
    return part;
  }

  export function buildWikiLinkEntityMap(
    nodes: NodeLike[],
  ): Map<string, WikiLinkTarget> {
    const values = new Map<string, WikiLinkTarget>();
    const collisions = new Set<string>();

    function insert(key: string, target: WikiLinkTarget) {
      if (!key || collisions.has(key)) return;
      const existing = values.get(key);
      if (
        existing &&
        (existing.id !== target.id || existing.kind !== target.kind)
      ) {
        values.delete(key);
        collisions.add(key);
        return;
      }
      values.set(key, target);
    }

    for (const node of nodes) {
      const target = { id: node.id, kind: node.kind };
      for (const raw of [node.name, ...(node.aliases ?? [])]) {
        insert(raw.trim().toLowerCase(), target);
        insert(normalizeWikiLinkKey(raw), target);
      }
    }

    return values;
  }
  ```

- [ ] **Step 4: Run utility tests and verify they fail before implementation**

  Run after Step 2 and before Step 3:

  ```bash
  pnpm -C apps/desktop test:run src/lib/wikilinks.test.ts
  ```

  Expected before implementation: fail with module/function not found.

- [ ] **Step 5: Run utility tests and verify they pass**

  ```bash
  pnpm -C apps/desktop test:run src/lib/wikilinks.test.ts
  ```

  Expected after implementation: pass.

- [ ] **Step 6: Update `WikiText` tests**

  In `apps/desktop/src/components/WikiText.test.ts`, add:

  ```ts
  it("calls onMissingLinkClick when an unmatched wikilink button is clicked", async () => {
    const onMissingLinkClick = vi.fn();
    render(WikiText, {
      props: {
        text: "Go to [[Moon Gate]]",
        entities: new Map(),
        onMissingLinkClick,
      },
    });

    await fireEvent.click(
      screen.getByRole("button", { name: "Create article for Moon Gate" }),
    );
    expect(onMissingLinkClick).toHaveBeenCalledWith("Moon Gate");
  });

  it("keeps unmatched wikilinks inert when no missing-link callback is provided", () => {
    render(WikiText, {
      props: { text: "Go to [[Moon Gate]]", entities: new Map() },
    });

    expect(
      screen.queryByRole("button", { name: "Create article for Moon Gate" }),
    ).toBeNull();
    expect(screen.getByText(/\[\[Moon Gate\]\]/)).toBeTruthy();
  });
  ```

- [ ] **Step 7: Update `WikiText.svelte` implementation**

  Add the prop and render an unresolved button only when the callback exists:

  ```svelte
  interface Props {
    text: string;
    entities: Map<string, { id: string; kind: string }>;
    onEntityClick?: (id: string, kind: string) => void;
    onMissingLinkClick?: (name: string) => void;
  }
  ```

  Use lower-case lookup first, then normalized lookup if imported:

  ```ts
  import { normalizeWikiLinkKey } from "../lib/wikilinks";
  const key =
    entities.get(name.toLowerCase()) ??
    entities.get(normalizeWikiLinkKey(name));
  ```

  Render unmatched links:

  ```svelte
  {#if onMissingLinkClick}
    <button
      type="button"
      class="missing-link"
      aria-label={`Create article for ${seg.name}`}
      onclick={() => onMissingLinkClick(seg.name)}
    >
      [[{seg.name}]]
    </button>
  {:else}
    [[{seg.name}]]
  {/if}
  ```

- [ ] **Step 8: Validate `WikiText`**

  ```bash
  pnpm -C apps/desktop test:run src/components/WikiText.test.ts
  npx @sveltejs/mcp svelte-autofixer apps/desktop/src/components/WikiText.svelte
  ```

  Expected: tests pass; autofixer reports no required changes.

- [ ] **Step 9: Commit Task 1**

  ```bash
  git add apps/desktop/src/lib/commands.ts apps/desktop/src/lib/wikilinks.ts apps/desktop/src/lib/wikilinks.test.ts apps/desktop/src/components/WikiText.svelte apps/desktop/src/components/WikiText.test.ts
  git commit -m "feat: resolve wikilinks with aliases in UI"
  ```

---

### Task 2: Pending Create Flow In Shell And Entity Manager

**Files:**

- Modify: `apps/desktop/src/components/EntityForm.svelte`
- Modify: `apps/desktop/src/components/EntityManager.svelte`
- Modify: `apps/desktop/src/components/EntityManager.test.ts`
- Modify: `apps/desktop/src/shell/Shell.svelte`

**Interfaces:**

- Produces:
  - `type PendingCreate = { kind: EntityKind; name: string; sourceFindingId?: string }`
  - `EntityManager` prop `pendingCreate?: PendingCreate | null`
  - `EntityManager` prop `onPendingCreateConsumed?: () => void`
  - `EntityManager` prop `onPendingCreateSaved?: (sourceFindingId: string) => void`
  - `EntityForm` prop `initialName?: string`
  - `EntityForm` prop `ondirtychange?: (dirty: boolean) => void`

- [ ] **Step 1: Write failing `EntityManager` pending-create tests**

  Add tests to `apps/desktop/src/components/EntityManager.test.ts`:

  ```ts
  it("opens create form with pendingCreate name prefilled and consumes once", async () => {
    const onPendingCreateConsumed = vi.fn();
    render(EntityManager, {
      props: {
        campaignId: "camp1",
        kind: "location",
        pendingCreate: { kind: "location", name: "Moon Gate" },
        onPendingCreateConsumed,
      },
    });

    await waitFor(() => {
      expect((screen.getByLabelText(/^name$/i) as HTMLInputElement).value).toBe(
        "Moon Gate",
      );
    });
    expect(onPendingCreateConsumed).toHaveBeenCalledTimes(1);
  });

  it("calls onPendingCreateSaved after a Maintenance-origin pending create succeeds", async () => {
    const created = {
      ...mockNpc(),
      id: "loc1",
      kind: "location",
      name: "Moon Gate",
    };
    vi.mocked(commands.createEntity).mockResolvedValue(created);
    const onPendingCreateSaved = vi.fn();
    render(EntityManager, {
      props: {
        campaignId: "camp1",
        kind: "location",
        pendingCreate: {
          kind: "location",
          name: "Moon Gate",
          sourceFindingId: "lint_finding:1",
        },
        onPendingCreateSaved,
      },
    });

    await waitFor(() => screen.getByLabelText(/^name$/i));
    await fireEvent.submit(screen.getByRole("form"));

    await waitFor(() => expect(commands.createEntity).toHaveBeenCalled());
    expect(onPendingCreateSaved).toHaveBeenCalledWith("lint_finding:1");
  });
  ```

- [ ] **Step 2: Add dirty-state test**

  ```ts
  it("does not replace a dirty open form with a pending create without confirmation", async () => {
    const rendered = render(EntityManager, {
      props: { campaignId: "camp1", kind: "npc" },
    });
    await fireEvent.click(screen.getByRole("button", { name: /new npc/i }));
    await fireEvent.input(screen.getByLabelText(/^name$/i), {
      target: { value: "Unsaved NPC" },
    });

    await rendered.rerender({
      campaignId: "camp1",
      kind: "npc",
      pendingCreate: { kind: "npc", name: "Moon Gate" },
    });

    expect((screen.getByLabelText(/^name$/i) as HTMLInputElement).value).toBe(
      "Unsaved NPC",
    );
    expect(
      screen.getByRole("dialog", { name: /discard unsaved changes/i }),
    ).toBeTruthy();
  });
  ```

- [ ] **Step 3: Update `EntityForm.svelte`**

  Add props:

  ```ts
  initialName?: string;
  ondirtychange?: (dirty: boolean) => void;
  ```

  Seed create-mode name:

  ```ts
  let name = $derived(node?.name ?? initialName ?? "");
  ```

  Add a dirty notifier that includes every editable form field:

  ```ts
  const initialSnapshot = $derived.by(() =>
    JSON.stringify({
      name: node?.name ?? initialName ?? "",
      aliases: node?.aliases ?? [],
      summary: node?.summary ?? "",
      notes: node?.notes ?? "",
      dateStart: node?.date_start ?? "",
      dateEnd: node?.date_end ?? "",
      isOngoing: node?.is_ongoing ?? false,
      sequenceIndex: node?.sequence_index?.toString() ?? "",
      era: node?.era ?? "",
      durationLabel: node?.duration_label ?? "",
      sessionId: node?.session_id ?? "",
      playerName: node?.player_name ?? "",
      characterClass: node?.character_class ?? "",
      characterLevel: node?.character_level?.toString() ?? "",
      status: node?.status ?? "",
    }),
  );

  $effect(() => {
    const current = JSON.stringify({
      name,
      aliases,
      summary,
      notes,
      dateStart,
      dateEnd,
      isOngoing,
      sequenceIndex,
      era,
      durationLabel,
      sessionId,
      playerName,
      characterClass,
      characterLevel,
      status,
    });
    ondirtychange?.(current !== initialSnapshot);
  });
  ```

- [ ] **Step 4: Update `EntityManager.svelte` pending-create props**

  Define:

  ```ts
  type PendingCreate = {
    kind: EntityKind;
    name: string;
    sourceFindingId?: string;
  };
  ```

  Add props:

  ```ts
  pendingCreate?: PendingCreate | null;
  onPendingCreateConsumed?: () => void;
  onPendingCreateSaved?: (sourceFindingId: string) => void;
  ```

  Track:

  ```ts
  let pendingInitialName = $state<string | null>(null);
  let pendingSourceFindingId = $state<string | null>(null);
  let formDirty = $state(false);
  let blockedPendingCreate = $state<PendingCreate | null>(null);
  ```

  Add a helper:

  ```ts
  function openPendingCreate(request: PendingCreate) {
    if (showForm && formDirty) {
      blockedPendingCreate = request;
      return;
    }
    formNode = null;
    formError = null;
    pendingInitialName = request.name;
    pendingSourceFindingId = request.sourceFindingId ?? null;
    showForm = true;
    buildEntityMap();
    if (request.kind === "event") loadSessions();
  }
  ```

  Add an effect:

  ```ts
  $effect(() => {
    if (!pendingCreate || pendingCreate.kind !== kind) return;
    openPendingCreate(pendingCreate);
    onPendingCreateConsumed?.();
  });
  ```

  Pass to `EntityForm`:

  ```svelte
  <EntityForm
    initialName={pendingInitialName ?? undefined}
    ondirtychange={(dirty) => (formDirty = dirty)}
    ...
  />
  ```

  In `handleSave`, after a successful create:

  ```ts
  if (!formNode && pendingSourceFindingId) {
    onPendingCreateSaved?.(pendingSourceFindingId);
  }
  pendingInitialName = null;
  pendingSourceFindingId = null;
  formDirty = false;
  ```

- [ ] **Step 5: Add dirty confirmation UI**

  In `EntityManager.svelte`, if `blockedPendingCreate` is set, show a modal:

  ```svelte
  {#if blockedPendingCreate}
    <div class="modal-backdrop" use:modalBehavior={{ onClose: () => (blockedPendingCreate = null) }}>
      <div class="modal" role="dialog" aria-modal="true" aria-labelledby="pending-create-title">
        <h3 id="pending-create-title">Discard unsaved changes?</h3>
        <p>Creating [[{blockedPendingCreate.name}]] will replace the current form.</p>
        <button type="button" onclick={() => {
          const request = blockedPendingCreate;
          blockedPendingCreate = null;
          formDirty = false;
          if (request) openPendingCreate(request);
        }}>Discard and create</button>
        <button type="button" class="btn-ghost" onclick={() => (blockedPendingCreate = null)}>
          Keep editing
        </button>
      </div>
    </div>
  {/if}
  ```

- [ ] **Step 6: Update `Shell.svelte`**

  Add:

  ```ts
  type PendingCreate = {
    kind: EntityKind;
    name: string;
    sourceFindingId?: string;
  };

  let pendingCreate = $state<PendingCreate | null>(null);
  let createChooser = $state<{ name: string; sourceFindingId?: string } | null>(
    null,
  );

  function openCreateKindChooser(name: string, sourceFindingId?: string) {
    createChooser = { name, sourceFindingId };
  }

  function createFromWikilink(
    name: string,
    kind: EntityKind,
    sourceFindingId?: string,
  ) {
    pendingCreate = { kind, name, sourceFindingId };
    view = { kind: "notebook", category: KIND_TO_CATEGORY[kind] };
    createChooser = null;
  }
  ```

  Add chooser markup near the other shell-level modal/picker blocks:

  ```svelte
  {#if createChooser}
    <div class="modal-backdrop" use:modalBehavior={{ onClose: () => (createChooser = null) }}>
      <div class="modal" role="dialog" aria-modal="true" aria-labelledby="create-link-title">
        <h3 id="create-link-title">Create article for [[{createChooser.name}]]</h3>
        <div class="kind-grid">
          {#each Object.entries(KIND_TO_CATEGORY) as [kind] (kind)}
            <button
              type="button"
              onclick={() =>
                createFromWikilink(
                  createChooser?.name ?? '',
                  kind as EntityKind,
                  createChooser?.sourceFindingId,
                )}
            >
              {kind}
            </button>
          {/each}
        </div>
        <button type="button" class="btn-ghost" onclick={() => (createChooser = null)}>
          Cancel
        </button>
      </div>
    </div>
  {/if}
  ```

  Pass to the current `EntityManager`:

  ```svelte
  pendingCreate={pendingCreate && pendingCreate.kind === ENTITY_KIND_MAP[view.category]
    ? pendingCreate
    : null}
  onPendingCreateConsumed={() => (pendingCreate = null)}
  onPendingCreateSaved={async (findingId) => {
    await resolveLintFinding(findingId);
    await refreshMaintenanceCount();
  }}
  ```

  Import `resolveLintFinding` from `../lib/commands`.

- [ ] **Step 7: Run focused frontend tests**

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityManager.test.ts
  npx @sveltejs/mcp svelte-autofixer apps/desktop/src/components/EntityForm.svelte
  npx @sveltejs/mcp svelte-autofixer apps/desktop/src/components/EntityManager.svelte
  npx @sveltejs/mcp svelte-autofixer apps/desktop/src/shell/Shell.svelte
  ```

  Expected: tests pass; autofixer reports no required changes.

- [ ] **Step 8: Commit Task 2**

  ```bash
  git add apps/desktop/src/components/EntityForm.svelte apps/desktop/src/components/EntityManager.svelte apps/desktop/src/components/EntityManager.test.ts apps/desktop/src/shell/Shell.svelte
  git commit -m "feat: add wikilink create flow"
  ```

---

### Task 3: Maintenance Finding Presentation And Create Action

**Files:**

- Modify: `apps/desktop/src/views/MaintenanceView.svelte`
- Modify: `apps/desktop/src/views/MaintenanceView.test.ts`
- Modify: `apps/desktop/src/shell/Shell.svelte`

**Interfaces:**

- Consumes: Shell `createFromWikilink(name, kind, sourceFindingId?)`
- Produces: `MaintenanceView` prop
  `onCreateMissingArticle?: (name: string, sourceFindingId: string) => void`

- [ ] **Step 1: Add failing Maintenance tests for labels and actions**

  In `apps/desktop/src/views/MaintenanceView.test.ts`, replace/extend the
  existing broken-wikilink tests with:

  ```ts
  it("renders candidate-backed wikilinks as possible name mismatches", async () => {
    const onCreateMissingArticle = vi.fn();
    m.getLintFindings.mockResolvedValue([
      finding({
        id: "lint5",
        kind: "broken_wikilink",
        payload: {
          entity: "npc:mira",
          entity_name: "Mira",
          link_text: "The Quassars",
          candidates: [
            { id: "faction:q", name: "The Quassar Family", similarity: 0.92 },
          ],
        },
      }),
    ]);

    render(MaintenanceView, { props: { onCreateMissingArticle } });
    await fireEvent.click(screen.getByRole("tab", { name: "Findings" }));

    expect(
      await screen.findByText("Possible name mismatch"),
    ).toBeInTheDocument();
    expect(screen.getByText("[[The Quassars]] in Mira")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Use suggestion" }),
    ).toBeInTheDocument();
    await fireEvent.click(
      screen.getByRole("button", { name: "Create article" }),
    );
    expect(onCreateMissingArticle).toHaveBeenCalledWith(
      "The Quassars",
      "lint5",
    );
  });

  it("renders no-candidate wikilinks as missing articles", async () => {
    const onCreateMissingArticle = vi.fn();
    m.getLintFindings.mockResolvedValue([
      finding({
        id: "lint6",
        kind: "broken_wikilink",
        payload: {
          entity: "npc:mira",
          entity_name: "Mira",
          link_text: "Ashen Ferry",
          candidates: [],
        },
      }),
    ]);

    render(MaintenanceView, { props: { onCreateMissingArticle } });
    await fireEvent.click(screen.getByRole("tab", { name: "Findings" }));

    expect(await screen.findByText("Missing article")).toBeInTheDocument();
    expect(screen.getByText("[[Ashen Ferry]] in Mira")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Use suggestion" })).toBeNull();
    await fireEvent.click(
      screen.getByRole("button", { name: "Create article" }),
    );
    expect(onCreateMissingArticle).toHaveBeenCalledWith("Ashen Ferry", "lint6");
  });
  ```

- [ ] **Step 2: Update `MaintenanceView.svelte` props and grouping**

  Add prop:

  ```ts
  onCreateMissingArticle?: (name: string, sourceFindingId: string) => void;
  ```

  Add helpers:

  ```ts
  function hasCandidates(f: LintFinding): boolean {
    return candidatesOf(f).length > 0;
  }

  function brokenWikilinkLabel(f: LintFinding): string {
    return hasCandidates(f) ? "Possible name mismatch" : "Missing article";
  }
  ```

  For `broken_wikilink`, use a `Wikilinks` group heading and put the per-card
  label (`Possible name mismatch` or `Missing article`) inside each card.

- [ ] **Step 3: Rename actions**

  Candidate card actions:

  ```svelte
  <button type="button" disabled={busy === f.id} onclick={() => confirmSuggestion(f, candidatesOf(f)[0])}>
    Use suggestion
  </button>
  <button type="button" disabled={busy === f.id} onclick={() => onCreateMissingArticle?.(String(f.payload.link_text), f.id)}>
    Create article
  </button>
  ```

  No-candidate card actions:

  ```svelte
  <button type="button" disabled={busy === f.id} onclick={() => onCreateMissingArticle?.(String(f.payload.link_text), f.id)}>
    Create article
  </button>
  ```

  Rename `Open entity` to `Open source`.

- [ ] **Step 4: Wire Maintenance to Shell**

  In `Shell.svelte`, pass:

  ```svelte
  onCreateMissingArticle={(name, findingId) => openCreateKindChooser(name, findingId)}
  ```

- [ ] **Step 5: Run Maintenance tests**

  ```bash
  pnpm -C apps/desktop test:run src/views/MaintenanceView.test.ts
  npx @sveltejs/mcp svelte-autofixer apps/desktop/src/views/MaintenanceView.svelte
  ```

  Expected: tests pass; autofixer reports no required changes.

- [ ] **Step 6: Commit Task 3**

  ```bash
  git add apps/desktop/src/views/MaintenanceView.svelte apps/desktop/src/views/MaintenanceView.test.ts apps/desktop/src/shell/Shell.svelte
  git commit -m "feat: update wikilink findings"
  ```

---

### Task 4: Backend Graph Missing Wikilink Nodes

**Files:**

- Modify: `crates/chronacle-extraction/src/entity_service/types.rs`
- Modify: `crates/chronacle-extraction/src/entity_service/relations/graph.rs`
- Modify: `crates/chronacle-extraction/src/entity_service/relations/relations_tests.rs`

**Interfaces:**

- Produces synthetic graph nodes:
  - `GraphNodeRef { id: "missing_wikilink:<source_kind>:<source_id>:<normalized>", kind: "missing_wikilink", name, missing: Some(true), source_id: Some(source_id), source_kind: Some(source_kind) }`
- Produces synthetic graph edges:
  - `GraphEdge { from_id: source_id, from_kind: source_kind, to_id: missing_id, to_kind: "missing_wikilink", rel_type: "unresolved", notes: None }`

- [ ] **Step 1: Extend Rust `GraphNodeRef`**

  In `types.rs`:

  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
  pub struct GraphNodeRef {
      pub id: String,
      pub kind: String,
      pub name: String,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub missing: Option<bool>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub source_id: Option<String>,
      #[serde(skip_serializing_if = "Option::is_none")]
      pub source_kind: Option<String>,
  }
  ```

  Update every existing `GraphNodeRef { ... }` construction to set the new fields
  to `None`.

- [ ] **Step 2: Write failing backend graph tests**

  Add to `relations_tests.rs`:

  ```rust
  #[tokio::test]
  async fn graph_includes_missing_wikilink_node_from_notes() {
      let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
          .await
          .unwrap();
      db.use_ns("test").use_db("test").await.unwrap();
      chronacle_db::run_migrations(&db).await.unwrap();
      db.query(
          "CREATE campaign:camp1 SET name='Test', system='5e', created_at=time::now(), updated_at=time::now();",
      )
      .await
      .unwrap()
      .check()
      .unwrap();
      let npc = create(
          &db,
          Some("camp1"),
          None,
          EntityKind::Npc,
          EntityInput {
              name: "Mira".to_string(),
              notes: Some("Ask [[Moon Gate]] about this.".to_string()),
              ..Default::default()
          },
      )
      .await
      .unwrap();

      let graph = crate::entity_service::get_entity_graph(&db, &npc.id, "npc", 1)
          .await
          .unwrap();

      assert!(graph.nodes.iter().any(|n| {
          n.kind == "missing_wikilink"
              && n.name == "Moon Gate"
              && n.missing == Some(true)
              && n.source_id.as_deref() == Some(npc.id.as_str())
      }));
      assert!(graph.edges.iter().any(|e| {
          e.from_id == npc.id
              && e.from_kind == "npc"
              && e.to_kind == "missing_wikilink"
              && e.rel_type == "unresolved"
      }));
  }

  #[tokio::test]
  async fn graph_skips_missing_node_when_wikilink_resolves_by_alias() {
      let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
          .await
          .unwrap();
      db.use_ns("test").use_db("test").await.unwrap();
      chronacle_db::run_migrations(&db).await.unwrap();
      db.query(
          "CREATE campaign:camp1 SET name='Test', system='5e', created_at=time::now(), updated_at=time::now();",
      )
      .await
      .unwrap()
      .check()
      .unwrap();
      let target = create(
          &db,
          Some("camp1"),
          None,
          EntityKind::Location,
          EntityInput {
              name: "The Moon Gate".to_string(),
              aliases: Some(vec!["Selene Door".to_string()]),
              ..Default::default()
          },
      )
      .await
      .unwrap();
      let npc = create(
          &db,
          Some("camp1"),
          None,
          EntityKind::Npc,
          EntityInput {
              name: "Mira".to_string(),
              notes: Some("Ask [[Selene Door]] about this.".to_string()),
              ..Default::default()
          },
      )
      .await
      .unwrap();

      let graph = crate::entity_service::get_entity_graph(&db, &npc.id, "npc", 1)
          .await
          .unwrap();

      assert!(!graph.nodes.iter().any(|n| n.kind == "missing_wikilink"));
      assert!(graph.nodes.iter().any(|n| n.id == target.id && n.kind == "location"));
  }
  ```

- [ ] **Step 3: Implement missing-node assembly in `graph.rs`**

  Import resolver helpers and the existing scope projection:

  ```rust
  use super::super::SELECT_SCOPE_ALIASES;
  use crate::naming;
  use crate::wikilink::{query_all_entity_names, resolve_exact, WikilinkScope};
  use regex::Regex;
  use std::sync::LazyLock;
  ```

  Add a regex:

  ```rust
  static WIKILINK_RE: LazyLock<Regex> =
      LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").expect("wikilink regex is valid"));
  ```

  Determine scope by projecting `campaign` / `collection` with
  `SELECT_SCOPE_ALIASES`, matching `wikilink_backfill.rs` and CRUD reads:

  ```rust
  #[derive(Deserialize)]
  struct SourceTextRow {
      id: Thing,
      notes: Option<String>,
      codex_article: Option<String>,
      campaign: Option<Thing>,
      collection: Option<Thing>,
  }
  ```

  Query every concrete node already in the graph for `notes`, `codex_article`,
  and projected scope:

  ```rust
  let query = format!(
      "SELECT id, notes, codex_article, {SELECT_SCOPE_ALIASES} \
       FROM {table} WHERE id IN $ids AND vault_deleted != true"
  );
  ```

  Then for each row:

  ```rust
  let scope_collection = row.collection.as_ref().map(|t| t.id.to_raw());
  let scope_campaign = row.campaign.as_ref().map(|t| t.id.to_raw());
  let scope = if let Some(collection_id) = scope_collection.as_deref() {
      WikilinkScope::Collection { collection_id }
  } else if let Some(campaign_id) = scope_campaign.as_deref() {
      WikilinkScope::Campaign { campaign_id }
  } else {
      continue;
  };
  let names = query_all_entity_names(db, &scope)
      .await
      .map_err(|e| EntityError::Database {
          message: e.to_string(),
      })?;
  for link_text in extracted_links(&row.notes, &row.codex_article) {
      if resolve_exact(&link_text, &names).is_some() {
          continue;
      }
      let missing_key = naming::normalize(&link_text);
      let missing_id = format!("missing_wikilink:{}:{}:{}", row.id.tb, row.id.id.to_raw(), missing_key);
      nodes.push(GraphNodeRef {
          id: missing_id.clone(),
          kind: "missing_wikilink".to_string(),
          name: link_text.clone(),
          missing: Some(true),
          source_id: Some(row.id.id.to_raw()),
          source_kind: Some(row.id.tb.clone()),
      });
      edges.push(GraphEdge {
          from_id: row.id.id.to_raw(),
          from_kind: row.id.tb.clone(),
          to_id: missing_id,
          to_kind: "missing_wikilink".to_string(),
          rel_type: "unresolved".to_string(),
          notes: None,
      });
  }
  ```

  Keep synthetic missing nodes out of the DB name-resolution query; they are added
  after persisted nodes are resolved.

- [ ] **Step 4: Run backend tests**

  ```bash
  cargo test -p chronacle-extraction entity_service::relations
  ```

  Expected: new and existing relation tests pass.

- [ ] **Step 5: Commit Task 4**

  ```bash
  git add crates/chronacle-extraction/src/entity_service/types.rs crates/chronacle-extraction/src/entity_service/relations/graph.rs crates/chronacle-extraction/src/entity_service/relations/relations_tests.rs
  git commit -m "feat: show unresolved wikilinks in graph"
  ```

---

### Task 5: Graph Frontend Missing Nodes

**Files:**

- Modify: `apps/desktop/src/components/EntityGraph.svelte`
- Modify: `apps/desktop/src/components/EntityGraph.test.ts`
- Modify: `apps/desktop/src/shell/Shell.svelte`

**Interfaces:**

- Consumes: `GraphNodeRef.missing === true` or `kind === "missing_wikilink"`
- Produces: `EntityGraph` prop `onMissingLinkClick?: (name: string) => void`

- [ ] **Step 1: Add failing graph frontend tests**

  In `EntityGraph.test.ts`, add:

  ```ts
  it("renders missing wikilink nodes distinctly and opens create flow on click", async () => {
    const onMissingLinkClick = vi.fn();
    m.getEntityGraph.mockResolvedValueOnce({
      nodes: [
        { id: "mira", kind: "npc", name: "Mira" },
        {
          id: "missing_wikilink:npc:mira:moon gate",
          kind: "missing_wikilink",
          name: "Moon Gate",
          missing: true,
          source_id: "mira",
          source_kind: "npc",
        },
      ],
      edges: [
        {
          from_id: "mira",
          from_kind: "npc",
          to_id: "missing_wikilink:npc:mira:moon gate",
          to_kind: "missing_wikilink",
          rel_type: "unresolved",
          notes: null,
        },
      ],
    });

    const { container } = render(EntityGraph, {
      entityId: "mira",
      entityKind: "npc",
      onMissingLinkClick,
    });

    expect(await screen.findByText("[[Moon Gate]]")).toBeTruthy();
    const missingNode = container.querySelector(
      '[data-missing="true"]',
    ) as Element;
    expect(missingNode).toBeTruthy();

    await fireEvent.click(missingNode);
    expect(onMissingLinkClick).toHaveBeenCalledWith("Moon Gate");
  });
  ```

- [ ] **Step 2: Update `EntityGraph.svelte` props and helpers**

  Add prop:

  ```ts
  onMissingLinkClick?: (name: string) => void;
  ```

  Helper:

  ```ts
  function isMissingNode(n: GraphNodeRef | SimNode): boolean {
    return n.missing === true || n.kind === "missing_wikilink";
  }
  ```

  In `onNodeClick`:

  ```ts
  const n = liveNode(id);
  if (!n) return;
  if (isMissingNode(n)) {
    onMissingLinkClick?.(n.name);
    return;
  }
  void recenter(n.id, n.kind);
  ```

- [ ] **Step 3: Render missing node styling**

  In the node group:

  ```svelte
  data-missing={isMissingNode(n) ? 'true' : undefined}
  class:node--missing={isMissingNode(n)}
  ```

  Circle:

  ```svelte
  <circle
    cx={0}
    cy={0}
    r={isMissingNode(n) ? 12 : n.id === centerId ? 16 : 10}
    class={[
      'node-circle',
      n.id === centerId && 'node-circle--center',
      isMissingNode(n) && 'node-circle--missing',
    ]}
    fill={isMissingNode(n) ? 'transparent' : kindColor(n.kind)}
  />
  ```

  Label:

  ```svelte
  >{isMissingNode(n) ? `[[${n.name}]]` : n.name}</text>
  ```

  Hide expand:

  ```svelte
  {#if n.id !== centerId && !isMissingNode(n)}
  ```

  CSS:

  ```css
  .node-circle--missing {
    stroke: var(--warning, #f8c35b);
    stroke-width: 2;
    stroke-dasharray: 4 3;
    filter: drop-shadow(0 0 5px rgba(248, 195, 91, 0.25));
  }

  .node--missing .node-label {
    fill: var(--warning, #f8c35b);
    font-style: italic;
  }
  ```

- [ ] **Step 4: Wire shell graph callback**

  Where `Shell.svelte` renders `EntityGraph`, pass:

  ```svelte
  onMissingLinkClick={(name) => openCreateKindChooser(name)}
  ```

- [ ] **Step 5: Run graph tests and autofixer**

  ```bash
  pnpm -C apps/desktop test:run src/components/EntityGraph.test.ts
  npx @sveltejs/mcp svelte-autofixer apps/desktop/src/components/EntityGraph.svelte
  ```

  Expected: tests pass; autofixer reports no required changes.

- [ ] **Step 6: Commit Task 5**

  ```bash
  git add apps/desktop/src/components/EntityGraph.svelte apps/desktop/src/components/EntityGraph.test.ts apps/desktop/src/shell/Shell.svelte
  git commit -m "feat: create from graph wikilinks"
  ```

---

### Task 6: Acceptance Scenarios And Full Verification

**Files:**

- Create or modify: `apps/desktop/tests/e2e/features/unresolved-wikilinks.feature`
- Modify: backend step definitions under `apps/desktop/tests/e2e/backend/steps/`
  if an existing feature file cannot reuse current steps.

**Interfaces:**

- Produces BDD coverage for:
  - create missing article from text link
  - choose suggestion or new article in Maintenance
  - no-candidate finding reads as missing article
  - create missing article from graph node

- [ ] **Step 1: Add feature scenarios**

  Create `apps/desktop/tests/e2e/features/unresolved-wikilinks.feature`:

  ```gherkin
  Feature: Unresolved wikilink creation

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

    Scenario: Create a missing article from the relationship graph
      Given an NPC article contains the unresolved link "[[Moon Gate]]"
      When the GM opens that NPC's relationship graph
      Then the graph shows a distinct missing-link node named "[[Moon Gate]]"
      When the GM clicks the missing-link node
      And creates a Location named "Moon Gate"
      Then the graph shows "Moon Gate" as a normal Location node
  ```

- [ ] **Step 2: Add or reuse step definitions**

  Search existing steps first:

  ```bash
  rg -n "Maintenance|wikilink|relationship graph|article contains|creates a Location" apps/desktop/tests/e2e/backend/steps
  ```

  If missing, add focused step definitions that seed the SurrealDB service layer
  directly, matching current backend Playwright patterns.

- [ ] **Step 3: Run focused frontend tests**

  ```bash
  pnpm -C apps/desktop test:run src/lib/wikilinks.test.ts src/components/WikiText.test.ts src/components/EntityManager.test.ts src/views/MaintenanceView.test.ts src/components/EntityGraph.test.ts
  ```

  Expected: all pass.

- [ ] **Step 4: Run Rust graph tests**

  ```bash
  cargo test -p chronacle-extraction entity_service::relations
  ```

  Expected: all pass.

- [ ] **Step 5: Run typecheck and lint**

  ```bash
  pnpm -C apps/desktop typecheck
  pnpm -C apps/desktop lint
  ```

  Expected: both pass.

- [ ] **Step 6: Run broader test suites**

  ```bash
  pnpm -C apps/desktop test:run
  cargo test --workspace
  ```

  Expected: both pass.

- [ ] **Step 7: Format**

  ```bash
  cargo fmt --all
  apps/desktop/node_modules/.bin/prettier --write apps/desktop/src apps/desktop/tests/e2e/features docs/superpowers/plans/2026-07-17-unresolved-wikilink-create-article.md
  ```

  Expected: no unintended files outside this feature are formatted.

- [ ] **Step 8: Commit Task 6**

  ```bash
  git add apps/desktop/tests/e2e/features/unresolved-wikilinks.feature apps/desktop/tests/e2e/backend/steps
  git commit -m "test: cover unresolved wikilink creation"
  ```

- [ ] **Step 9: Push branch**

  ```bash
  git push
  ```

  Expected: pushes to the tracked upstream
  `origin/agent/unresolved-wikilink-article-creation`.

---

## Plan Self-Review

- Spec coverage: text click flow, Maintenance labels/actions, graph missing nodes,
  shared creation, frontend alias/normalization rendering, backend graph payload,
  and tests are covered.
- Placeholder scan: no `TBD`, `TODO`, or deferred implementation language remains.
- Type consistency: `PendingCreate`, `onMissingLinkClick`, `GraphNodeRef.missing`,
  and `missing_wikilink` are named consistently across tasks.
