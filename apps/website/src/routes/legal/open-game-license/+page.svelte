<script lang="ts">
  import { asset, resolve } from '$app/paths';
  import EyeMark from '$lib/brand/EyeMark.svelte';
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import {
    OFFICIAL_SRD_SOURCE_URL,
    OPEN_GAME_CONTENT_LABEL,
    OPEN_GAME_LICENSE_PDF_PATH,
  } from '$lib/legal/open-game-content';

  const sourceUrl = 'https://github.com/nunico/chronacle';
  const projectLicenseUrl = 'https://github.com/nunico/chronacle/blob/main/LICENSE';
  const licensePdfUrl = asset(OPEN_GAME_LICENSE_PDF_PATH);
</script>

<svelte:head>
  <title>Open Game License · Chronacle</title>
  <meta
    name="description"
    content="Open Game Content identification and the complete license copy for Chronacle's SRD 5.1 example."
  />
</svelte:head>

<div class="legal-page" data-pagefind-ignore="all">
  <header class="legal-header">
    <a href={resolve('/')} aria-label="Chronacle home">
      <EyeMark size={34} glow={false} />
      <span>Chronacle</span>
    </a>
  </header>

  <article class="legal-card">
    <p class="eyebrow">Legal</p>
    <h1>Open Game Content</h1>
    <p class="lede">
      The parts identified below use Open Game Content from the System Reference Document 5.1.
    </p>

    <section aria-labelledby="identification-heading">
      <h2 id="identification-heading">Identification</h2>
      <p>
        In the product example on Chronacle's home page, the question, ruling, explanation, source
        line, and quotation inside the panel marked <strong>{OPEN_GAME_CONTENT_LABEL}</strong> are Open
        Game Content. No other part of this website is designated as Open Game Content.
      </p>
    </section>

    <section aria-labelledby="license-heading">
      <h2 id="license-heading">License and source</h2>
      <p>
        Read the complete, unmodified Open Game License Version 1.0a and Section 15 notice in the
        two-page license copy below.
      </p>
      <div class="actions">
        <a class="primary-link" href={licensePdfUrl}>Open the license PDF</a>
        <a href={OFFICIAL_SRD_SOURCE_URL} target="_blank" rel="external noopener noreferrer">
          Open the official SRD 5.1 source document
        </a>
      </div>
      <object
        class="license-frame"
        data={licensePdfUrl}
        type="application/pdf"
        aria-label="Open Game License Version 1.0a and Section 15 notice"
      >
        <p>
          This browser cannot display the PDF inline.
          <a href={licensePdfUrl}>Open the license PDF instead.</a>
        </p>
      </object>
    </section>
  </article>

  <SiteFooter
    labels={{
      home: 'Chronacle home',
      tagline: 'Check sources instead of guessing answers.',
      navigation: 'Footer navigation',
      manual: 'Manual',
      source: 'Source',
      license: 'Project license',
      copyright: 'Chronacle · AGPL-3.0 with Branding Exception',
    }}
    links={{
      home: '/',
      manual: '/en/manual',
      source: sourceUrl,
      license: projectLicenseUrl,
    }}
  />
</div>

<style>
  .legal-page {
    min-height: 100vh;
  }

  .legal-header {
    border-bottom: 1px solid var(--line-faint);
    background: rgb(5 6 15 / 76%);
    backdrop-filter: blur(16px);
  }

  .legal-header > a {
    display: flex;
    width: min(100%, calc(var(--content-width) + 3rem));
    min-height: 4.25rem;
    align-items: center;
    gap: var(--s-3);
    margin: 0 auto;
    padding: var(--s-2) var(--s-6);
    color: var(--fg-1);
    font-family: var(--font-display);
    font-weight: 750;
    letter-spacing: 0.04em;
    text-decoration: none;
  }

  .legal-card {
    width: min(calc(100% - 2rem), 58rem);
    margin: var(--s-12) auto var(--s-20);
    border: 1px solid var(--line);
    border-radius: var(--r-xl);
    background: rgb(16 19 42 / 82%);
    padding: clamp(1.25rem, 4vw, 3rem);
    box-shadow: var(--shadow-3);
  }

  .eyebrow {
    margin: 0 0 var(--s-3);
    color: var(--rune-gold);
    font-family: var(--font-mono);
    font-size: 0.75rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    font-size: clamp(2rem, 7vw, 3.75rem);
  }

  .lede {
    max-width: 48rem;
    margin: var(--s-5) 0 var(--s-10);
    color: var(--fg-2);
    font-size: 1.15rem;
  }

  section + section {
    margin-top: var(--s-10);
    padding-top: var(--s-8);
    border-top: 1px solid var(--line-faint);
  }

  h2 {
    margin: 0 0 var(--s-3);
    font-size: 1.35rem;
  }

  section p {
    max-width: var(--reading-width);
    color: var(--fg-2);
  }

  strong {
    color: var(--rune-gold);
    font-family: var(--font-mono);
    font-size: 0.88em;
    font-weight: 550;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3) var(--s-5);
    align-items: center;
    margin: var(--s-5) 0 var(--s-6);
    font-family: var(--font-sans);
  }

  .primary-link {
    display: inline-flex;
    min-height: 2.75rem;
    align-items: center;
    border: 1px solid var(--line-glow);
    border-radius: var(--r-md);
    background: var(--grad-action);
    padding: var(--s-2) var(--s-4);
    color: var(--fg-on-accent);
    font-weight: 650;
    text-decoration: none;
  }

  .license-frame {
    width: 100%;
    min-height: min(74vh, 52rem);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    background: white;
  }

  @media (max-width: 40rem) {
    .legal-header > a {
      padding-inline: var(--s-4);
    }

    .legal-card {
      margin-block: var(--s-6) var(--s-12);
    }

    .license-frame {
      min-height: 34rem;
    }
  }
</style>
