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