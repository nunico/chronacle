<script lang="ts">
  import { resolve } from '$app/paths';
  import { manualBase } from '$lib/i18n/locale';
  import type { ManualArticle } from '$lib/content/types';
  import { sectionLabel } from '$lib/content/sections';

  let { article }: { article: ManualArticle } = $props();

  const homeLabel = $derived(article.locale === 'de' ? 'Startseite' : 'Home');
  const manualLabel = $derived(article.locale === 'de' ? 'Handbuch' : 'Manual');
</script>

<nav
  class="breadcrumbs"
  aria-label={article.locale === 'de' ? 'Brotkrümelnavigation' : 'Breadcrumbs'}
  data-pagefind-ignore
>
  <ol>
    <li><a href={resolve('/')}>{homeLabel}</a></li>
    <li><a href={resolve(manualBase(article.locale))}>{manualLabel}</a></li>
    <li><span>{sectionLabel(article.locale, article.section)}</span></li>
    <li aria-current="page"><span>{article.title}</span></li>
  </ol>
</nav>

<style>
  .breadcrumbs {
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 0.75rem;
  }

  ol {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
    align-items: center;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li:not(:last-child)::after {
    margin-left: var(--s-2);
    color: var(--fg-4);
    content: '/';
  }

  a {
    color: var(--fg-2);
    text-decoration: none;
  }

  li:last-child {
    color: var(--gem);
  }
</style>
