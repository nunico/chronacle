<script lang="ts">
  import type { Pathname } from '$app/types';
  import ButtonLink from '$lib/components/ButtonLink.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import type { LandingCopy } from '$lib/i18n/landing-copy';
  import BookOpen from 'lucide-svelte/icons/book-open';
  import Download from 'lucide-svelte/icons/download';

  interface Props {
    copy: LandingCopy['hero'];
    downloadUrl: string;
    manualPath: Pathname;
  }

  let { copy, downloadUrl, manualPath }: Props = $props();
</script>

<section class="hero" aria-labelledby="landing-heading">
  <div class="hero__aura" aria-hidden="true"></div>
  <div class="hero__inner">
    <img class="hero__mark" src="/brand/chronacle-icon.png" alt="" width="112" height="112" />
    <p class="hero__eyebrow">{copy.eyebrow}</p>
    <h1 id="landing-heading" aria-label={copy.headingLabel}>{copy.heading}</h1>
    <p class="hero__body">{copy.body}</p>
    <div class="hero__actions">
      <ButtonLink href={downloadUrl} external size="large">
        <Icon icon={Download} size={18} />
        <span>{copy.download}</span>
      </ButtonLink>
      <ButtonLink href={manualPath} variant="outline" size="large">
        <Icon icon={BookOpen} size={18} />
        <span>{copy.manual}</span>
      </ButtonLink>
    </div>
    <p class="hero__trust">{copy.trust}</p>
  </div>
</section>

<style>
  .hero {
    position: relative;
    overflow: hidden;
    padding: clamp(4.5rem, 10vw, 7.5rem) var(--s-4) var(--s-12);
    text-align: center;
  }

  .hero__inner {
    position: relative;
    z-index: 1;
    width: min(100%, 58rem);
    margin: 0 auto;
  }

  .hero__aura {
    position: absolute;
    width: min(52rem, 130vw);
    height: 36rem;
    top: -15rem;
    left: 50%;
    background: var(--aura);
    opacity: 0.85;
    transform: translateX(-50%);
  }

  .hero__mark {
    width: 7rem;
    height: 7rem;
    margin-bottom: var(--s-6);
    border: 1px solid var(--line);
    border-radius: 1.8rem;
    box-shadow: var(--shadow-3);
  }

  .hero__eyebrow {
    display: inline-flex;
    margin: 0 0 var(--s-5);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-full);
    background: var(--info-bg);
    padding: 0.35rem 0.8rem;
    color: var(--arcane-300);
    font-family: var(--font-sans);
    font-size: 0.75rem;
    font-weight: 650;
    letter-spacing: 0.06em;
  }

  h1 {
    max-width: 15ch;
    margin: 0 auto var(--s-5);
    background: linear-gradient(110deg, var(--fg-1) 18%, var(--gem) 58%, var(--violet-300));
    background-clip: text;
    color: transparent;
    font-size: clamp(2.6rem, 7vw, 4.5rem);
    font-weight: 790;
    letter-spacing: 0.005em;
  }

  .hero__body {
    max-width: 62ch;
    margin: 0 auto var(--s-8);
    color: var(--fg-2);
    font-family: var(--font-serif);
    font-size: clamp(1.05rem, 2.4vw, 1.3rem);
    line-height: 1.6;
  }

  .hero__actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--s-3);
  }

  .hero__trust {
    margin: var(--s-5) 0 0;
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }

  @media (max-width: 28rem) {
    .hero {
      padding-top: var(--s-16);
    }

    .hero__actions :global(.button-link) {
      width: 100%;
    }
  }
</style>
