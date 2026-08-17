<script lang="ts">
  import { resolve } from '$app/paths';
  import EyeMark from '$lib/brand/EyeMark.svelte';
  import type { ManualArticle, ManualHeading } from '$lib/content/types';
  import ArticleNavigation from './ArticleNavigation.svelte';
  import Breadcrumbs from './Breadcrumbs.svelte';
  import ManualDrawer from './ManualDrawer.svelte';
  import ManualSidebar from './ManualSidebar.svelte';
  import TableOfContents from './TableOfContents.svelte';

  let { article }: { article: ManualArticle } = $props();

  const Article = $derived(article.component);
  let headings = $derived<ManualHeading[]>(article.headings ?? []);
  const homeLabel = $derived(
    article.locale === 'de' ? 'Zur Chronacle-Startseite' : 'Chronacle home',
  );

  function setHeadings(nextHeadings: ManualHeading[]): void {
    headings = nextHeadings;
  }
</script>

<header class="manual-header">
  <div class="manual-header__inner">
    <a class="manual-header__brand" href={resolve('/')} aria-label={homeLabel}>
      <EyeMark size={34} glow={false} />
      <span>Chron<span aria-hidden="true">a</span>cle</span>
    </a>
    <span class="manual-header__divider" aria-hidden="true"></span>
    <span class="manual-header__title">{article.locale === 'de' ? 'Handbuch' : 'Manual'}</span>
    <div class="manual-header__drawer">
      <ManualDrawer locale={article.locale} currentSlug={article.slug} />
    </div>
  </div>
</header>

<div class="manual-shell">
  <aside class="manual-shell__sidebar">
    <ManualSidebar locale={article.locale} currentSlug={article.slug} />
  </aside>

  <section
    class="manual-shell__content"
    aria-label={article.locale === 'de' ? 'Handbuchartikel' : 'Manual article'}
  >
    <Breadcrumbs {article} />
    <Article onheadings={setHeadings} />
    <ArticleNavigation {article} />
  </section>

  <aside
    class="manual-shell__toc"
    aria-label={article.locale === 'de' ? 'Seitenübersicht' : 'Page outline'}
  >
    <TableOfContents {headings} locale={article.locale} />
  </aside>
</div>

<style>
  .manual-header {
    position: sticky;
    z-index: 20;
    top: 0;
    border-bottom: 1px solid var(--line-faint);
    background: rgb(5 6 15 / 84%);
    backdrop-filter: blur(18px);
  }

  .manual-header__inner {
    display: flex;
    width: min(100%, 96rem);
    min-height: 4.25rem;
    align-items: center;
    gap: var(--s-3);
    margin: 0 auto;
    padding: var(--s-2) var(--s-6);
    font-family: var(--font-sans);
  }

  .manual-header__brand {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    color: var(--fg-1);
    font-family: var(--font-display);
    font-weight: 750;
    letter-spacing: 0.04em;
    text-decoration: none;
  }

  .manual-header__brand > span > span {
    color: var(--violet-400);
  }

  .manual-header__divider {
    width: 1px;
    height: 1.5rem;
    margin-left: var(--s-1);
    background: var(--line);
  }

  .manual-header__title {
    color: var(--fg-2);
    font-size: 0.875rem;
    font-weight: 600;
  }

  .manual-header__drawer {
    margin-left: auto;
  }

  .manual-shell {
    display: grid;
    width: min(100%, 96rem);
    grid-template-columns: 14rem minmax(0, var(--reading-width)) 12rem;
    justify-content: center;
    gap: clamp(2rem, 4vw, 4rem);
    margin: 0 auto;
    padding: var(--s-8) var(--s-6) var(--s-16);
  }

  .manual-shell__sidebar,
  .manual-shell__toc {
    position: sticky;
    top: 6.25rem;
    align-self: start;
    max-height: calc(100vh - 8rem);
    overflow-y: auto;
  }

  .manual-shell__content {
    min-width: 0;
  }

  .manual-shell__content > :global(.breadcrumbs) {
    margin-bottom: var(--s-8);
  }

  @media (max-width: 74rem) {
    .manual-shell {
      grid-template-columns: 14rem minmax(0, var(--reading-width));
    }

    .manual-shell__toc {
      display: none;
    }
  }

  @media (max-width: 62rem) {
    .manual-shell {
      grid-template-columns: minmax(0, var(--reading-width));
      padding-top: var(--s-6);
    }

    .manual-shell__sidebar {
      display: none;
    }
  }

  @media (max-width: 36rem) {
    .manual-header__inner,
    .manual-shell {
      padding-inline: var(--s-4);
    }

    .manual-header__brand > span {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip: rect(0 0 0 0);
      white-space: nowrap;
    }
  }
</style>
