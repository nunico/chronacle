<script lang="ts">
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
        <div class="progress-bar">
          <div class="progress-fill" style="width: {progress}%"></div>
        </div>
        <span class="progress-pct">{progress}%</span>
      </div>
    {/if}
    {#if phase === 'error'}
      <button type="button" class="dismiss-btn" aria-label="Dismiss" onclick={onDismiss}>
        ×
      </button>
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

  .dismiss-btn {
    margin-left: 0.5rem;
    background: none;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-3);
    font-size: 0.85rem;
    line-height: 1;
    padding: 1px 6px;
    cursor: pointer;
  }

  .dismiss-btn:hover {
    color: var(--fg-1);
    border-color: var(--line-strong);
  }

  .progress-bar-container {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    margin-top: 0.3rem;
  }

  .progress-bar {
    width: 200px;
    height: 4px;
    background: var(--line);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--arcane-500);
    transition: width 0.3s ease;
  }

  .progress-pct {
    font-size: 0.75rem;
    color: var(--fg-3);
    min-width: 2.5rem;
    text-align: right;
  }
</style>
