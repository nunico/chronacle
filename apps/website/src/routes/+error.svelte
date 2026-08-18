<script lang="ts">
  import { page } from '$app/state';
  import { resolve } from '$app/paths';
  import EyeMark from '$lib/brand/EyeMark.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import { manualBase, routeLocale } from '$lib/i18n/locale';
  import SearchDialog from '$lib/search/SearchDialog.svelte';
  import ArrowLeft from 'lucide-svelte/icons/arrow-left';
  import BookOpen from 'lucide-svelte/icons/book-open';
  import Search from 'lucide-svelte/icons/search';

  const locale = $derived(routeLocale(page.url.pathname));
  const manualPath = $derived(manualBase(locale));
  const isManualRoute = $derived(
    page.url.pathname === manualPath || page.url.pathname.startsWith(`${manualPath}/`),
  );
  const copy = $derived(
    locale === 'de'
      ? {
          title: 'Diese Seite gibt es nicht.',
          detail: 'Der Link ist vielleicht veraltet oder die Adresse stimmt nicht ganz.',
          home: 'Zur Chronacle-Startseite',
          manual: 'Handbuchüberblick',
          search: 'Handbuch durchsuchen',
          pageTitle: 'Seite nicht gefunden — Chronacle',
        }
      : {
          title: 'That page is not here.',
          detail: 'The link may be out of date, or the address may be slightly off.',
          home: 'Chronacle home',
          manual: 'Manual overview',
          search: 'Search the manual',
          pageTitle: 'Page not found — Chronacle',
        },
  );

  $effect(() => {
    document.documentElement.lang = locale;
  });
</script>

<svelte:head>
  <base href="/" />
  <title>{copy.pageTitle}</title>
  <meta name="robots" content="noindex" />
</svelte:head>

<main id="main-content" class="error-page" lang={locale} data-pagefind-ignore="all">
  <div class="error-page__mark"><EyeMark size={68} glow={false} /></div>
  <p class="error-page__code">404</p>
  <h1>{copy.title}</h1>
  <p class="error-page__detail">{copy.detail}</p>
  <div class="error-page__actions">
    <a class="error-page__primary" href={resolve('/')}>
      <Icon icon={ArrowLeft} size={17} />
      {copy.home}
    </a>
    <a href={resolve(manualPath)}>
      <Icon icon={BookOpen} size={17} />
      {copy.manual}
    </a>
    {#if isManualRoute}
      <button type="button" data-manual-search aria-label={copy.search}>
        <Icon icon={Search} size={17} />
        {copy.search}
      </button>
    {/if}
  </div>
</main>

{#if isManualRoute}
  <SearchDialog {locale} />
{/if}

<style>
  .error-page {
    display: grid;
    min-height: 100dvh;
    place-content: center;
    justify-items: center;
    padding: var(--s-12) var(--s-6);
    color: var(--fg-1);
    text-align: center;
  }

  .error-page__mark {
    margin-bottom: var(--s-5);
  }

  .error-page__code {
    margin: 0 0 var(--s-2);
    color: var(--arcane-300);
    font-family: var(--font-mono);
    font-size: 0.75rem;
    font-weight: 650;
    letter-spacing: 0.16em;
  }

  h1 {
    max-width: 18ch;
    margin: 0;
    font-size: clamp(2rem, 5vw, 3.25rem);
  }

  .error-page__detail {
    max-width: 34rem;
    margin: var(--s-4) 0 0;
    color: var(--fg-2);
    font-family: var(--font-serif);
    font-size: 1.125rem;
  }

  .error-page__actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--s-3);
    margin-top: var(--s-8);
  }

  .error-page__actions a,
  .error-page__actions button {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg-panel);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 0.875rem;
    font-weight: 650;
    text-decoration: none;
    cursor: pointer;
  }

  .error-page__actions a:hover,
  .error-page__actions button:hover {
    border-color: var(--line-strong);
  }

  .error-page__actions .error-page__primary {
    border-color: var(--arcane-400);
    background: var(--grad-action);
    color: white;
  }

  @media (max-width: 32rem) {
    .error-page {
      padding-inline: var(--s-4);
    }

    .error-page__actions {
      width: 100%;
      flex-direction: column;
    }
  }
</style>
