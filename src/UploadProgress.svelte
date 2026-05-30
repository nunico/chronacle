<script lang="ts">
  let {
    filename,
    status,
    progress,
    isActive,
  }: {
    filename: string;
    status: string;
    progress: number;
    isActive: boolean;
  } = $props();
</script>

{#if isActive || status}
  <div class="upload-status">
    <span class="upload-filename">{filename}</span>
    <span class="upload-progress-text">{status}</span>
    {#if isActive}
      <div class="progress-bar-container">
        <div class="progress-bar">
          <div class="progress-fill" style="width: {progress}%"></div>
        </div>
        <span class="progress-pct">{progress}%</span>
      </div>
    {/if}
  </div>
{/if}

<style>
  .upload-status {
    margin-top: 0.5rem;
    font-size: 0.8rem;
    color: var(--text-muted);
    text-align: center;
  }

  .upload-filename {
    font-weight: 600;
    margin-right: 0.5rem;
  }

  .upload-progress-text {
    color: var(--accent);
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
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.3s ease;
  }

  .progress-pct {
    font-size: 0.75rem;
    color: var(--text-muted);
    min-width: 2.5rem;
    text-align: right;
  }
</style>