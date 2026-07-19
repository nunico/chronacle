<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    controlId: string;
    helpText?: string;
    errorText?: string;
    control?: Snippet<[string | undefined]>;
    children?: Snippet;
  }

  let { label, controlId, helpText, errorText, control, children }: Props = $props();
  let helpId = $derived(helpText ? `${controlId}-help` : undefined);
  let errorId = $derived(errorText ? `${controlId}-error` : undefined);
  let describedBy = $derived([helpId, errorId].filter(Boolean).join(' ') || undefined);
</script>

<div class="field">
  <label for={controlId}>{label}</label>
  <div class="control">
    {#if control}
      {@render control(describedBy)}
    {:else if children}
      {@render children()}
    {/if}
  </div>
  {#if helpText}<p id={helpId} class="help">{helpText}</p>{/if}
  {#if errorText}<p id={errorId} class="error" role="alert">{errorText}</p>{/if}
</div>

<style>
  .field {
    display: grid;
    gap: var(--s-1);
  }

  label {
    color: var(--fg-2);
    font-size: 0.8125rem;
    font-weight: 600;
  }

  .help,
  .error {
    margin: 0;
    font-size: 0.8125rem;
  }

  .help {
    color: var(--fg-3);
  }

  .error {
    color: var(--danger);
  }
</style>
