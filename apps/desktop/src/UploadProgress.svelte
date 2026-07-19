<script lang="ts">
  import ProgressBar from './components/ui/ProgressBar.svelte';
  import Button from './components/ui/Button.svelte';
  import { i18n } from './lib/locale.svelte';
  export type UploadPhase = 'idle' | 'active' | 'done' | 'error';

  let {
    phase,
    filename,
    status,
    progress,
    onDismiss,
  }: {
    phase: UploadPhase;
    filename: string;
    status: string;
    progress: number;
    onDismiss: () => void;
  } = $props();

  const AUTO_HIDE_MS = 4000;

  // Success state lingers briefly so the user sees completion, then clears
  // itself. Errors stay until explicitly dismissed.
  $effect(() => {
    if (phase !== 'done') return;
    const t = setTimeout(onDismiss, AUTO_HIDE_MS);
    return () => clearTimeout(t);
  });
</script>

{#if phase !== 'idle'}
  <div class="upload-status" class:error={phase === 'error'} role="status">
    <span class="upload-filename">{filename}</span>
    <span class="upload-progress-text">{status}</span>
    {#if phase === 'active' || phase === 'done'}
      <div class="progress-bar-container">
        <ProgressBar value={progress} label={i18n.t('progress.uploadProgress')} />
      </div>
    {/if}
    {#if phase === 'error'}
      <Button variant="ghost" iconOnly ariaLabel={i18n.t('common.dismiss')} onclick={onDismiss}>
        ×
      </Button>
    {/if}
  </div>
{/if}

<style>
  .upload-status {
    margin-top: 0.5rem;
    font-size: 0.8rem;
    color: var(--fg-3);
    text-align: center;
    font-family: var(--font-sans);
  }

  .upload-filename {
    font-weight: 600;
    margin-right: 0.5rem;
  }

  .upload-progress-text {
    color: var(--arcane-500);
  }

  .upload-status.error .upload-progress-text {
    color: var(--danger);
  }

  .progress-bar-container {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    margin-top: 0.3rem;
  }

  .progress-bar-container {
    width: 200px;
  }
</style>
