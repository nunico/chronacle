<script lang="ts">
  type Status = 'running' | 'done' | 'empty' | 'cancelled' | 'error';

  let {
    status,
    title,
    detail,
    entitiesFound,
    relationsFound,
    onCancel,
  }: {
    status: Status;
    title: string;
    detail: string;
    entitiesFound: number;
    relationsFound: number;
    // Only invoked from the live `running` card; historical cards omit it.
    onCancel?: () => void;
  } = $props();
</script>

<div class="extract-card" class:running={status === 'running'} role="status" aria-live="polite">
  <div class="head">
    {#if status === 'running'}
      <span class="spinner" aria-hidden="true"></span>
    {/if}
    <span class="title">{title}</span>
    {#if status === 'running'}
      <button class="btn-cancel" onclick={() => onCancel?.()}>Cancel</button>
    {/if}
  </div>

  <p class="detail">{detail}</p>

  {#if entitiesFound > 0 || relationsFound > 0}
    <p class="counts">{entitiesFound} entities · {relationsFound} relations</p>
  {/if}
</div>

<style>
  .extract-card {
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--bg-panel-2);
    padding: 12px 14px;
    margin: 8px 0;
  }
  .head { display: flex; align-items: center; gap: 8px; }
  .title { font-weight: 600; color: var(--fg-1); flex: 1; }
  .detail { margin: 6px 0 0; color: var(--fg-2); font-size: 0.9rem; }
  .counts { margin: 4px 0 0; color: var(--fg-3); font-size: 0.8rem; }
  .btn-cancel {
    background: transparent; color: var(--fg-3);
    border: 1px solid var(--line); border-radius: 6px;
    padding: 4px 10px; cursor: pointer; font-size: 0.8rem;
  }
  .spinner {
    width: 12px; height: 12px; border-radius: 50%;
    border: 2px solid var(--line); border-top-color: var(--violet-300);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
