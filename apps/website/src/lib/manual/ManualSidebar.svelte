<script lang="ts">
  import { resolve } from '$app/paths';
  import { articlesFor } from '$lib/content/registry';
  import { manualSections, sectionLabel } from '$lib/content/sections';
  import type { Locale } from '$lib/i18n/types';

  interface Props {
    locale: Locale;
    currentSlug: string;
    navigationLabel?: string;
  }

  let { locale, currentSlug, navigationLabel }: Props = $props();
  const componentId = $props.id();
  const articles = $derived(articlesFor(locale));
  const label = $derived(
    navigationLabel ?? (locale === 'de' ? 'Handbuchnavigation' : 'Manual navigation'),
  );
</script>

<nav class="manual-sidebar" aria-label={label}>
  {#each manualSections as section (section)}
    {@const sectionArticles = articles.filter((article) => article.section === section)}
    {#if sectionArticles.length > 0}
      <section aria-labelledby={`${componentId}-${section}`}>
        <h2 id={`${componentId}-${section}`}>{sectionLabel(locale, section)}</h2>
        <ul>
          {#each sectionArticles as article (article.href)}
            <li>
              <a
                href={resolve(article.href)}
                aria-current={article.slug === currentSlug ? 'page' : undefined}
              >
                {article.navTitle ?? article.title}
              </a>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/each}
</nav>

<style>
  .manual-sidebar {
    display: grid;
    gap: var(--s-6);
    font-family: var(--font-sans);
  }

  section,
  ul {
    margin: 0;
    padding: 0;
  }

  h2 {
    margin: 0 0 var(--s-2);
    color: var(--fg-3);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  ul {
    display: grid;
    gap: 2px;
    list-style: none;
  }

  a {
    display: block;
    padding: var(--s-2) var(--s-3);
    border-left: 1px solid var(--line);
    color: var(--fg-2);
    font-size: 0.875rem;
    line-height: 1.35;
    text-decoration: none;
  }

  a:hover {
    border-left-color: var(--line-strong);
    background: rgb(124 148 255 / 5%);
    color: var(--fg-1);
  }

  a[aria-current='page'] {
    border-left-color: var(--arcane-400);
    background: linear-gradient(90deg, rgb(61 91 255 / 13%), transparent);
    color: var(--gem-bright);
  }
</style>
