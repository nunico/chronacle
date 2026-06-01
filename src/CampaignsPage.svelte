<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getCollections,
    createCollection,
    updateCollection,
    deleteCollection,
    getCampaigns,
    createCampaign,
    deleteCampaign,
    getSources,
    deleteSource,
    getCampaignCollections,
    addCampaignCollection,
    removeCampaignCollection,
    type Collection,
    type Campaign,
    type Source,
  } from './lib/commands';

  // ── State ─────────────────────────────────────────────────────────
  let collections = $state<Collection[]>([]);
  let campaigns = $state<Campaign[]>([]);

  type Selection =
    | { kind: 'collection'; id: string }
    | { kind: 'campaign'; id: string }
    | null;

  let selected = $state<Selection>(null);
  let sources = $state<Source[]>([]);
  let campaignCollections = $state<Collection[]>([]);

  let loadingSources = $state(false);
  let loadingCampaignCols = $state(false);
  let error = $state('');

  // ── Create collection form ────────────────────────────────────────
  let showNewCollection = $state(false);
  let newCollectionName = $state('');

  // ── Create campaign form ──────────────────────────────────────────
  let showNewCampaign = $state(false);
  let newCampaignName = $state('');
  let newCampaignSystem = $state('');

  // ── Inline rename state ───────────────────────────────────────────
  let renamingId = $state<string | null>(null);
  let renameValue = $state('');

  // ── Deleting state ────────────────────────────────────────────────
  let deletingCollectionId = $state<string | null>(null);
  let deletingCampaignId = $state<string | null>(null);
  let deletingSourceId = $state<string | null>(null);

  // ── Subscribe/unsubscribe state ───────────────────────────────────
  let addingCollectionId = $state<string | null>(null);

  onMount(async () => {
    await loadAll();
  });

  async function loadAll() {
    try {
      [collections, campaigns] = await Promise.all([getCollections(), getCampaigns()]);
    } catch (e) {
      error = String(e);
    }
  }

  async function selectCollection(id: string) {
    selected = { kind: 'collection', id };
    loadingSources = true;
    error = '';
    try {
      sources = await getSources(id);
    } catch (e) {
      error = String(e);
    } finally {
      loadingSources = false;
    }
  }

  async function selectCampaign(id: string) {
    selected = { kind: 'campaign', id };
    loadingCampaignCols = true;
    error = '';
    try {
      campaignCollections = await getCampaignCollections(id);
    } catch (e) {
      error = String(e);
    } finally {
      loadingCampaignCols = false;
    }
  }

  async function handleCreateCollection() {
    if (!newCollectionName.trim()) return;
    error = '';
    try {
      await createCollection(newCollectionName.trim());
      newCollectionName = '';
      showNewCollection = false;
      await loadAll();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleCreateCampaign() {
    if (!newCampaignName.trim()) return;
    error = '';
    try {
      await createCampaign(newCampaignName.trim(), newCampaignSystem.trim() || '');
      newCampaignName = '';
      newCampaignSystem = '';
      showNewCampaign = false;
      await loadAll();
    } catch (e) {
      error = String(e);
    }
  }

  async function startRename(id: string, currentName: string) {
    renamingId = id;
    renameValue = currentName;
  }

  async function commitRename(col: Collection) {
    if (!renameValue.trim() || renameValue === col.name) {
      renamingId = null;
      return;
    }
    error = '';
    try {
      await updateCollection(col.id, renameValue.trim(), col.description ?? undefined);
      renamingId = null;
      await loadAll();
      if (selected?.kind === 'collection' && selected.id === col.id) {
        await selectCollection(col.id);
      }
    } catch (e) {
      error = String(e);
    }
  }

  async function handleDeleteCollection(id: string) {
    if (!confirm('Delete this collection? All sources must be removed first.')) return;
    deletingCollectionId = id;
    error = '';
    try {
      await deleteCollection(id);
      if (selected?.kind === 'collection' && selected.id === id) {
        selected = null;
        sources = [];
      }
      await loadAll();
    } catch (e) {
      error = String(e);
    } finally {
      deletingCollectionId = null;
    }
  }

  async function handleDeleteCampaign(id: string) {
    if (!confirm('Delete this campaign?')) return;
    deletingCampaignId = id;
    error = '';
    try {
      await deleteCampaign(id);
      if (selected?.kind === 'campaign' && selected.id === id) {
        selected = null;
        campaignCollections = [];
      }
      await loadAll();
    } catch (e) {
      error = String(e);
    } finally {
      deletingCampaignId = null;
    }
  }

  async function handleDeleteSource(id: string) {
    if (!confirm('Delete this source and all its indexed chunks?')) return;
    deletingSourceId = id;
    error = '';
    try {
      await deleteSource(id);
      if (selected?.kind === 'collection') {
        sources = await getSources(selected.id);
      }
    } catch (e) {
      error = String(e);
    } finally {
      deletingSourceId = null;
    }
  }

  async function handleAddCollection(campaignId: string, collectionId: string) {
    if (campaignCollections.some((c) => c.id === collectionId)) return;
    addingCollectionId = collectionId;
    error = '';
    try {
      await addCampaignCollection(campaignId, collectionId);
      campaignCollections = await getCampaignCollections(campaignId);
    } catch (e) {
      error = String(e);
    } finally {
      addingCollectionId = null;
    }
  }

  async function handleRemoveCollection(campaignId: string, collectionId: string) {
    error = '';
    try {
      await removeCampaignCollection(campaignId, collectionId);
      campaignCollections = campaignCollections.filter((c) => c.id !== collectionId);
    } catch (e) {
      error = String(e);
    }
  }

  // ── Derived helpers ───────────────────────────────────────────────
  function selectedCollection(): Collection | undefined {
    if (selected?.kind !== 'collection') return undefined;
    return collections.find((c) => c.id === selected!.id);
  }

  function selectedCampaign(): Campaign | undefined {
    if (selected?.kind !== 'campaign') return undefined;
    return campaigns.find((c) => c.id === selected!.id);
  }

  function availableToAdd(): Collection[] {
    const subscribed = new Set(campaignCollections.map((c) => c.id));
    return collections.filter((c) => !subscribed.has(c.id));
  }
</script>

<div class="page">
  <!-- ── Sidebar ──────────────────────────────────────────────────── -->
  <aside class="sidebar">
    {#if error}
      <div class="error">{error}</div>
    {/if}

    <!-- Collections section -->
    <div class="section-header">
      <span class="section-title">Collections</span>
      <button class="icon-btn" onclick={() => (showNewCollection = !showNewCollection)}>
        {showNewCollection ? '✕' : '+'}
      </button>
    </div>

    {#if showNewCollection}
      <div class="inline-form">
        <input
          bind:value={newCollectionName}
          placeholder="Collection name"
          onkeydown={(e) => e.key === 'Enter' && handleCreateCollection()}
        />
        <button class="primary-btn" onclick={handleCreateCollection}>Create</button>
      </div>
    {/if}

    <div class="item-list">
      {#each collections as col}
        <div class="item-row">
          {#if renamingId === col.id}
            <input
              class="rename-input"
              bind:value={renameValue}
              onblur={() => commitRename(col)}
              onkeydown={(e) => {
                if (e.key === 'Enter') commitRename(col);
                if (e.key === 'Escape') renamingId = null;
              }}
            />
          {:else}
            <button
              class="item-btn"
              class:active={selected?.kind === 'collection' && selected.id === col.id}
              onclick={() => selectCollection(col.id)}
            >
              <span class="item-icon">📚</span>
              <span class="item-name">{col.name}</span>
            </button>
          {/if}
          <div class="item-actions">
            <button class="icon-btn-sm" title="Rename" onclick={() => startRename(col.id, col.name)}>✏</button>
            <button
              class="icon-btn-sm danger"
              title="Delete"
              disabled={deletingCollectionId === col.id}
              onclick={() => handleDeleteCollection(col.id)}
            >✖</button>
          </div>
        </div>
      {/each}
    </div>

    <div class="divider"></div>

    <!-- Campaigns section -->
    <div class="section-header">
      <span class="section-title">Campaigns</span>
      <button class="icon-btn" onclick={() => (showNewCampaign = !showNewCampaign)}>
        {showNewCampaign ? '✕' : '+'}
      </button>
    </div>

    {#if showNewCampaign}
      <div class="inline-form">
        <input
          bind:value={newCampaignName}
          placeholder="Campaign name"
          onkeydown={(e) => e.key === 'Enter' && handleCreateCampaign()}
        />
        <input
          bind:value={newCampaignSystem}
          placeholder="Game system (e.g. D&D 5e)"
          onkeydown={(e) => e.key === 'Enter' && handleCreateCampaign()}
        />
        <button class="primary-btn" onclick={handleCreateCampaign}>Create</button>
      </div>
    {/if}

    <div class="item-list">
      {#each campaigns as campaign}
        <div class="item-row">
          <button
            class="item-btn"
            class:active={selected?.kind === 'campaign' && selected.id === campaign.id}
            onclick={() => selectCampaign(campaign.id)}
          >
            <span class="item-icon">📂</span>
            <span class="item-name">{campaign.name}</span>
            {#if campaign.system}
              <span class="item-tag">{campaign.system}</span>
            {/if}
          </button>
          <div class="item-actions">
            <button
              class="icon-btn-sm danger"
              title="Delete campaign"
              disabled={deletingCampaignId === campaign.id}
              onclick={() => handleDeleteCampaign(campaign.id)}
            >✖</button>
          </div>
        </div>
      {/each}
    </div>
  </aside>

  <!-- ── Main area ──────────────────────────────────────────────────── -->
  <main class="main">
    {#if selected === null}
      <div class="empty-state">
        <p>Select a collection to browse its sources, or a campaign to manage its subscriptions.</p>
      </div>

    {:else if selected.kind === 'collection'}
      {@const col = selectedCollection()}
      {#if col}
        <h2 class="main-title">{col.name}</h2>
        {#if loadingSources}
          <p class="muted">Loading sources…</p>
        {:else if sources.length === 0}
          <div class="empty-state">
            <p>No sources in this collection yet.</p>
            <p class="hint">Upload a PDF using the <strong>Upload PDF</strong> button above.</p>
          </div>
        {:else}
          <div class="source-list">
            {#each sources as source}
              <div class="source-card">
                <div class="source-info">
                  <span class="source-name">{source.display_name}</span>
                  <span class="source-meta">
                    <span
                      class="badge"
                      class:badge-ok={source.index_status === 'done'}
                      class:badge-pending={source.index_status === 'pending' || source.index_status === 'indexing'}
                      class:badge-error={source.index_status === 'error'}
                    >{source.index_status}</span>
                    <span>{source.page_count} pages</span>
                    <span class="source-type">{source.source_type}</span>
                  </span>
                </div>
                <button
                  class="delete-source-btn"
                  disabled={deletingSourceId === source.id}
                  onclick={() => handleDeleteSource(source.id)}
                >
                  {#if deletingSourceId === source.id}
                    <span class="spinner-small"></span>
                  {:else}
                    ✖
                  {/if}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      {/if}

    {:else if selected.kind === 'campaign'}
      {@const campaign = selectedCampaign()}
      {#if campaign}
        <h2 class="main-title">
          {campaign.name}
          {#if campaign.system}<span class="system-badge">{campaign.system}</span>{/if}
        </h2>
        <h3 class="sub-title">Source Collections</h3>

        {#if loadingCampaignCols}
          <p class="muted">Loading…</p>
        {:else}
          <div class="chips">
            {#each campaignCollections as col}
              <span class="chip">
                {col.name}
                <button
                  class="chip-remove"
                  title="Unsubscribe"
                  onclick={() => handleRemoveCollection(campaign.id, col.id)}
                >✕</button>
              </span>
            {/each}
          </div>

          {#if availableToAdd().length > 0}
            <div class="add-collection-row">
              <span class="muted">Add collection:</span>
              {#each availableToAdd() as col}
                <button
                  class="add-chip-btn"
                  disabled={addingCollectionId === col.id}
                  onclick={() => handleAddCollection(campaign.id, col.id)}
                >+ {col.name}</button>
              {/each}
            </div>
          {/if}

          {#if campaignCollections.length === 0 && availableToAdd().length === 0}
            <p class="hint">No collections exist yet. Create a collection first, then subscribe this campaign to it.</p>
          {/if}
        {/if}
      {/if}
    {/if}
  </main>
</div>

<style>
  .page {
    display: flex;
    gap: 0;
    height: 100%;
    overflow: hidden;
  }

  .sidebar {
    width: 260px;
    min-width: 260px;
    border-right: 1px solid var(--border);
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    overflow-y: auto;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.25rem 0;
  }

  .section-title {
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  .icon-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
    font-size: 0.85rem;
    cursor: pointer;
    color: var(--text);
    line-height: 1.4;
  }

  .icon-btn:hover { background: var(--bg-assistant); }

  .inline-form {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.4rem;
    background: var(--bg-assistant);
    border-radius: 6px;
  }

  .inline-form input {
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .primary-btn {
    padding: 0.3rem 0.6rem;
    border: none;
    border-radius: 4px;
    background: var(--accent);
    color: #fff;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .primary-btn:hover { filter: brightness(1.15); }

  .item-list {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .item-row {
    display: flex;
    align-items: center;
    gap: 0.15rem;
  }

  .item-row .item-actions {
    display: flex;
    gap: 0.1rem;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .item-row:hover .item-actions { opacity: 1; }

  .item-btn {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.5rem;
    border: none;
    border-radius: 5px;
    background: none;
    cursor: pointer;
    text-align: left;
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
    transition: background 0.15s;
    min-width: 0;
  }

  .item-btn:hover { background: var(--bg-assistant); }
  .item-btn.active { background: var(--accent); color: #fff; }

  .item-icon { font-size: 0.9rem; flex-shrink: 0; }

  .item-name {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-weight: 500;
  }

  .item-tag {
    font-size: 0.7rem;
    opacity: 0.7;
    flex-shrink: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 60px;
  }

  .icon-btn-sm {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.2rem;
    color: var(--text-muted);
    border-radius: 3px;
    line-height: 1;
  }

  .icon-btn-sm:hover { background: var(--bg-assistant); color: var(--text); }
  .icon-btn-sm.danger:hover { color: #e74c3c; }
  .icon-btn-sm:disabled { opacity: 0.4; cursor: not-allowed; }

  .rename-input {
    flex: 1;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--accent);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .divider {
    height: 1px;
    background: var(--border);
    margin: 0.5rem 0;
  }

  .error {
    color: #e74c3c;
    font-size: 0.8rem;
    padding: 0.3rem 0.5rem;
    background: rgba(231, 76, 60, 0.1);
    border-radius: 4px;
  }

  /* ── Main area ─────────────────────────────────────────────────── */
  .main {
    flex: 1;
    padding: 1rem 1.25rem;
    overflow-y: auto;
  }

  .main-title {
    margin: 0 0 1rem 0;
    font-size: 1.1rem;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .system-badge {
    font-size: 0.75rem;
    font-weight: 400;
    opacity: 0.6;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
  }

  .sub-title {
    font-size: 0.85rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin: 0 0 0.5rem 0;
  }

  .empty-state {
    text-align: center;
    padding: 2rem 1rem;
    color: var(--text-muted);
  }

  .hint { font-size: 0.85rem; margin-top: 0.5rem; }
  .muted { color: var(--text-muted); font-size: 0.9rem; }

  .source-list { display: flex; flex-direction: column; gap: 0.5rem; }

  .source-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.7rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    transition: background 0.15s, border-color 0.15s;
  }

  .source-card:hover { background: var(--bg-assistant); border-color: var(--accent); }

  .source-info { flex: 1; display: flex; flex-direction: column; gap: 0.25rem; }
  .source-name { font-weight: 600; font-size: 0.9rem; }

  .source-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .badge {
    display: inline-block;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    font-size: 0.75rem;
    font-weight: 500;
    text-transform: uppercase;
  }

  .badge-ok { background: rgba(39, 174, 96, 0.15); color: #27ae60; }
  .badge-pending { background: rgba(241, 196, 15, 0.15); color: #f1c40f; }
  .badge-error { background: rgba(231, 76, 60, 0.15); color: #e74c3c; }
  .source-type { text-transform: capitalize; }

  .delete-source-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: #e74c3c;
    font-size: 0.85rem;
    padding: 0.3rem;
    opacity: 0;
    transition: opacity 0.15s;
    line-height: 1;
  }

  .source-card:hover .delete-source-btn:not(:disabled) { opacity: 1; }
  .delete-source-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
    margin-bottom: 0.75rem;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0.5rem;
    background: var(--bg-assistant);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 0.8rem;
    font-weight: 500;
  }

  .chip-remove {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 0.7rem;
    color: var(--text-muted);
    padding: 0;
    line-height: 1;
  }

  .chip-remove:hover { color: #e74c3c; }

  .add-collection-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
    margin-top: 0.5rem;
  }

  .add-chip-btn {
    padding: 0.2rem 0.6rem;
    border: 1px dashed var(--border);
    border-radius: 999px;
    background: none;
    cursor: pointer;
    font-size: 0.8rem;
    color: var(--text-muted);
    transition: border-color 0.15s, color 0.15s;
  }

  .add-chip-btn:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
  .add-chip-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .spinner-small {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid var(--border);
    border-top-color: #e74c3c;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    vertical-align: middle;
  }

  @keyframes spin { to { transform: rotate(360deg); } }
</style>
