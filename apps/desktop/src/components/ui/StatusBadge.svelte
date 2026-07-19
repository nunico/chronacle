<script lang="ts">
  import type { Snippet } from 'svelte';

  type StatusVariant = 'info' | 'success' | 'warning' | 'danger';

  interface Props {
    variant?: StatusVariant;
    text: string;
    children?: Snippet;
  }

  let { variant = 'info', text, children }: Props = $props();
</script>

<span class={['badge', variant]} role="status">
  <span class="dot" aria-hidden="true"></span>
  {#if children}{@render children()}{:else}{text}{/if}
</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
    padding: 3px var(--s-2);
    border: 1px solid currentcolor;
    border-radius: var(--r-full);
    font-size: 0.75rem;
    font-weight: 600;
    line-height: 1.2;
  }

  .dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: var(--r-full);
    background: currentcolor;
  }

  .info {
    background: var(--info-bg);
    color: var(--info);
  }

  .success {
    background: var(--success-bg);
    color: var(--success);
  }

  .warning {
    background: var(--warning-bg);
    color: var(--warning);
  }

  .danger {
    background: var(--danger-bg);
    color: var(--danger);
  }
</style>
