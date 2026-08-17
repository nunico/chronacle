<script lang="ts">
  import type { Snippet } from 'svelte';

  type Variant = 'note' | 'warning' | 'example';

  let {
    variant = 'note',
    label,
    children,
  }: { variant?: Variant; label?: string; children: Snippet } = $props();

  const visibleLabel = $derived(label ?? variant[0].toUpperCase() + variant.slice(1));
</script>

<aside class={['callout', `callout--${variant}`]} role="note" aria-label={visibleLabel}>
  <strong>{visibleLabel}</strong>
  <div>{@render children()}</div>
</aside>

<style>
  .callout {
    margin: var(--s-6) 0;
    padding: var(--s-4) var(--s-5);
    border: 1px solid var(--line-strong);
    border-left: 3px solid var(--arcane-400);
    border-radius: var(--r-md);
    background: var(--info-bg);
    color: var(--fg-2);
  }

  .callout--warning {
    border-left-color: var(--rune-gold);
    background: var(--warning-bg);
  }

  .callout--example {
    border-left-color: var(--violet-400);
    background: rgb(123 92 255 / 10%);
  }

  strong {
    display: block;
    margin-bottom: var(--s-2);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 0.8125rem;
    letter-spacing: 0.03em;
  }

  div :global(:first-child) {
    margin-top: 0;
  }

  div :global(:last-child) {
    margin-bottom: 0;
  }
</style>
