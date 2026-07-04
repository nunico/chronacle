<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Icon from '../components/Icon.svelte';
  import {
    getCollections,
    getCampaignCollections,
    addCampaignCollection,
    removeCampaignCollection,
    getSources,
    deleteSource,
    createCampaign,
    updateCampaign,
    deleteCampaign,
    getCodexStatus,
    compileCollection,
    cancelCompile,
    type Collection,
    type Campaign,
    type Source,
    type OnOwnedCollection,
    type CodexStatus,
    type CompileProgress,
  } from '../lib/commands';
  import { collectionIcon } from './collection-icons';
  import EntityManager from '../components/EntityManager.svelte';
  import { SvelteMap, SvelteSet } from 'svelte/reactivity';

  let {
    activeCampaignId,
    campaigns,
    setActiveCampaignId,
    onOpenUpload,
    refreshCampaigns,
  }: {
    activeCampaignId: string | null;
    campaigns: Campaign[];
    setActiveCampaignId: (id: string | null) => void;
    onOpenUpload: (collectionId: string) => void;
    refreshCampaigns: () => Promise<void>;
  } = $props();

  let collections = $state<Collection[]>([]);
  let subscribed = $state<Collection[]>([]);
  let sourcesByCol = new SvelteMap<string, Source[]>();
  let expanded = new SvelteSet<string>();
  let error = $state('');

  let codexStatusByCol = new SvelteMap<string, CodexStatus>();
  let compilingCol = $state<string | null>(null);
  let compileDetail = $state('');
  let unlistenCodex: UnlistenFn | null = null;
  let destroyed = false;

  let manageOpen = $state(false);
  let newName = $state('');
  let newSystem = $state('');
  let editingId = $state<string | null>(null);
  let editName = $state('');
  let editSystem = $state('');

  let active = $derived(campaigns.find((c) => c.id === activeCampaignId) ?? null);
  let activeTab = $state<'library' | 'entities'>('library');

  onMount(async () => {
    try {
      collections = await getCollections();
    } catch (e) {
      error = String(e);
    }
    await refreshSubscribed();
    const un = await listen<CompileProgress>('codex-progress', (event) => {
      if (!compilingCol) return;
      compileDetail = event.payload.detail;
    });
    if (destroyed) {
      un();
    } else {
      unlistenCodex = un;
    }
  });

  onDestroy(() => {
    destroyed = true;
    if (unlistenCodex) unlistenCodex();
  });

  $effect(() => {
    void activeCampaignId;
    refreshSubscribed();
  });

  async function refreshSubscribed() {
    if (!activeCampaignId) {
      subscribed = [];
      return;
    }
    try {
      subscribed = await getCampaignCollections(activeCampaignId);
      await refreshCodexStatuses();
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshCodexStatuses() {
    for (const c of subscribed) {
      try {
        codexStatusByCol.set(c.id, await getCodexStatus(c.id));
      } catch (e) {
        error = String(e);
      }
    }
  }

  async function toggleCompile(c: Collection) {
    if (compilingCol === c.id) {
      try {
        await cancelCompile();
      } catch (e) {
        error = String(e);
      }
      return;
    }
    error = '';
    compilingCol = c.id;
    compileDetail = '';
    try {
      await compileCollection(c.id);
      codexStatusByCol.set(c.id, await getCodexStatus(c.id));
    } catch (e) {
      error = String(e);
    } finally {
      compilingCol = null;
      compileDetail = '';
    }
  }

  function isSubscribed(id: string): boolean {
    return subscribed.some((c) => c.id === id);
  }

  async function toggleSubscribe(c: Collection) {
    if (!activeCampaignId) return;
    error = '';
    try {
      if (isSubscribed(c.id)) {
        await removeCampaignCollection(activeCampaignId, c.id);
      } else {
        await addCampaignCollection(activeCampaignId, c.id);
      }
      await refreshSubscribed();
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleExpand(c: Collection) {
    if (expanded.has(c.id)) {
      expanded.delete(c.id);
    } else {
      expanded.add(c.id);
      if (!sourcesByCol.has(c.id)) {
        try {
          const list = await getSources(c.id);
          sourcesByCol.set(c.id, list);
        } catch (e) {
          error = String(e);
        }
      }
    }
  }

  async function removeSource(s: Source, colId: string) {
    if (!confirm('Delete this source and all its indexed chunks?')) return;
    try {
      await deleteSource(s.id);
      const list = await getSources(colId);
      sourcesByCol.set(colId, list);
    } catch (e) {
      error = String(e);
    }
  }

  async function createNewCampaign() {
    if (!newName.trim()) return;
    try {
      const c = await createCampaign(newName.trim(), newSystem.trim());
      newName = '';
      newSystem = '';
      await refreshCampaigns();
      setActiveCampaignId(c.id);
    } catch (e) {
      error = String(e);
    }
  }

  function startEdit(c: Campaign) {
    editingId = c.id;
    editName = c.name;
    editSystem = c.system ?? '';
  }

  async function commitEdit() {
    if (!editingId || !editName.trim()) {
      editingId = null;
      return;
    }
    try {
      await updateCampaign(editingId, editName.trim(), editSystem.trim());
      editingId = null;
      await refreshCampaigns();
    } catch (e) {
      error = String(e);
      editingId = null;
    }
  }

  let deleteTarget = $state<Campaign | null>(null);

  function removeCampaign(c: Campaign) {
    deleteTarget = c;
  }

  async function confirmDelete(mode: OnOwnedCollection) {
    if (!deleteTarget) return;
    const target = deleteTarget;
    deleteTarget = null;
    try {
      await deleteCampaign(target.id, mode);
      if (activeCampaignId === target.id) setActiveCampaignId(null);
      await refreshCampaigns();
    } catch (e) {
      error = String(e);
    }
  }

  let subCount = $derived(subscribed.length);
  let bookCount = $derived(
    subscribed.reduce((n, c) => n + (sourcesByCol.get(c.id)?.length ?? 0), 0),
  );
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape' && deleteTarget) deleteTarget = null;
  }}
/>

<div class="scroll">
  <div class="cv">
    {#if error}
      <div class="error">{error}</div>
    {/if}

    <section class="hero">
      <div class="gem"></div>
      <div class="hero-text">
        <div class="eyebrow">Campaign</div>
        <h1>{active?.name ?? 'No campaign yet'}</h1>
        <p class="meta">
          {active?.system ?? '—'}
          {#if !active}<span class="hint"> · create one below to get started</span>{/if}
        </p>
      </div>
      {#if active}
        <button class="edit" onclick={() => startEdit(active)}>
          <Icon name="pencil" size={14} />
          Edit details
        </button>
      {/if}
    </section>

    <div class="view-tabs" role="tablist">
      <button
        role="tab"
        aria-selected={activeTab === 'library'}
        class="view-tab"
        class:active={activeTab === 'library'}
        onclick={() => { activeTab = 'library'; }}
      >
        Library
      </button>
      <button
        role="tab"
        aria-selected={activeTab === 'entities'}
        class="view-tab"
        class:active={activeTab === 'entities'}
        onclick={() => { activeTab = 'entities'; }}
      >
        Entities
      </button>
    </div>

    {#if activeTab === 'library'}
    <div class="stats">
      <div class="stat"><span class="n">{subCount}</span><span class="l">collections</span></div>
      <div class="stat"><span class="n">{bookCount}</span><span class="l">books loaded</span></div>
      <div class="stat"><span class="n">—</span><span class="l">notebook entries</span></div>
      <div class="stat"><span class="n">—</span><span class="l">sessions logged</span></div>
    </div>

    <section class="manage">
      <button class="manage-head" onclick={() => (manageOpen = !manageOpen)}>
        <Icon name={manageOpen ? 'chevron-down' : 'chevron-right'} size={16} />
        Manage campaigns
        <span class="ct">{campaigns.length}</span>
      </button>
      {#if manageOpen}
        <div class="manage-body">
          {#each campaigns as c (c.id)}
            <div class="manage-row" class:active={activeCampaignId === c.id}>
              {#if editingId === c.id}
                <input class="m-edit" bind:value={editName} placeholder="Name" />
                <input class="m-edit" bind:value={editSystem} placeholder="System (optional)" />
                <button class="m-btn primary" onclick={commitEdit}>Save</button>
                <button class="m-btn" onclick={() => (editingId = null)}>Cancel</button>
              {:else}
                <button class="m-pick" onclick={() => setActiveCampaignId(c.id)}>
                  <span class="m-nm">{c.name}</span>
                  {#if c.system}<span class="m-sys">{c.system}</span>{/if}
                </button>
                <button class="m-btn" onclick={() => startEdit(c)} title="Rename">
                  <Icon name="pencil" size={13} />
                </button>
                <button class="m-btn danger" onclick={() => removeCampaign(c)} title="Delete">
                  <Icon name="trash-2" size={13} />
                </button>
              {/if}
            </div>
          {/each}
          <div class="manage-new">
            <input bind:value={newName} placeholder="New campaign name" />
            <input bind:value={newSystem} placeholder="System (optional)" />
            <button class="m-btn primary" onclick={createNewCampaign}>+ Create</button>
          </div>
        </div>
      {/if}
    </section>

    <section class="collections">
      <div class="sec-head">
        <h2>Source collections</h2>
        <p>
          Subscribe this campaign to the rulebooks and lore it should draw from. Collections are
          shared across campaigns; subscribing is per-campaign.
        </p>
      </div>

      {#if collections.length === 0}
        <p class="muted">No collections yet. Upload a PDF to create one.</p>
      {/if}

      {#each collections as c (c.id)}
        {@const on = isSubscribed(c.id)}
        {@const isOpen = expanded.has(c.id)}
        {@const list = sourcesByCol.get(c.id) ?? []}
        {@const codexStatus = codexStatusByCol.get(c.id)}
        {@const compiling = compilingCol === c.id}
        <div class="coll" class:on>
          <div
            class="coll-head"
            role="button"
            tabindex="0"
            onclick={() => toggleExpand(c)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                toggleExpand(c);
              }
            }}
          >
            <span class="coll-ic"><Icon name={collectionIcon(c.name)} size={18} /></span>
            <span class="coll-text">
              <span class="nm">{c.name}</span>
              <span class="ct">
                {list.length} {list.length === 1 ? 'book' : 'books'} ·
                {#if !activeCampaignId}
                  shared
                {:else if on}
                  subscribed
                {:else}
                  not subscribed
                {/if}
              </span>
            </span>
            {#if on && codexStatus && codexStatus.stale_entities > 0}
              <span class="codex-badge">{codexStatus.stale_entities} stale</span>
            {/if}
            {#if on}
              <button
                class="m-btn compile-btn"
                aria-label="{compiling ? 'Cancel' : 'Compile'} {c.name}"
                onclick={(e) => {
                  e.stopPropagation();
                  toggleCompile(c);
                }}
                onkeydown={(e) => {
                  e.stopPropagation();
                }}
              >
                {compiling ? 'Cancel' : 'Compile'}
              </button>
            {/if}
            <span
              class="sub-toggle"
              class:on
              role="switch"
              aria-checked={on}
              tabindex="0"
              aria-label="Subscribe to {c.name}"
              onclick={(e) => {
                e.stopPropagation();
                toggleSubscribe(c);
              }}
              onkeydown={(e) => {
                e.stopPropagation();
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  toggleSubscribe(c);
                }
              }}
            >
              <span class="knob"></span>
            </span>
            <Icon name={isOpen ? 'chevron-up' : 'chevron-down'} size={16} />
          </div>
          {#if compiling}
            <div class="codex-progress">{compileDetail || 'Compiling…'}</div>
          {/if}
          {#if isOpen}
            <div class="books">
              {#each list as s (s.id)}
                <div class="book">
                  <Icon name="file-text" size={14} />
                  <span class="bnm">{s.display_name}</span>
                  <span
                    class="book-status"
                    class:ok={s.index_status === 'done'}
                    class:idx={s.index_status === 'pending' || s.index_status === 'indexing'}
                    class:err={s.index_status === 'error'}
                  >
                    {s.index_status === 'done'
                      ? 'Indexed'
                      : s.index_status === 'error'
                        ? 'Error'
                        : 'Indexing…'}
                  </span>
                  <button class="m-btn danger" onclick={() => removeSource(s, c.id)} title="Delete">
                    <Icon name="trash-2" size={13} />
                  </button>
                </div>
              {/each}
              <button class="add-book" onclick={() => onOpenUpload(c.id)}>
                <Icon name="plus" size={14} />
                Add book
              </button>
            </div>
          {/if}
        </div>
      {/each}
    </section>
    {:else if activeTab === 'entities' && active}
      <EntityManager campaignId={active.id} />
    {:else if activeTab === 'entities'}
      <p class="muted">Select a campaign to manage entities.</p>
    {/if}
  </div>

  {#if deleteTarget}
    <div class="modal-overlay" role="presentation" onclick={() => (deleteTarget = null)}>
      <div
        class="modal"
        role="dialog"
        aria-label="Delete campaign"
        onclick={(e) => e.stopPropagation()}
      >
        <h3>Delete "{deleteTarget.name}"?</h3>
        <p>
          If this campaign has its own collection of notes and entities, choose what happens to
          it.
        </p>
        <div class="modal-actions">
          <button class="m-btn danger" onclick={() => confirmDelete('delete')}>
            Delete campaign and its notes
          </button>
          <button class="m-btn" onclick={() => confirmDelete('convert_to_regular')}>
            Keep notes as a regular collection
          </button>
          <button class="m-btn" onclick={() => (deleteTarget = null)}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .cv {
    max-width: 820px;
    margin: 0 auto;
    padding: 30px 26px 40px;
    font-family: var(--font-sans);
  }
  .error {
    padding: 8px 12px;
    background: var(--danger-bg);
    color: var(--danger);
    border: 1px solid rgba(242, 103, 75, 0.4);
    border-radius: var(--r-md);
    margin-bottom: 14px;
    font-size: 13px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-bottom: 22px;
  }
  .hero .gem {
    width: 56px;
    height: 56px;
    border-radius: var(--r-lg);
    background: var(--grad-gem);
    box-shadow: var(--glow-violet);
    flex: none;
  }
  .eyebrow {
    font-family: var(--font-sans);
    font-weight: 700;
    font-size: 11px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--arcane-300);
    margin-bottom: 4px;
  }
  .hero h1 {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 26px;
    margin: 0;
    color: var(--fg-1);
  }
  .meta {
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--fg-3);
    margin: 4px 0 0;
  }
  .meta .hint {
    color: var(--arcane-300);
  }
  .edit {
    margin-left: auto;
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    color: var(--fg-2);
    font-weight: 600;
    font-size: 13px;
    background: none;
  }
  .edit:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    margin-bottom: 24px;
  }
  .stat {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 12px 14px;
    box-shadow: var(--shadow-card);
  }
  .stat .n {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 700;
    color: var(--fg-1);
    display: block;
  }
  .stat .l {
    font-size: 11.5px;
    color: var(--fg-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .manage {
    margin-bottom: 22px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .manage-head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    background: none;
    border: 0;
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 13.5px;
    text-align: left;
  }
  .manage-head .ct {
    margin-left: auto;
    color: var(--fg-3);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .manage-body {
    border-top: 1px solid var(--line);
    padding: 8px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .manage-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: var(--r-sm);
  }
  .manage-row.active {
    background: rgba(91, 120, 255, 0.08);
  }
  .m-pick {
    flex: 1;
    text-align: left;
    background: none;
    border: 0;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
    padding: 6px 8px;
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .m-sys {
    color: var(--fg-3);
    font-size: 12px;
  }
  .m-edit {
    flex: 1;
    padding: 5px 8px;
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .manage-new {
    display: flex;
    gap: 6px;
    padding: 4px 6px;
  }
  .manage-new input {
    flex: 1;
    padding: 5px 8px;
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .m-btn {
    padding: 5px 10px;
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 12.5px;
  }
  .m-btn:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .m-btn.primary {
    background: var(--grad-arcane);
    border-color: transparent;
    color: var(--fg-on-accent);
  }
  .m-btn.danger {
    color: var(--danger);
    border-color: rgba(242, 103, 75, 0.4);
  }
  .m-btn.danger:hover {
    background: var(--danger-bg);
  }
  .collections .sec-head {
    margin-bottom: 12px;
  }
  .collections h2 {
    font-family: var(--font-display);
    font-size: 18px;
    margin: 0 0 4px;
    color: var(--fg-1);
  }
  .collections .sec-head p {
    color: var(--fg-3);
    font-size: 13px;
    margin: 0;
  }
  .coll {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    margin-bottom: 10px;
  }
  .coll.on {
    border-color: var(--line-strong);
  }
  .coll-head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: none;
    border: 0;
    text-align: left;
    cursor: pointer;
  }
  .coll-ic {
    color: var(--violet-300);
  }
  .coll-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .coll-text .nm {
    color: var(--fg-1);
    font-weight: 600;
    font-size: 14px;
  }
  .coll-text .ct {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--fg-3);
  }
  .codex-badge {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 2px 7px;
    border-radius: var(--r-full);
    background: var(--warning-bg);
    color: var(--warning);
    flex: none;
  }
  .compile-btn {
    flex: none;
    padding: 4px 10px;
    font-size: 12px;
  }
  .codex-progress {
    padding: 0 12px 8px;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--fg-3);
  }
  .sub-toggle {
    width: 32px;
    height: 18px;
    border-radius: var(--r-full);
    background: var(--bg-inset);
    border: 1px solid var(--line);
    flex: none;
    position: relative;
    cursor: pointer;
  }
  .sub-toggle .knob {
    position: absolute;
    top: 1.5px;
    left: 1.5px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--fg-3);
    transition: transform var(--dur) var(--ease-arcane), background var(--dur);
  }
  .sub-toggle.on {
    background: rgba(91, 120, 255, 0.3);
    border-color: var(--line-glow);
    box-shadow: var(--glow-arcane);
  }
  .sub-toggle.on .knob {
    transform: translateX(13px);
    background: var(--gem);
  }
  .books {
    border-top: 1px solid var(--line-faint);
    padding: 8px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .book {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px;
    font-size: 13px;
    color: var(--fg-2);
  }
  .bnm {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .book-status {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 2px 7px;
    border-radius: var(--r-full);
    background: var(--bg-inset);
  }
  .book-status.ok {
    color: var(--success);
    background: var(--success-bg);
  }
  .book-status.idx {
    color: var(--warning);
    background: var(--warning-bg);
  }
  .book-status.err {
    color: var(--danger);
    background: var(--danger-bg);
  }
  .add-book {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    align-self: flex-start;
    border-radius: var(--r-full);
    border: 1px dashed var(--line);
    background: none;
    color: var(--fg-3);
    font-family: var(--font-sans);
    font-size: 12.5px;
    margin-top: 4px;
  }
  .add-book:hover {
    border-color: var(--line-glow);
    color: var(--arcane-300);
  }
  .muted {
    color: var(--fg-3);
    font-size: 13px;
  }
  .view-tabs {
    display: flex;
    gap: 4px;
    padding: 0 0 0 2px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 20px;
  }
  .view-tab {
    background: none;
    border: none;
    color: var(--fg-3);
    padding: 10px 16px;
    cursor: pointer;
    font-family: var(--font-sans);
    font-size: 13.5px;
    font-weight: 500;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
  }
  .view-tab.active {
    color: var(--fg-1);
    border-bottom-color: var(--violet-300);
  }
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: grid;
    place-items: center;
    z-index: 50;
  }
  .modal {
    background: var(--bg-raised, #1d1a17);
    border: 1px solid var(--border, #3a352f);
    border-radius: 8px;
    padding: 1.25rem;
    max-width: 26rem;
  }
  .modal-actions {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-top: 1rem;
  }
</style>
