<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { downloadEmbeddingModel, getEmbeddingProviderStatus } from './lib/commands';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import Button from './components/ui/Button.svelte';
  import ProgressBar from './components/ui/ProgressBar.svelte';
  import { i18n } from './lib/locale.svelte';

  let { onModelReady }: { onModelReady: () => void } = $props();

  let isDownloading = $state(false);
  let progress = $state(0);
  let statusMessage = $state('');
  let currentFile = $state('');
  let bytesDownloaded = $state(0);
  let totalBytes = $state(0);
  let error = $state('');
  let selectedModel = $state('nomic-embed-text-v1.5');

  // Guards against firing onModelReady twice (once from the terminal
  // progress event, once from the command's promise resolving).
  let finished = false;

  function finish() {
    if (finished) return;
    finished = true;
    onModelReady();
  }

  // Delay before auto-check (model might already be cached)
  let checking = $state(true);

  let unlisten: UnlistenFn | null = null;

  onMount(async () => {
    // Only the local backend needs an up-front model download. Cloud users, and
    // platforms without a local ONNX Runtime (e.g. Intel Macs), go straight into
    // the app and configure/use embeddings from there.
    try {
      const status = await getEmbeddingProviderStatus();
      selectedModel =
        status.mode === 'local_multilingual'
          ? 'multilingual-e5-base'
          : status.mode === 'local_nomic'
            ? 'nomic-embed-text-v1.5'
            : (status.model ?? 'nomic-embed-text-v1.5');
      if (status.backend !== 'local' || !status.local_available || status.local_cached) {
        onModelReady();
        return;
      }
    } catch {
      // If status can't be read, fall through to the download screen.
    }
    checking = false;
  });

  onDestroy(() => {
    unlisten?.();
  });

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  async function handleDownload() {
    isDownloading = true;
    error = '';
    progress = 0;
    statusMessage = i18n.t('modelDownload.connecting');

    unlisten = await listen<{
      status: string;
      file: string;
      bytes_downloaded: number;
      total_bytes: number;
      progress: number;
    }>('model-download-progress', (event) => {
      const p = event.payload;
      progress = Math.round(p.progress * 100);
      currentFile = p.file || '';

      if (p.bytes_downloaded !== undefined && p.total_bytes !== undefined) {
        bytesDownloaded = p.bytes_downloaded;
        totalBytes = p.total_bytes;
      }

      switch (p.status) {
        case 'downloading':
          if (p.file) {
            const name = p.file.split('/').pop() || p.file;
            statusMessage = i18n.t('modelDownload.downloading', { name });
          }
          break;
        case 'done':
          statusMessage = i18n.t('modelDownload.ready');
          progress = 100;
          setTimeout(finish, 500);
          break;
        case 'error':
          error = p as unknown as string;
          statusMessage = i18n.t('modelDownload.failed');
          isDownloading = false;
          break;
      }
    });

    try {
      // The command resolves only after the backend has cached the model and
      // swapped in the real embedding provider, so a successful return is the
      // authoritative completion signal — don't rely solely on the terminal
      // progress event, which can be missed.
      await downloadEmbeddingModel();
      statusMessage = i18n.t('modelDownload.ready');
      progress = 100;
      finish();
    } catch (e) {
      error = String(e);
      statusMessage = i18n.t('modelDownload.failed');
      isDownloading = false;
    }
  }
</script>

<div class="download-screen">
  <div class="card">
    <div class="icon">🧠</div>
    <h1>{i18n.t('modelDownload.title')}</h1>
    <p class="desc">
      {i18n.t('modelDownload.description')}
    </p>

    <p class="model-name">{selectedModel} ({i18n.t('settingsPage.dimension')}: 768)</p>

    {#if checking}
      <div class="checking">
        <span class="spinner"></span>
        <span>{i18n.t('modelDownload.checking')}</span>
      </div>
    {:else if !isDownloading && !error}
      <Button onclick={handleDownload}>{i18n.t('modelDownload.start')}</Button>
    {/if}

    {#if isDownloading}
      <div class="progress-section">
        <div class="status-text">{statusMessage}</div>
        {#if currentFile}
          <div class="file-info">
            {currentFile} — {formatBytes(bytesDownloaded)} / {formatBytes(totalBytes)}
          </div>
        {/if}
        <ProgressBar value={progress} label={statusMessage} />
      </div>
    {/if}

    {#if error}
      <div class="error-box">
        <p class="error-title">{i18n.t('modelDownload.failed')}</p>
        <p class="error-detail">{error}</p>
        <Button variant="secondary" onclick={handleDownload}>{i18n.t('modelDownload.retry')}</Button
        >
      </div>
    {/if}
  </div>
</div>

<style>
  .download-screen {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 2rem;
  }

  .card {
    max-width: 480px;
    width: 100%;
    text-align: center;
    background: var(--bg-assistant);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 2.5rem 2rem;
  }

  .icon {
    font-size: 3rem;
    margin-bottom: 1rem;
  }

  h1 {
    font-size: 1.4rem;
    font-weight: 700;
    margin: 0 0 0.75rem;
    font-family: var(--font-display);
  }

  .desc {
    font-size: 0.9rem;
    color: var(--fg-3);
    line-height: 1.6;
    margin: 0 0 1rem;
    font-family: var(--font-sans);
  }

  .model-name {
    font-size: 0.8rem;
    color: var(--arcane-500);
    background: color-mix(in srgb, var(--arcane-500) 10%, transparent);
    display: inline-block;
    padding: 0.25rem 0.75rem;
    border-radius: 4px;
    margin: 0 0 1.5rem;
    font-family: monospace;
  }

  .checking {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    color: var(--fg-3);
    font-size: 0.9rem;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--line);
    border-top-color: var(--arcane-500);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .progress-section {
    margin-top: 1rem;
  }

  .status-text {
    font-size: 0.85rem;
    color: var(--arcane-500);
    font-weight: 600;
    margin-bottom: 0.3rem;
  }

  .file-info {
    font-size: 0.75rem;
    color: var(--fg-3);
    margin-bottom: 0.5rem;
    font-family: monospace;
    word-break: break-all;
  }

  .error-box {
    margin-top: 1rem;
    padding: 0.75rem 1rem;
    background: color-mix(in srgb, #e53e3e 8%, transparent);
    border: 1px solid #e53e3e;
    border-radius: 6px;
  }

  .error-title {
    font-weight: 600;
    color: #e53e3e;
    margin: 0 0 0.3rem;
  }

  .error-detail {
    font-size: 0.8rem;
    color: var(--fg-3);
    margin: 0 0 0.5rem;
    word-break: break-word;
  }
</style>
