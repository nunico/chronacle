<script lang="ts">
  import { resolve } from '$app/paths';
  import { articlesFor, getTranslation } from '$lib/content/registry';
  import type { ManualArticle } from '$lib/content/types';

  let { article }: { article: ManualArticle } = $props();

  const adjacent = $derived.by(() => {
    const articles = articlesFor(article.locale);
    const index = articles.findIndex((candidate) => candidate.slug === article.slug);
    return {
      previous: index > 0 ? articles[index - 1] : undefined,
      next: index >= 0 ? articles[index + 1] : undefined,
    };
  });
  const translation = $derived(getTranslation(article.locale, article.slug));
  const labels = $derived(
    article.locale === 'de'
      ? { previous: 'Zurück', next: 'Weiter', language: 'English' }
      : { previous: 'Previous', next: 'Next', language: 'Deutsch' },
  );
</script>

<nav
  class="article-navigation"
  aria-label={article.locale === 'de' ? 'Artikelnavigation' : 'Article navigation'}
>
  <div class="article-navigation__sequence">
    {#if adjacent.previous}
      <a class="article-navigation__previous" href={resolve(adjacent.previous.href)}>
        <span>{labels.previous}</span>
        <strong>{adjacent.previous.navTitle ?? adjacent.previous.title}</strong>
      </a>
    {/if}
    {#if adjacent.next}
      <a class="article-navigation__next" href={resolve(adjacent.next.href)}>
        <span>{labels.next}</span>
        <strong>{adjacent.next.navTitle ?? adjacent.next.title}</strong>
      </a>
    {/if}
  </div>
  <a
    class="article-navigation__translation"
    href={resolve(translation.href)}
    hreflang={translation.locale}
  >
    {labels.language}
  </a>
</nav>

<style>
  .article-navigation {
    display: grid;
    gap: var(--s-6);
    margin-top: var(--s-12);
    padding-top: var(--s-8);
    border-top: 1px solid var(--line);
    font-family: var(--font-sans);
  }

  .article-navigation__sequence {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--s-4);
  }

  .article-navigation__sequence a {
    display: grid;
    gap: var(--s-1);
    min-height: 5rem;
    padding: var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: rgb(16 19 42 / 58%);
    text-decoration: none;
  }

  .article-navigation__sequence a:hover {
    border-color: var(--line-strong);
    background: var(--bg-panel);
  }

  .article-navigation__next {
    grid-column: 2;
    text-align: right;
  }

  span {
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  strong {
    color: var(--gem);
    font-size: 0.9375rem;
  }

  .article-navigation__translation {
    justify-self: start;
    color: var(--fg-2);
    font-size: 0.875rem;
  }

  @media (max-width: 34rem) {
    .article-navigation__sequence {
      grid-template-columns: 1fr;
    }

    .article-navigation__next {
      grid-column: 1;
      text-align: left;
    }
  }
</style>
