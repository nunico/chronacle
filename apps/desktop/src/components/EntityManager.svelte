<script lang="ts">
  import { SvelteMap } from 'svelte/reactivity';
  import {
    getEntities,
    getEntity,
    createEntity,
    updateEntity,
    softDeleteEntity,
    getSessions,
    compileEntity,
    type EntityKind,
    type GraphNode,
    type EntityInput,
    type EntityError,
    type Session,
  } from '../lib/commands';
  import EntityForm from './EntityForm.svelte';
  import WikiText from './WikiText.svelte';
  import { modalBehavior } from '../lib/actions/modal';
  import { buildWikiLinkEntityMap } from '../lib/wikilinks';

  interface PendingCreate {
    kind: EntityKind;
    name: string;
    sourceFindingId?: string;
  }

  interface Props {
    campaignId: string;
    kind: EntityKind;
    /// Bumped by the `c` shortcut to start creating a new entity.
    createNonce?: number;
    /// Set to an entity id to open its edit form once entities are loaded (deep-link).
    openId?: string | null;
    /// Called immediately after the deep-link edit form is opened so the caller
    /// can clear openId and prevent the effect from re-firing on entity list mutations.
    onOpenIdConsumed?: () => void;
    /// Called when the user clicks the "Graph" button on an entity row.
    onViewGraph?: (node: GraphNode) => void;
    /// Called when the user clicks a related entity in the Relationships section.
    onOpenEntity?: (id: string, kind: string) => void;
    pendingCreate?: PendingCreate | null;
    onPendingCreateConsumed?: () => void;
    onPendingCreateSaved?: (sourceFindingId: string) => void;
    onMissingLinkClick?: (name: string) => void;
  }

  let {
    campaignId,
    kind,
    createNonce = 0,
    openId = null,
    onOpenIdConsumed,
    onViewGraph,
    onOpenEntity,
    pendingCreate = null,
    onPendingCreateConsumed,
    onPendingCreateSaved,
    onMissingLinkClick,
  }: Props = $props();

  const KIND_LABEL: Record<EntityKind, string> = {
    npc: 'NPC',
    location: 'Location',
    faction: 'Faction',
    creature: 'Creature',
    item: 'Item',
    event: 'Event',
    player_character: 'PC',
    misc: 'Misc',
  };
  let entities = $state<GraphNode[]>([]);
  let loading = $state(false);
  let formNode = $state<GraphNode | null>(null); // null = create, non-null = edit
  let showForm = $state(false);
  let formError = $state<EntityError | null>(null);
  let toast = $state<string | null>(null);
  let deleteConfirm = $state<GraphNode | null>(null);
  // SvelteMap is inherently reactive — no $state wrapper needed
  let entityMap = new SvelteMap<string, { id: string; kind: string }>();
  let sessions = $state<Session[]>([]);
  let recompiling = $state(false);
  let pendingInitialName = $state<string | null>(null);
  let pendingSourceFindingId = $state<string | null>(null);
  let formDirty = $state(false);
  let blockedPendingCreate = $state<PendingCreate | null>(null);
  let consumedPendingCreate = $state<PendingCreate | null>(null);
  let loadedScope = $state<string | null>(null);

  async function loadEntities() {
    loading = true;
    try {
      entities = await getEntities(campaignId, kind);
    } catch (e) {
      showToastMsg((e as EntityError).message ?? 'Failed to load entities');
    } finally {
      loading = false;
    }
  }

  async function buildEntityMap() {
    const allKinds: EntityKind[] = [
      'npc',
      'location',
      'faction',
      'creature',
      'item',
      'event',
      'player_character',
      'misc',
    ];
    try {
      const results = await Promise.all(
        allKinds.map((k) => getEntities(campaignId, k).catch(() => [])),
      );
      entityMap.clear();
      buildWikiLinkEntityMap(results.flat()).forEach((target, key) => {
        entityMap.set(key, target);
      });
    } catch {
      // ignore — entity map is best-effort
    }
  }

  async function loadSessions() {
    try {
      sessions = await getSessions(campaignId);
    } catch {
      sessions = [];
    }
  }

  function openCreate() {
    formNode = null;
    formError = null;
    pendingInitialName = null;
    pendingSourceFindingId = null;
    formDirty = false;
    showForm = true;
    buildEntityMap();
    if (kind === 'event') loadSessions();
  }

  function openPendingCreate(request: PendingCreate) {
    if (showForm && formDirty) {
      blockedPendingCreate = request;
      return;
    }
    formNode = null;
    formError = null;
    pendingInitialName = request.name;
    pendingSourceFindingId = request.sourceFindingId ?? null;
    formDirty = false;
    showForm = true;
    buildEntityMap();
    if (request.kind === 'event') loadSessions();
  }

  // The `c` shortcut bumps createNonce; open the create form.
  $effect(() => {
    if (createNonce > 0) openCreate();
  });

  // Deep-link: when asked to open a specific entity, open its edit form once
  // it's present in the loaded list. Immediately invokes onOpenIdConsumed so
  // the caller can clear openId — preventing the effect from re-firing when the
  // entities array is mutated by a subsequent save or delete.
  $effect(() => {
    if (!openId) return;
    const node = entities.find((n) => n.id === openId);
    if (node) {
      openEdit(node);
      onOpenIdConsumed?.();
    }
  });

  function openEdit(node: GraphNode) {
    formNode = node;
    formError = null;
    pendingInitialName = null;
    pendingSourceFindingId = null;
    formDirty = false;
    showForm = true;
    buildEntityMap();
    if (kind === 'event') loadSessions();
  }

  async function handleSave(input: EntityInput) {
    formError = null;
    try {
      if (formNode) {
        const updated = await updateEntity(formNode.id, kind, input);
        entities = entities.map((e) => (e.id === updated.id ? updated : e));
      } else {
        const created = await createEntity(campaignId, kind, input);
        entities = [created, ...entities];
        if (pendingSourceFindingId) {
          onPendingCreateSaved?.(pendingSourceFindingId);
        }
      }
      pendingInitialName = null;
      pendingSourceFindingId = null;
      formDirty = false;
      showForm = false;
    } catch (e) {
      const err = e as EntityError;
      if (err.code === 'VALIDATION') {
        formError = err;
      } else if (err.code === 'NOT_FOUND') {
        showToastMsg('Entity no longer exists — refresh the list');
        showForm = false;
        await loadEntities();
      } else {
        showToastMsg(err.message ?? 'An error occurred');
      }
    }
  }

  async function confirmDelete(node: GraphNode) {
    try {
      await softDeleteEntity(node.id, kind);
      entities = entities.filter((e) => e.id !== node.id);
    } catch (e) {
      showToastMsg((e as EntityError).message ?? 'Failed to delete');
    } finally {
      deleteConfirm = null;
    }
  }

  async function handleRecompile() {
    if (!formNode || recompiling) return;
    recompiling = true;
    try {
      const ok = await compileEntity(kind, formNode.id);
      if (!ok) {
        showToastMsg('No source context found — article unchanged');
        return;
      }
      const refreshed = await getEntity(formNode.id, kind);
      formNode = refreshed;
      entities = entities.map((e) => (e.id === refreshed.id ? refreshed : e));
    } catch (e) {
      showToastMsg((e as EntityError).message ?? 'Failed to recompile article');
    } finally {
      recompiling = false;
    }
  }

  function showToastMsg(msg: string) {
    toast = msg;
    setTimeout(() => {
      toast = null;
    }, 4000);
  }

  // Reset form and reload when kind or campaign changes
  $effect(() => {
    const scope = `${campaignId}:${kind}`;
    if (scope === loadedScope) return;
    loadedScope = scope;
    showForm = false;
    formNode = null;
    formError = null;
    pendingInitialName = null;
    pendingSourceFindingId = null;
    formDirty = false;
    blockedPendingCreate = null;
    if (campaignId) loadEntities();
  });

  $effect(() => {
    if (!pendingCreate || pendingCreate.kind !== kind || pendingCreate === consumedPendingCreate) {
      return;
    }
    consumedPendingCreate = pendingCreate;
    openPendingCreate(pendingCreate);
    onPendingCreateConsumed?.();
  });
</script>

<div class="entity-manager">
  <div class="content">
    <!-- List panel -->
    <div class="list-panel">
      <div class="list-header">
        <button class="btn-primary" onclick={openCreate}>
          + New {KIND_LABEL[kind]}
        </button>
      </div>

      {#if loading}
        <p class="muted">Loading…</p>
      {:else if entities.length === 0}
        <p class="muted">No {KIND_LABEL[kind].toLowerCase()}s yet.</p>
      {:else}
        <ul class="entity-list">
          {#each entities as node (node.id)}
            <li class="entity-row" class:selected={formNode?.id === node.id}>
              <button class="entity-name" onclick={() => openEdit(node)}>{node.name}</button>
              {#if onViewGraph}
                <button
                  class="btn-icon entity-graph-btn"
                  title="View relationships"
                  onclick={() => onViewGraph(node)}>Graph</button
                >
              {/if}
              <button
                class="btn-icon delete"
                aria-label="Delete {node.name}"
                onclick={() => {
                  deleteConfirm = node;
                }}>×</button
              >
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Form panel -->
    {#if showForm}
      <div class="form-panel" oninput={() => (formDirty = true)} onchange={() => (formDirty = true)}>
        {#if formNode?.notes}
          <div class="notes-preview">
            <WikiText text={formNode.notes} entities={entityMap} {onMissingLinkClick} />
          </div>
        {/if}
        {#if formNode}
          <div class="codex-section">
            <div class="codex-header">
              <h3>Codex Article</h3>
              {#if formNode.codex_stale !== false}
                <span class="chip-stale">Stale</span>
              {/if}
              <button
                class="btn-ghost btn-recompile"
                type="button"
                aria-label="Recompile article"
                disabled={recompiling}
                onclick={handleRecompile}
              >
                {recompiling ? 'Recompiling…' : 'Recompile'}
              </button>
            </div>
            <div class="codex-article">
              {#if formNode.codex_article}
                <WikiText
                  text={formNode.codex_article}
                  entities={entityMap}
                  onEntityClick={onOpenEntity}
                  {onMissingLinkClick}
                />
              {:else}
                <p class="muted">No article compiled yet</p>
              {/if}
            </div>
          </div>
        {/if}
        <EntityForm
          {kind}
          node={formNode}
          error={formError}
          initialName={pendingInitialName ?? undefined}
          ondirtychange={(dirty) => (formDirty = dirty)}
          sessions={kind === 'event' ? sessions : []}
          {entityMap}
          onsave={handleSave}
          oncancel={() => {
            showForm = false;
            formNode = null;
            pendingInitialName = null;
            pendingSourceFindingId = null;
            formDirty = false;
          }}
          {onOpenEntity}
        />
      </div>
    {/if}
  </div>

  <!-- Delete confirmation -->
  {#if deleteConfirm}
    <div
      class="overlay"
      role="dialog"
      aria-modal="true"
      use:modalBehavior={{
        onClose: () => {
          deleteConfirm = null;
        },
      }}
    >
      <div class="confirm-box">
        <p>
          Remove <strong>{deleteConfirm.name}</strong>? It disappears from Chronacle and your
          vault. (Files you edited by hand in the vault are kept.)
        </p>
        <div class="actions">
          <button class="btn-danger" onclick={() => confirmDelete(deleteConfirm as GraphNode)}
            >Delete</button
          >
          <button
            class="btn-ghost"
            onclick={() => {
              deleteConfirm = null;
            }}>Cancel</button
          >
        </div>
      </div>
    </div>
  {/if}

  {#if blockedPendingCreate}
    <div
      class="overlay"
      use:modalBehavior={{
        onClose: () => {
          blockedPendingCreate = null;
        },
      }}
    >
      <div
        class="confirm-box"
        role="dialog"
        aria-modal="true"
        aria-labelledby="pending-create-title"
      >
        <h3 id="pending-create-title">Discard unsaved changes?</h3>
        <p>Creating [[{blockedPendingCreate.name}]] will replace the current form.</p>
        <div class="actions">
          <button
            type="button"
            class="btn-danger"
            onclick={() => {
              const request = blockedPendingCreate;
              blockedPendingCreate = null;
              formDirty = false;
              if (request) openPendingCreate(request);
            }}>Discard and create</button
          >
          <button
            type="button"
            class="btn-ghost"
            onclick={() => {
              blockedPendingCreate = null;
            }}>Keep editing</button
          >
        </div>
      </div>
    </div>
  {/if}

  <!-- Toast -->
  {#if toast}
    <div class="toast" role="alert">{toast}</div>
  {/if}
</div>

<style>
  .entity-manager {
    display: flex;
    flex-direction: column;
    gap: 0;
    min-height: 60vh;
  }
  .content {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .list-panel {
    flex: 0 0 260px;
    border-right: 1px solid var(--line);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .list-header {
    padding: 10px;
    border-bottom: 1px solid var(--line);
  }
  .entity-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .entity-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    border-bottom: 1px solid var(--line);
  }
  .entity-row.selected {
    background: var(--bg-panel-2);
  }
  .entity-name {
    flex: 1;
    background: none;
    border: none;
    color: var(--fg-1);
    text-align: left;
    padding: 10px 4px;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .btn-icon {
    background: none;
    border: none;
    color: var(--fg-4);
    cursor: pointer;
    font-size: 1rem;
  }
  .btn-icon.delete:hover {
    color: var(--danger);
  }
  .form-panel {
    flex: 1;
    padding: 16px;
    overflow-y: auto;
  }
  .notes-preview {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 12px;
    margin-bottom: 12px;
    font-size: 0.9rem;
    color: var(--fg-2);
    line-height: 1.5;
  }
  .muted {
    color: var(--fg-3);
    font-size: 0.85rem;
    padding: 16px;
  }
  .codex-section {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 12px;
    margin-bottom: 12px;
  }
  .codex-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .codex-header h3 {
    margin: 0;
    font-size: 0.9rem;
    color: var(--fg-1);
    flex: 1;
  }
  .chip-stale {
    background: var(--danger);
    color: var(--bg-abyss);
    border-radius: 10px;
    padding: 2px 8px;
    font-size: 0.75rem;
    font-weight: 600;
  }
  .btn-recompile {
    font-size: 0.8rem;
    padding: 4px 10px;
  }
  .codex-article {
    white-space: pre-wrap;
    font-size: 0.9rem;
    color: var(--fg-2);
    line-height: 1.5;
  }
  .btn-primary {
    background: var(--violet-300);
    color: var(--bg-abyss);
    border: none;
    border-radius: 6px;
    padding: 6px 12px;
    cursor: pointer;
    font-size: 0.85rem;
    font-weight: 600;
  }
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .confirm-box {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 20px;
    max-width: 360px;
    width: 90%;
  }
  .confirm-box p {
    margin: 0 0 16px;
    color: var(--fg-1);
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn-danger {
    background: var(--danger);
    color: var(--bg-abyss);
    border: none;
    border-radius: 6px;
    padding: 6px 14px;
    cursor: pointer;
    font-weight: 600;
  }
  .btn-ghost {
    background: transparent;
    color: var(--fg-3);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 6px 14px;
    cursor: pointer;
  }
  .toast {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-panel-2);
    color: var(--fg-1);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 10px 20px;
    z-index: 200;
    font-size: 0.9rem;
  }
</style>
