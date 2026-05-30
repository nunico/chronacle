<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getCampaigns,
    createCampaign,
    deleteCampaign,
    getSources,
    deleteSource,
    type Campaign,
    type Source,
  } from './lib/commands';

  let campaigns = $state<Campaign[]>([]);
  let sources = $state<Source[]>([]);
  let selectedCampaignId = $state<string | null>(null);
  let showCreateForm = $state(false);
  let newName = $state('');
  let newSystem = $state('');
  let error = $state('');
  let loading = $state(true);
  let sourceLoading = $state(false);

  onMount(() => {
    loadCampaigns();
    loadSources(null);
  });

  async function loadCampaigns() {
    loading = true;
    try {
      campaigns = await getCampaigns();
    } catch (e) {
      console.error('Failed to load campaigns:', e);
    } finally {
      loading = false;
    }
  }

  async function loadSources(campaignId: string | null) {
    sourceLoading = true;
    try {
      sources = await getSources(campaignId);
    } catch (e) {
      console.error('Failed to load sources:', e);
    } finally {
      sourceLoading = false;
    }
  }

  function selectCampaign(id: string | null) {
    selectedCampaignId = id;
    loadSources(id);
  }

  async function handleCreate() {
    if (!newName.trim()) return;
    error = '';
    try {
      await createCampaign(newName.trim(), newSystem.trim() || 'Generic');
      newName = '';
      newSystem = '';
      showCreateForm = false;
      await loadCampaigns();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleDelete(id: string) {
    if (!confirm('Delete this campaign and all its sources?')) return;
    try {
      await deleteCampaign(id);
      if (selectedCampaignId === id) {
        selectedCampaignId = null;
        await loadSources(null);
      }
      await loadCampaigns();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleDeleteSource(id: string) {
    if (!confirm('Delete this source and all its indexed chunks?')) return;
    try {
      await deleteSource(id);
      await loadSources(selectedCampaignId);
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="campaigns-page">
  <div class="sidebar">
    <div class="sidebar-header">
      <h2>Campaigns</h2>
      <button class="add-btn" onclick={() => (showCreateForm = !showCreateForm)}>
        {showCreateForm ? 'Cancel' : '+ New'}
      </button>
    </div>

    {#if showCreateForm}
      <div class="create-form">
        <input
          bind:value={newName}
          placeholder="Campaign name"
          onkeydown={(e) => e.key === 'Enter' && handleCreate()}
        />
        <input
          bind:value={newSystem}
          placeholder="Game system (e.g. D&D 5e)"
          onkeydown={(e) => e.key === 'Enter' && handleCreate()}
        />
        <button class="primary-btn" onclick={handleCreate}>Create</button>
      </div>
    {/if}

    {#if error}
      <div class="error">{error}</div>
    {/if}

    <div class="campaign-list">
      <button
        class="campaign-item global"
        class:active={selectedCampaignId === null}
        onclick={() => selectCampaign(null)}
      >
        <span class="campaign-icon">🌍</span>
        <span class="campaign-name">Global Sources</span>
      </button>

      {#each campaigns as campaign}
        <div class="campaign-item-row">
          <button
            class="campaign-item"
            class:active={selectedCampaignId === campaign.id}
            onclick={() => selectCampaign(campaign.id)}
          >
            <span class="campaign-icon">📂</span>
            <span class="campaign-name">{campaign.name}</span>
            <span class="campaign-system">{campaign.system}</span>
          </button>
          <button
            class="delete-btn"
            title="Delete campaign"
            onclick={() => handleDelete(campaign.id)}
          >
            ✖
          </button>
        </div>
      {/each}
    </div>
  </div>

  <div class="main-area">
    <h2>
      {selectedCampaignId === null
        ? 'Global Sources'
        : campaigns.find((c) => c.id === selectedCampaignId)?.name ?? 'Sources'}
    </h2>

    {#if sourceLoading}
      <p class="loading-text">Loading sources…</p>
    {:else if sources.length === 0}
      <div class="empty-state">
        <p>No sources in this campaign yet.</p>
        <p class="hint">Upload a PDF using the <strong>Upload PDF</strong> button above.</p>
      </div>
    {:else}
      <div class="source-list">
        {#each sources as source}
          <div class="source-card">
            <div class="source-info">
              <span class="source-name">{source.display_name}</span>
              <span class="source-meta">
                <span class="badge" class:badge-ok={source.index_status === 'done'} class:badge-pending={source.index_status === 'pending' || source.index_status === 'indexing'} class:badge-error={source.index_status === 'error'}>
                  {source.index_status}
                </span>
                <span>{source.page_count} pages</span>
                <span class="source-type">{source.source_type}</span>
              </span>
            </div>
            <button
              class="delete-source-btn"
              title="Delete source"
              onclick={() => handleDeleteSource(source.id)}
            >
              ✖
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .campaigns-page {
    display: flex;
    gap: 1rem;
    height: 100%;
    overflow: hidden;
  }

  .sidebar {
    width: 260px;
    min-width: 260px;
    border-right: 1px solid var(--border);
    padding-right: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    overflow-y: auto;
  }

  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .sidebar-header h2 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
  }

  .add-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.25rem 0.5rem;
    font-size: 0.8rem;
    cursor: pointer;
    color: var(--text);
  }

  .add-btn:hover {
    background: var(--bg-assistant);
  }

  .create-form {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.5rem;
    background: var(--bg-assistant);
    border-radius: 6px;
  }

  .create-form input {
    padding: 0.4rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .primary-btn {
    padding: 0.35rem 0.75rem;
    border: none;
    border-radius: 4px;
    background: var(--accent);
    color: #fff;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .primary-btn:hover {
    filter: brightness(1.15);
  }

  .error {
    color: #e74c3c;
    font-size: 0.8rem;
    padding: 0.3rem 0.5rem;
    background: rgba(231, 76, 60, 0.1);
    border-radius: 4px;
  }

  .campaign-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .campaign-item-row {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .campaign-item-row .delete-btn {
    opacity: 0;
    transition: opacity 0.15s;
  }

  .campaign-item-row:hover .delete-btn {
    opacity: 1;
  }

  .campaign-item {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.6rem;
    border: none;
    border-radius: 6px;
    background: none;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s;
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .campaign-item:hover {
    background: var(--bg-assistant);
  }

  .campaign-item.active {
    background: var(--accent);
    color: #fff;
  }

  .campaign-icon {
    font-size: 1rem;
    flex-shrink: 0;
  }

  .campaign-name {
    font-weight: 600;
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .campaign-system {
    font-size: 0.75rem;
    opacity: 0.7;
    flex-shrink: 0;
  }

  .campaign-item.active .campaign-system {
    opacity: 0.9;
  }

  .campaign-item.global {
    margin-bottom: 0.25rem;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    padding-bottom: 0.6rem;
  }

  .delete-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: #e74c3c;
    font-size: 0.8rem;
    padding: 0.25rem;
    line-height: 1;
  }

  .main-area {
    flex: 1;
    overflow-y: auto;
  }

  .main-area h2 {
    margin: 0 0 1rem 0;
    font-size: 1.1rem;
    font-weight: 600;
  }

  .loading-text {
    color: var(--text-muted);
    font-size: 0.9rem;
  }

  .empty-state {
    text-align: center;
    padding: 2rem 1rem;
    color: var(--text-muted);
  }

  .empty-state .hint {
    font-size: 0.85rem;
    margin-top: 0.5rem;
  }

  .source-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .source-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.7rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    transition: background 0.15s, border-color 0.15s;
  }

  .source-card:hover {
    background: var(--bg-assistant);
    border-color: var(--accent);
  }

  .source-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .source-name {
    font-weight: 600;
    font-size: 0.9rem;
  }

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

  .badge-ok {
    background: rgba(39, 174, 96, 0.15);
    color: #27ae60;
  }

  .badge-pending {
    background: rgba(241, 196, 15, 0.15);
    color: #f1c40f;
  }

  .badge-error {
    background: rgba(231, 76, 60, 0.15);
    color: #e74c3c;
  }

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

  .source-card:hover .delete-source-btn {
    opacity: 1;
  }

  .source-type {
    text-transform: capitalize;
  }
</style>