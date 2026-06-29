<script lang="ts">
  import { toasts, dismissToast } from '../lib/toast.svelte';
  import Icon from './Icon.svelte';
</script>

{#if toasts.length > 0}
  <div class="toast-stack">
    {#each toasts as t (t.id)}
      <div class="toast {t.variant}" role={t.variant === 'error' ? 'alert' : 'status'}>
        <span class="toast-msg">{t.message}</span>
        {#if t.variant === 'error'}
          <button
            type="button"
            class="toast-dismiss"
            aria-label="Dismiss"
            onclick={() => dismissToast(t.id)}
          >
            <Icon name="x" size={14} />
          </button>
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .toast-stack {
    position: fixed;
    bottom: 20px;
    right: 20px;
    z-index: 300;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: min(420px, 90vw);
  }
  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--r-md);
    border: 1px solid var(--line-strong);
    background: var(--bg-panel);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
    line-height: 1.45;
    box-shadow: var(--shadow-3);
  }
  .toast.success {
    border-left: 3px solid var(--success);
  }
  .toast.info {
    border-left: 3px solid var(--arcane-400);
  }
  .toast.error {
    border-left: 3px solid var(--danger);
    background: var(--danger-bg, rgba(242, 103, 75, 0.08));
  }
  .toast-msg {
    flex: 1;
    word-break: break-word;
  }
  .toast-dismiss {
    flex: none;
    background: none;
    border: 0;
    padding: 2px;
    color: var(--fg-3);
    cursor: pointer;
  }
  .toast-dismiss:hover {
    color: var(--fg-1);
  }
</style>
