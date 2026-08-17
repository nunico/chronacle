<script lang="ts">
  import type { Pathname } from '$app/types';
  import ButtonLink from '$lib/components/ButtonLink.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import type { LandingCopy } from '$lib/i18n/landing-copy';
  import BookOpen from 'lucide-svelte/icons/book-open';
  import Download from 'lucide-svelte/icons/download';

  interface Props {
    copy: LandingCopy['download'];
    downloadUrl: string;
    manualPath: Pathname;
  }

  let { copy, downloadUrl, manualPath }: Props = $props();
</script>

<section class="download" id="download" aria-labelledby="download-heading">
  <div class="download__panel">
    <p class="download__eyebrow">{copy.eyebrow}</p>
    <h2 id="download-heading">{copy.heading}</h2>
    <p class="download__body">{copy.body}</p>
    <div class="download__actions">
      <ButtonLink href={downloadUrl} external size="large">
        <Icon icon={Download} size={18} />
        <span>{copy.primary}</span>
      </ButtonLink>
      <ButtonLink href={manualPath} variant="outline" size="large">
        <Icon icon={BookOpen} size={18} />
        <span>{copy.secondary}</span>
      </ButtonLink>
    </div>
    <p class="download__note">{copy.note}</p>
  </div>
</section>

<style>
  .download {
    padding: 0 var(--s-4) var(--s-20);
  }

  .download__panel {
    position: relative;
    width: min(100%, 54rem);
    margin: 0 auto;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-2xl);
    background: linear-gradient(180deg, var(--bg-panel), rgb(16 19 42 / 62%));
    padding: clamp(2rem, 7vw, 3.5rem);
    overflow: hidden;
    box-shadow: var(--shadow-3);
    text-align: center;
  }

  .download__panel::before {
    position: absolute;
    height: 1px;
    inset: 0 12% auto;
    background: linear-gradient(90deg, transparent, var(--line-glow), transparent);
    content: '';
  }

  .download__eyebrow {
    margin: 0 0 var(--s-3);
    color: var(--arcane-300);
    font-family: var(--font-sans);
    font-size: 0.75rem;
    font-weight: 650;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0 0 var(--s-4);
    font-size: clamp(2rem, 5vw, 3rem);
  }

  .download__body {
    max-width: 52ch;
    margin: 0 auto var(--s-6);
    color: var(--fg-2);
    font-family: var(--font-serif);
    font-size: 1.08rem;
  }

  .download__actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--s-3);
  }

  .download__note {
    margin: var(--s-5) 0 0;
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }

  @media (max-width: 28rem) {
    .download__actions :global(.button-link) {
      width: 100%;
    }
  }
</style>
