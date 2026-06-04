<script lang="ts">
  import {
    getEntities, createEntity, updateEntity, deleteEntity,
    type EntityKind, type GraphNode, type EntityInput, type EntityError,
  } from '../lib/commands';
  import EntityForm from './EntityForm.svelte';

  interface Props {
    campaignId: string;
  }

  let { campaignId }: Props = $props();

  type Tab = { kind: EntityKind; label: string };
  const TABS: Tab[] = [
    { kind: 'npc',              label: 'NPC' },
    { kind: 'location',         label: 'Location' },
    { kind: 'faction',          label: 'Faction' },
    { kind: 'creature',         label: 'Creature' },
    { kind: 'item',             label: 'Item' },
    { kind: 'event',            label: 'Event' },
    { kind: 'player_character', label: 'PC' },
    { kind: 'misc',             label: 'Misc' },
  ];

  let activeKind = $state<EntityKind>('npc');
  let entities = $state<GraphNode[]>([]);
  let loading = $state(false);
  let formNode = $state<GraphNode | null>(null); // null = create, non-null = edit
  let showForm = $state(false);
  let formError = $state<EntityError | null>(null);
  let toast = $state<string | null>(null);
  let deleteConfirm = $state<GraphNode | null>(null);

  async function loadEntities(kind: EntityKind) {
    loading = true;
    try {
      entities = await getEntities(campaignId, kind);
    } catch (e) {
      showToastMsg((e as EntityError).message ?? 'Failed to load entities');
    } finally {
      loading = false;
    }
  }

  function selectTab(kind: EntityKind) {
    activeKind = kind;
    showForm = false;
    formNode = null;
    formError = null;
    // $effect fires automatically because activeKind changed
  }

  function openCreate() {
    formNode = null;
    formError = null;
    showForm = true;
  }

  function openEdit(node: GraphNode) {
    formNode = node;
    formError = null;
    showForm = true;
  }

  async function handleSave(input: EntityInput) {
    formError = null;
    try {
      if (formNode) {
        const updated = await updateEntity(formNode.id, activeKind, input);
        entities = entities.map(e => e.id === updated.id ? updated : e);
      } else {
        const created = await createEntity(campaignId, activeKind, input);
        entities = [created, ...entities];
      }
      showForm = false;
    } catch (e) {
      const err = e as EntityError;
      if (err.code === 'VALIDATION') {
        formError = err;
      } else if (err.code === 'NOT_FOUND') {
        showToastMsg('Entity no longer exists — refresh the list');
        showForm = false;
        await loadEntities(activeKind);
      } else {
        showToastMsg(err.message ?? 'An error occurred');
      }
    }
  }

  async function confirmDelete(node: GraphNode) {
    try {
      await deleteEntity(node.id, activeKind);
      entities = entities.filter(e => e.id !== node.id);
    } catch (e) {
      showToastMsg((e as EntityError).message ?? 'Failed to delete');
    } finally {
      deleteConfirm = null;
    }
  }

  function showToastMsg(msg: string) {
    toast = msg;
    setTimeout(() => { toast = null; }, 4000);
  }

  // Reload on kind or campaign change
  $effect(() => {
    if (campaignId) loadEntities(activeKind);
  });
</script>

<div class="entity-manager">
  <!-- Type tabs -->
  <div class="type-tabs" role="tablist">
    {#each TABS as tab (tab.kind)}
      <button
        role="tab"
        aria-selected={activeKind === tab.kind}
        class="type-tab"
        class:active={activeKind === tab.kind}
        onclick={() => selectTab(tab.kind)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="content">
    <!-- List panel -->
    <div class="list-panel">
      <div class="list-header">
        <button class="btn-primary" onclick={openCreate}>
          + New {TABS.find(t => t.kind === activeKind)?.label}
        </button>
      </div>

      {#if loading}
        <p class="muted">Loading…</p>
      {:else if entities.length === 0}
        <p class="muted">No {TABS.find(t => t.kind === activeKind)?.label?.toLowerCase()}s yet.</p>
      {:else}
        <ul class="entity-list">
          {#each entities as node (node.id)}
            <li class="entity-row" class:selected={formNode?.id === node.id}>
              <button class="entity-name" onclick={() => openEdit(node)}>{node.name}</button>
              <button
                class="btn-icon delete"
                aria-label="Delete {node.name}"
                onclick={() => { deleteConfirm = node; }}
              >×</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Form panel -->
    {#if showForm}
      <div class="form-panel">
        <EntityForm
          kind={activeKind}
          node={formNode}
          error={formError}
          onsave={handleSave}
          oncancel={() => { showForm = false; formNode = null; }}
        />
      </div>
    {/if}
  </div>

  <!-- Delete confirmation -->
  {#if deleteConfirm}
    <div class="overlay" role="dialog" aria-modal="true">
      <div class="confirm-box">
        <p>Delete <strong>{deleteConfirm.name}</strong>? This cannot be undone.</p>
        <div class="actions">
          <button class="btn-danger" onclick={() => confirmDelete(deleteConfirm!)}>Delete</button>
          <button class="btn-ghost" onclick={() => { deleteConfirm = null; }}>Cancel</button>
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
  .entity-manager { display: flex; flex-direction: column; gap: 0; height: 100%; }
  .type-tabs { display: flex; gap: 2px; border-bottom: 1px solid var(--border, #333); padding: 0 8px; }
  .type-tab {
    background: none; border: none; color: var(--text-secondary, #aaa);
    padding: 8px 12px; cursor: pointer; font-size: 0.85rem; border-bottom: 2px solid transparent;
    margin-bottom: -1px;
  }
  .type-tab.active { color: var(--text-primary, #fff); border-bottom-color: var(--accent, #cba6f7); }
  .content { display: flex; flex: 1; overflow: hidden; }
  .list-panel { flex: 0 0 260px; border-right: 1px solid var(--border, #333); overflow-y: auto; display: flex; flex-direction: column; }
  .list-header { padding: 10px; border-bottom: 1px solid var(--border, #333); }
  .entity-list { list-style: none; margin: 0; padding: 0; }
  .entity-row {
    display: flex; align-items: center; gap: 4px; padding: 0 8px;
    border-bottom: 1px solid var(--border, #222);
  }
  .entity-row.selected { background: var(--surface-2, #1e1e2e); }
  .entity-name {
    flex: 1; background: none; border: none; color: var(--text-primary, #fff);
    text-align: left; padding: 10px 4px; cursor: pointer; font-size: 0.9rem;
  }
  .btn-icon { background: none; border: none; color: var(--text-tertiary, #666); cursor: pointer; font-size: 1rem; }
  .btn-icon.delete:hover { color: var(--error, #f38ba8); }
  .form-panel { flex: 1; padding: 16px; overflow-y: auto; }
  .muted { color: var(--text-secondary, #aaa); font-size: 0.85rem; padding: 16px; }
  .btn-primary {
    background: var(--accent, #cba6f7); color: #1e1e2e; border: none;
    border-radius: 6px; padding: 6px 12px; cursor: pointer; font-size: 0.85rem; font-weight: 600;
  }
  .overlay {
    position: fixed; inset: 0; background: rgba(0,0,0,0.6);
    display: flex; align-items: center; justify-content: center; z-index: 100;
  }
  .confirm-box {
    background: var(--surface-1, #181825); border: 1px solid var(--border, #333);
    border-radius: 10px; padding: 20px; max-width: 360px; width: 90%;
  }
  .confirm-box p { margin: 0 0 16px; color: var(--text-primary, #fff); }
  .actions { display: flex; gap: 8px; }
  .btn-danger {
    background: var(--error, #f38ba8); color: #1e1e2e; border: none;
    border-radius: 6px; padding: 6px 14px; cursor: pointer; font-weight: 600;
  }
  .btn-ghost {
    background: transparent; color: var(--text-secondary, #aaa);
    border: 1px solid var(--border, #333); border-radius: 6px; padding: 6px 14px; cursor: pointer;
  }
  .toast {
    position: fixed; bottom: 20px; left: 50%; transform: translateX(-50%);
    background: var(--surface-2, #1e1e2e); color: var(--text-primary, #fff);
    border: 1px solid var(--border, #333); border-radius: 8px;
    padding: 10px 20px; z-index: 200; font-size: 0.9rem;
  }
</style>
