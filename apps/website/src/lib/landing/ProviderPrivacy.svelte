<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import type { LandingCopy } from '$lib/i18n/landing-copy';
  import HardDrive from 'lucide-svelte/icons/hard-drive';

  interface Props {
    copy: LandingCopy['provider'];
  }

  let { copy }: Props = $props();
</script>

<section class="provider" aria-labelledby="provider-heading">
  <div class="provider__panel">
    <span class="provider__icon"><Icon icon={HardDrive} size={25} /></span>
    <div>
      <p class="provider__eyebrow">{copy.eyebrow}</p>
      <h2 id="provider-heading">{copy.heading}</h2>
      <p class="provider__body">{copy.body}</p>
      <p class="provider__local">{copy.localOption}</p>
    </div>
  </div>
</section>

<style>
  .provider {
    padding: var(--s-4) var(--s-4) var(--s-20);
  }

  .provider__panel {
    display: grid;
    width: min(100%, 58rem);
    grid-template-columns: auto 1fr;
    gap: var(--s-6);
    margin: 0 auto;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-xl);
    background:
      linear-gradient(110deg, rgb(23 28 58 / 82%), rgb(10 12 26 / 76%)),
      url('/brand/tex-circuit.png') right center / auto 100% no-repeat;
    padding: clamp(1.5rem, 5vw, 2.75rem);
    box-shadow: var(--shadow-3);
  }

  .provider__icon {
    display: inline-flex;
    width: 3.25rem;
    height: 3.25rem;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    background: var(--bg-inset);
    color: var(--violet-300);
  }

  .provider__eyebrow {
    margin: 0 0 var(--s-2);
    color: var(--arcane-300);
    font-family: var(--font-sans);
    font-size: 0.75rem;
    font-weight: 650;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0 0 var(--s-4);
    font-size: clamp(1.8rem, 4vw, 2.6rem);
  }

  .provider__body {
    max-width: 64ch;
    margin: 0;
    color: var(--fg-2);
    font-family: var(--font-serif);
    font-size: 1.05rem;
    line-height: 1.65;
  }

  .provider__local {
    margin: var(--s-4) 0 0;
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }

  @media (max-width: 36rem) {
    .provider__panel {
      grid-template-columns: 1fr;
    }
  }
</style>
