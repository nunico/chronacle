<script lang="ts">
  import type { Snippet } from 'svelte';
  import { modalBehavior } from '../../lib/actions/modal';

  interface Props {
    title: string;
    onclose?: () => void;
    body?: Snippet;
    actions?: Snippet;
    children?: Snippet;
  }

  let { title, onclose, body, actions, children }: Props = $props();
  let titleId = $derived(`dialog-${title.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`);

  function handleClose() {
    onclose?.();
  }
</script>

<div class="overlay">
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    tabindex="-1"
    use:modalBehavior={{ onClose: handleClose }}
  >
    <h2 id={titleId}>{title}</h2>
    <div class="body">
      {#if body}
        {@render body()}
      {:else if children}
        {@render children()}
      {/if}
    </div>
    {#if actions}<footer class="actions">{@render actions()}</footer>{/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    z-index: 200;
    display: grid;
    place-items: center;
    inset: 0;
    padding: var(--s-4);
    background: var(--bg-scrim);
  }

  .dialog {
    width: min(100%, 34rem);
    max-height: min(100%, 44rem);
    overflow: auto;
    padding: var(--s-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-lg);
    background: var(--bg-panel);
    box-shadow: var(--shadow-3);
  }

  h2 {
    margin: 0;
    color: var(--fg-1);
    font-family: var(--font-display);
    font-size: 1.25rem;
  }

  .body {
    margin-top: var(--s-3);
    color: var(--fg-2);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--s-2);
    margin-top: var(--s-5);
  }
</style>
