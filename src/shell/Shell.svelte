<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    getCampaigns,
    getCollections,
    createCollection,
    uploadSource,
    getMruCollectionId,
    setMruCollectionId,
    type Campaign,
    type Collection,
  } from '../lib/commands';
  import CampaignRail, { type View } from './CampaignRail.svelte';
  import CampaignSwitcher from './CampaignSwitcher.svelte';
  import Topbar from './Topbar.svelte';
  import OracleView from '../views/OracleView.svelte';
  import CampaignView from '../views/CampaignView.svelte';
  import NotesView from '../views/NotesView.svelte';
  import SettingsView from '../views/SettingsView.svelte';
  import UploadProgress from '../UploadProgress.svelte';
  import { findCategory } from './note-categories';

  const ACTIVE_KEY = 'chronacle_active_campaign_id';

  let view = $state<View>('oracle');
  let campaigns = $state<Campaign[]>([]);
  let activeCampaignId = $state<string | null>(null);
  let switcherOpen = $state(false);

  // Upload dialog state (lifted from old App.svelte)
  let isUploading = $state(false);
  let uploadProgress = $state(0);
  let uploadStatus = $state('');
  let uploadedSourceName = $state('');
  let collections = $state<Collection[]>([]);
  let pendingPath = $state<string | null>(null);
  let pendingName = $state<string | null>(null);
  let showPicker = $state(false);
  let pickerCollectionId = $state('');
  let pickerNewName = $state('');
  let showNewCollectionInput = $state(false);
  let pickerError = $state('');

  onMount(async () => {
    try {
      campaigns = await getCampaigns();
    } catch (e) {
      console.error('Failed to load campaigns:', e);
    }
    const stored = localStorage.getItem(ACTIVE_KEY);
    if (stored && campaigns.some((c) => c.id === stored)) {
      activeCampaignId = stored;
    } else if (campaigns.length > 0) {
      setActiveCampaignId(campaigns[0].id);
    } else {
      activeCampaignId = null;
      view = 'campaign';
    }
  });

  function setActiveCampaignId(id: string | null) {
    activeCampaignId = id;
    if (id) localStorage.setItem(ACTIVE_KEY, id);
    else localStorage.removeItem(ACTIVE_KEY);
  }

  async function refreshCampaigns() {
    campaigns = await getCampaigns();
    if (activeCampaignId && !campaigns.some((c) => c.id === activeCampaignId)) {
      // Active campaign was deleted — fall back to the first remaining one
      // (or null if there are none, which forces the user to the campaign view).
      if (campaigns.length > 0) {
        setActiveCampaignId(campaigns[0].id);
      } else {
        setActiveCampaignId(null);
        view = 'campaign';
      }
    } else if (!activeCampaignId && campaigns.length > 0) {
      setActiveCampaignId(campaigns[0].id);
    }
  }

  let activeCampaign = $derived(campaigns.find((c) => c.id === activeCampaignId) ?? null);

  // Topbar copy
  let head = $derived.by(() => {
    if (view === 'oracle')
      return { title: 'Oracle', sub: 'Ask in plain language — answers come cited' };
    if (view === 'campaign')
      return { title: 'Campaign', sub: 'Manage details & subscribed source collections' };
    if (view === 'settings')
      return { title: 'Settings', sub: 'Provider, models, and re-indexing' };
    const cat = findCategory(view.category);
    return { title: cat.label, sub: cat.sub };
  });

  async function openFilePicker(initialCollectionId?: string) {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;
    const path = typeof selected === 'string' ? selected : selected[0];
    const name = path.split('/').pop()?.split('\\').pop() ?? 'document.pdf';
    pendingPath = path;
    pendingName = name;

    if (initialCollectionId) {
      // Skip the picker dialog: upload straight into the given collection.
      await startUpload(path, name, initialCollectionId);
      return;
    }

    try {
      collections = await getCollections();
    } catch (e) {
      console.error('Failed to load collections:', e);
      collections = [];
    }
    const mru = getMruCollectionId();
    pickerCollectionId =
      mru && collections.some((c) => c.id === mru) ? mru : (collections[0]?.id ?? '');
    showPicker = true;
    pickerError = '';
    showNewCollectionInput = false;
    pickerNewName = '';
  }

  async function handlePickerCreateNew() {
    if (!pickerNewName.trim()) return;
    pickerError = '';
    try {
      const newCol = await createCollection(pickerNewName.trim());
      collections = [...collections, newCol];
      pickerCollectionId = newCol.id;
      pickerNewName = '';
      showNewCollectionInput = false;
    } catch (e) {
      pickerError = String(e);
    }
  }

  async function confirmUpload() {
    if (!pickerCollectionId || !pendingPath || !pendingName) return;
    pickerError = '';
    const path = pendingPath;
    const name = pendingName;
    const colId = pickerCollectionId;
    showPicker = false;
    pendingPath = null;
    pendingName = null;
    setMruCollectionId(colId);
    await startUpload(path, name, colId);
  }

  async function startUpload(path: string, name: string, collectionId: string) {
    isUploading = true;
    uploadProgress = 0;
    uploadStatus = 'Uploading…';
    uploadedSourceName = name;
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    try {
      unlistenProgress = await listen<{
        source_id: string;
        status: string;
        progress: number;
        step?: string;
      }>('ingestion-progress', (event) => {
        uploadProgress = Math.round(event.payload.progress * 100);
        if (event.payload.status === 'done') {
          uploadStatus = 'Ready!';
          uploadProgress = 100;
        } else if (event.payload.step) {
          uploadStatus = event.payload.step;
        } else {
          uploadStatus = 'Indexing PDF…';
        }
      });
      unlistenError = await listen<{ source_id: string; error: string }>(
        'ingestion-error',
        (event) => {
          uploadStatus = `Error: ${event.payload.error}`;
          console.error('Ingestion error:', event.payload.error);
          isUploading = false;
        },
      );
      await uploadSource(path, name, 'rules', collectionId);
    } catch (e) {
      uploadStatus = `Upload failed: ${String(e)}`;
      isUploading = false;
    } finally {
      if (unlistenProgress) unlistenProgress();
      if (unlistenError) unlistenError();
      isUploading = false;
    }
  }
</script>

<div class="app">
  <CampaignRail
    {view}
    {activeCampaign}
    setView={(v) => (view = v)}
    onOpenSwitcher={() => (switcherOpen = true)}
    onOpenUpload={() => openFilePicker()}
  />

  {#if switcherOpen}
    <CampaignSwitcher
      {campaigns}
      {activeCampaignId}
      onSelect={setActiveCampaignId}
      onManage={() => (view = 'campaign')}
      onClose={() => (switcherOpen = false)}
    />
  {/if}

  <main class="main">
    <Topbar title={head.title} sub={head.sub} />
    {#if view === 'oracle'}
      <OracleView {activeCampaignId} onOpenUpload={() => openFilePicker()} />
    {:else if view === 'campaign'}
      <CampaignView
        {activeCampaignId}
        {campaigns}
        {setActiveCampaignId}
        onOpenUpload={(colId) => openFilePicker(colId)}
        {refreshCampaigns}
      />
    {:else if view === 'settings'}
      <SettingsView />
    {:else}
      <NotesView category={view.category} />
    {/if}

    <UploadProgress
      filename={uploadedSourceName}
      status={uploadStatus}
      progress={uploadProgress}
      isActive={isUploading}
    />
  </main>

  {#if showPicker}
    <div class="picker-overlay">
      <div class="picker-dialog" role="dialog" aria-modal="true" aria-labelledby="picker-title">
        <h3 id="picker-title">Add "{pendingName}" to collection</h3>
        {#if pickerError}
          <div class="picker-error">{pickerError}</div>
        {/if}
        {#if collections.length > 0}
          <select bind:value={pickerCollectionId} class="picker-select">
            {#each collections as col (col.id)}
              <option value={col.id}>{col.name}</option>
            {/each}
          </select>
        {:else}
          <p class="picker-hint">No collections yet.</p>
        {/if}
        {#if showNewCollectionInput}
          <div class="picker-new">
            <input
              bind:value={pickerNewName}
              placeholder="New collection name"
              onkeydown={(e) => e.key === 'Enter' && handlePickerCreateNew()}
            />
            <button class="picker-create-btn" onclick={handlePickerCreateNew}>Create</button>
            <button class="picker-cancel-btn" onclick={() => (showNewCollectionInput = false)}
              >Cancel</button>
          </div>
        {:else}
          <button class="picker-new-btn" onclick={() => (showNewCollectionInput = true)}
            >+ Create new collection</button>
        {/if}
        <div class="picker-actions">
          <button
            class="picker-cancel-btn"
            data-testid="picker-cancel"
            onclick={() => {
              showPicker = false;
              pendingPath = null;
              pendingName = null;
            }}>Cancel</button>
          <button class="picker-confirm-btn" disabled={!pickerCollectionId} onclick={confirmUpload}
            >Upload</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 264px 1fr;
    height: 100%;
    background:
      radial-gradient(70% 80% at 100% 0%, rgba(123, 92, 255, 0.1), transparent 55%),
      var(--bg-void) var(--tex-starfield);
    background-size: auto, 900px;
    color: var(--fg-1);
    font-family: var(--font-sans);
    position: relative;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .picker-overlay {
    position: fixed;
    inset: 0;
    background: var(--bg-scrim);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .picker-dialog {
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-3);
    padding: 18px;
    width: 340px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-family: var(--font-sans);
  }
  .picker-dialog h3 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 16px;
    color: var(--fg-1);
  }
  .picker-error {
    color: var(--danger);
    background: var(--danger-bg);
    border-radius: var(--r-sm);
    padding: 6px 10px;
    font-size: 12.5px;
  }
  .picker-select,
  .picker-new input {
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg-inset);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
  }
  .picker-hint {
    font-size: 13px;
    color: var(--fg-3);
    margin: 0;
  }
  .picker-new {
    display: flex;
    gap: 6px;
  }
  .picker-new input {
    flex: 1;
  }
  .picker-new-btn {
    background: none;
    border: 1px dashed var(--line);
    border-radius: var(--r-md);
    padding: 6px 12px;
    font-size: 12.5px;
    color: var(--fg-3);
    font-family: var(--font-sans);
  }
  .picker-new-btn:hover {
    border-color: var(--line-glow);
    color: var(--arcane-300);
  }
  .picker-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .picker-cancel-btn {
    background: none;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 7px 12px;
    font-size: 13px;
    color: var(--fg-2);
    font-family: var(--font-sans);
  }
  .picker-confirm-btn,
  .picker-create-btn {
    border: 0;
    border-radius: var(--r-md);
    padding: 7px 14px;
    font-size: 13px;
    font-weight: 600;
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
    font-family: var(--font-sans);
  }
  .picker-confirm-btn:disabled {
    opacity: 0.5;
    box-shadow: none;
  }
</style>
