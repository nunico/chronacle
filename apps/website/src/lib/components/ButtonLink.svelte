<script lang="ts">
  import { resolve } from '$app/paths';
  import type { Pathname } from '$app/types';
  import type { Snippet } from 'svelte';

  type Variant = 'primary' | 'outline' | 'ghost';
  type Size = 'default' | 'large' | 'compact';

  interface Props {
    href: string;
    children: Snippet;
    variant?: Variant;
    size?: Size;
    external?: boolean;
    target?: '_blank' | '_self';
    rel?: string;
    class?: string;
  }

  let {
    href,
    children,
    variant = 'primary',
    size = 'default',
    external = false,
    target,
    rel,
    class: className = '',
  }: Props = $props();
</script>

{#if external}
  <a
    class={['button-link', `button-link--${variant}`, `button-link--${size}`, className]}
    {href}
    target={target ?? '_blank'}
    rel="external noopener noreferrer"
  >
    {@render children()}
  </a>
{:else}
  <a
    class={['button-link', `button-link--${variant}`, `button-link--${size}`, className]}
    href={resolve(href as Pathname)}
    {target}
    {rel}
  >
    {@render children()}
  </a>
{/if}

<style>
  .button-link {
    position: relative;
    display: inline-flex;
    min-height: 2.75rem;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
    border: 1px solid transparent;
    border-radius: var(--r-md);
    padding: 0.7rem 1.15rem;
    overflow: hidden;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 0.9375rem;
    font-weight: 650;
    line-height: 1;
    text-decoration: none;
    transition:
      border-color var(--dur) var(--ease-arcane),
      background-color var(--dur) var(--ease-arcane),
      box-shadow var(--dur) var(--ease-arcane),
      color var(--dur) var(--ease-arcane),
      filter var(--dur) var(--ease-arcane),
      transform var(--dur-fast) var(--ease-arcane);
  }

  .button-link::before {
    position: absolute;
    width: 56%;
    height: 1px;
    top: 0;
    left: 22%;
    background: linear-gradient(90deg, transparent, var(--gem), transparent);
    content: '';
    opacity: 0.42;
  }

  .button-link:hover {
    color: var(--fg-on-accent);
    transform: translateY(-1px);
  }

  .button-link:active {
    box-shadow: none;
    transform: scale(0.97);
  }

  .button-link--primary {
    background: var(--grad-arcane);
    box-shadow:
      var(--glow-arcane),
      inset 0 1px 0 rgb(255 255 255 / 22%);
    color: var(--fg-on-accent);
  }

  .button-link--primary:hover {
    filter: brightness(1.1);
  }

  .button-link--outline {
    border-color: var(--line-strong);
    background: rgb(10 12 26 / 64%);
  }

  .button-link--outline:hover {
    border-color: var(--line-glow);
    background: var(--bg-panel);
    box-shadow: var(--glow-arcane);
  }

  .button-link--ghost {
    color: var(--fg-2);
  }

  .button-link--ghost::before {
    display: none;
  }

  .button-link--ghost:hover {
    background: var(--info-bg);
    color: var(--fg-1);
  }

  .button-link--large {
    min-height: 3.25rem;
    padding: 0.9rem 1.5rem;
    font-size: 1rem;
  }

  .button-link--compact {
    min-height: 2.35rem;
    padding: 0.55rem 0.8rem;
    font-size: 0.875rem;
  }
</style>
