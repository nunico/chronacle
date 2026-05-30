<script lang="ts">
  import { onMount } from 'svelte';
  import { getCampaigns, uploadSource, type Campaign } from './lib/commands';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
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

  async function selectAndUploadPdf() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;

    const path = selected as string;
    const name = path.split('/').pop()?.split('\\').pop() || 'document.pdf';
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
        },
      );

      await uploadSource(path, name, 'rules', activeCampaignId ?? undefined);
    } catch (e) {
      uploadStatus = `Failed: ${e}`;
      console.error('Upload failed:', e);
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
      <button class="upload-btn" onclick={selectAndUploadPdf} disabled={isUploading}>
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
</style>