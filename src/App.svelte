<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    getCampaigns,
    uploadSource,
    getCollections,
    createCollection,
    getMruCollectionId,
    setMruCollectionId,
    type Campaign,
    type Collection,
  } from './lib/commands';
  import SettingsPage from './SettingsPage.svelte';
  import CampaignsPage from './CampaignsPage.svelte';
  import ChatPage from './ChatPage.svelte';
  import ModelDownload from './ModelDownload.svelte';
  import UploadProgress from './UploadProgress.svelte';

  type Page = 'chat' | 'campaigns' | 'settings';

  let modelReady = $state(false);
  let currentPage = $state<Page>('chat');
  let campaigns = $state<Campaign[]>([]);
  let activeCampaignId = $state<string | null>(null);

  // Upload state
  let isUploading = $state(false);
  let uploadProgress = $state(0);
  let uploadStatus = $state('');
  let uploadedSourceName = $state('');

  // Collection picker state
  let collections = $state<Collection[]>([]);
  let pendingUploadPath = $state<string | null>(null);
  let pendingUploadName = $state<string | null>(null);
  let showCollectionPicker = $state(false);
  let pickerCollectionId = $state('');
  let pickerNewName = $state('');
  let showNewCollectionInput = $state(false);
  let pickerError = $state('');

  onMount(async () => {
    try {
      const campaignList = await getCampaigns();
      campaigns = campaignList;
    } catch (e) {
      console.error('Failed to load campaigns:', e);
    }
  });

  function onModelReady() {
    modelReady = true;
  }

  async function openFilePicker() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;

    const path = typeof selected === 'string' ? selected : selected[0];
    const name = path.split('/').pop()?.split('\\').pop() ?? 'document.pdf';

    try {
      collections = await getCollections();
    } catch (e) {
      console.error('Failed to load collections:', e);
      collections = [];
    }

    const mru = getMruCollectionId();
    pickerCollectionId =
      mru && collections.some((c) => c.id === mru) ? mru : (collections[0]?.id ?? '');
    pendingUploadPath = path;
    pendingUploadName = name;
    showCollectionPicker = true;
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
    if (!pickerCollectionId || !pendingUploadPath || !pendingUploadName) return;
    pickerError = '';

    const path = pendingUploadPath;
    const name = pendingUploadName;
    const collectionId = pickerCollectionId;

    showCollectionPicker = false;
    pendingUploadPath = null;
    pendingUploadName = null;

    setMruCollectionId(collectionId);

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
      }>(
        'ingestion-progress',
        (event) => {
          uploadProgress = Math.round(event.payload.progress * 100);
          if (event.payload.status === 'done') {
            uploadStatus = 'Ready!';
            uploadProgress = 100;
          } else if (event.payload.step) {
            uploadStatus = event.payload.step;
          } else {
            uploadStatus = 'Indexing PDF…';
          }
        },
      );

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

{#if !modelReady}
  <ModelDownload {onModelReady} />
{:else}
  <header>
    <h1>Chronacle</h1>
    <span class="tagline">TTRPG GM Assistant</span>
    <nav>
      <button class="nav-btn" class:active={currentPage === 'chat'} onclick={() => (currentPage = 'chat')}>
        Chat
      </button>
      <button class="nav-btn" class:active={currentPage === 'campaigns'} onclick={() => (currentPage = 'campaigns')}>
        Campaigns
      </button>
      <button class="nav-btn" class:active={currentPage === 'settings'} onclick={() => (currentPage = 'settings')}>
        Settings
      </button>
      <button class="upload-btn" onclick={openFilePicker} disabled={isUploading}>
        {isUploading ? 'Uploading…' : 'Upload PDF'}
      </button>
    </nav>
    <UploadProgress
      filename={uploadedSourceName}
      status={uploadStatus}
      progress={uploadProgress}
      isActive={isUploading}
    />
  </header>

  <main>
    {#if currentPage === 'chat'}
      <ChatPage {campaigns} bind:activeCampaignId />
    {:else if currentPage === 'campaigns'}
      <CampaignsPage />
    {:else}
      <SettingsPage />
    {/if}

    {#if showCollectionPicker}
      <div class="picker-overlay">
        <div class="picker-dialog" role="dialog" aria-modal="true" aria-labelledby="picker-title">
          <h3 id="picker-title">Add "{pendingUploadName}" to collection</h3>
          {#if pickerError}
            <div class="picker-error">{pickerError}</div>
          {/if}

          {#if collections.length > 0}
            <select bind:value={pickerCollectionId} class="picker-select">
              {#each collections as col}
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
              <button class="picker-cancel-btn" onclick={() => (showNewCollectionInput = false)}>Cancel</button>
            </div>
          {:else}
            <button class="picker-new-btn" onclick={() => (showNewCollectionInput = true)}>
              + Create new collection
            </button>
          {/if}

          <div class="picker-actions">
            <button
              class="picker-cancel-btn"
              data-testid="picker-cancel"
              onclick={() => { showCollectionPicker = false; pendingUploadPath = null; pendingUploadName = null; }}
            >Cancel</button>
            <button
              class="picker-confirm-btn"
              disabled={!pickerCollectionId}
              onclick={confirmUpload}
            >Upload</button>
          </div>
        </div>
      </div>
    {/if}
  </main>
{/if}

<style>
  header {
    text-align: center;
    padding: 1rem 0;
    border-bottom: 1px solid var(--border);
  }

  header h1 {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0;
    letter-spacing: 0.05em;
  }

  .tagline {
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  header nav {
    display: flex;
    justify-content: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }

  .nav-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.35rem 1rem;
    color: var(--text-muted);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.85rem;
    transition: background 0.15s, color 0.15s;
  }

  .nav-btn:hover {
    background: var(--bg-assistant);
    color: var(--text);
  }

  .nav-btn.active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 1rem 0;
  }

  .upload-btn {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 4px;
    padding: 0.35rem 1rem;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.85rem;
    transition: background 0.15s;
  }

  .upload-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .upload-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ── Collection picker ──────────────────────────────────────────── */

  .picker-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .picker-dialog {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1.25rem;
    width: 320px;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .picker-dialog h3 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }

  .picker-error {
    color: #e74c3c;
    font-size: 0.8rem;
    padding: 0.3rem 0.5rem;
    background: rgba(231, 76, 60, 0.1);
    border-radius: 4px;
  }

  .picker-select {
    width: 100%;
    padding: 0.4rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .picker-hint { font-size: 0.85rem; color: var(--text-muted); margin: 0; }

  .picker-new {
    display: flex;
    gap: 0.35rem;
  }

  .picker-new input {
    flex: 1;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .picker-new-btn {
    background: none;
    border: 1px dashed var(--border);
    border-radius: 4px;
    padding: 0.3rem 0.6rem;
    font-size: 0.8rem;
    cursor: pointer;
    color: var(--text-muted);
  }

  .picker-new-btn:hover { border-color: var(--accent); color: var(--accent); }

  .picker-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .picker-cancel-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.35rem 0.7rem;
    font-size: 0.85rem;
    cursor: pointer;
    color: var(--text);
  }

  .picker-create-btn, .picker-confirm-btn {
    border: none;
    border-radius: 4px;
    padding: 0.35rem 0.7rem;
    font-size: 0.85rem;
    cursor: pointer;
    background: var(--accent);
    color: #fff;
    font-weight: 600;
  }

  .picker-confirm-btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
