<script lang="ts">
  import type { Snippet } from 'svelte';

  type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

  interface Props {
    variant?: ButtonVariant;
    loading?: boolean;
    loadingText?: string;
    iconOnly?: boolean;
    ariaLabel?: string;
    disabled?: boolean;
    type?: 'button' | 'submit' | 'reset';
    onclick?: (event: MouseEvent) => void;
    leading?: Snippet;
    trailing?: Snippet;
    children?: Snippet;
    class?: string;
    title?: string;
    testId?: string;
  }

  let {
    variant = 'primary',
    loading = false,
    loadingText = 'Saving…',
    iconOnly = false,
    ariaLabel,
    disabled = false,
    type = 'button',
    onclick,
    leading,
    trailing,
    children,
    class: className = '',
    title,
    testId,
  }: Props = $props();

  let isDisabled = $derived(disabled || loading);
  let iconAriaLabel = $derived.by(() => {
    const normalizedLabel = ariaLabel?.trim();
    if (iconOnly && !normalizedLabel) {
      throw new Error('Icon-only buttons require an ariaLabel');
    }
    return iconOnly ? normalizedLabel : undefined;
  });
</script>

<button
  {type}
  class={['button', 'single-line', variant, { 'icon-only': iconOnly }, className]}
  {title}
  data-testid={testId}
  aria-label={iconAriaLabel}
  aria-busy={loading || undefined}
  disabled={isDisabled}
  {onclick}
>
  {#if loading}
    <span class="spinner" aria-hidden="true"></span>
    <span class="button-label">{loadingText}</span>
  {:else}
    {#if leading}{@render leading()}{/if}
    {#if children}<span class="button-label">{@render children()}</span>{/if}
    {#if trailing}{@render trailing()}{/if}
  {/if}
</button>

<style>
  .button {
    display: inline-flex;
    min-width: 0;
    max-width: 100%;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
    min-height: 36px;
    padding: var(--s-2) var(--s-3);
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    font: 600 0.875rem/1.2 var(--font-sans);
    cursor: pointer;
    white-space: nowrap;
    overflow: visible;
    text-align: center;
    transition:
      background var(--dur-fast) var(--ease-arcane),
      border-color var(--dur-fast) var(--ease-arcane),
      box-shadow var(--dur-fast) var(--ease-arcane);
  }

  .primary {
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
  }

  .secondary {
    border-color: var(--line-strong);
    background: var(--bg-panel-2);
    color: var(--fg-1);
  }

  .ghost {
    border-color: var(--line);
    background: transparent;
    color: var(--fg-2);
  }

  .danger {
    border-color: color-mix(in srgb, var(--danger) 55%, transparent);
    background: var(--danger-bg);
    color: var(--danger);
  }

  .button:hover:not(:disabled) {
    border-color: var(--line-glow);
  }

  .button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .icon-only {
    width: 36px;
    padding: var(--s-2);
  }

  .button-label {
    min-width: 0;
    white-space: nowrap;
  }

  .icon-only .button-label {
    display: none;
  }

  .spinner {
    width: 0.875rem;
    height: 0.875rem;
    border: 2px solid currentcolor;
    border-right-color: transparent;
    border-radius: var(--r-full);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
