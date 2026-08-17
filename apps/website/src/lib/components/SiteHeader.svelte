<script module lang="ts">
  import type { Locale } from '$lib/i18n/types';
  import type { Pathname } from '$app/types';

  export interface SiteHeaderLabels {
    home: string;
    manual: string;
    source: string;
    download: string;
    language: string;
    english: string;
    german: string;
    navigation: string;
  }

  export interface SiteHeaderLinks {
    home: Pathname;
    manual: Pathname;
    source: string;
    download: string;
  }

  export interface SiteHeaderProps {
    locale: Locale;
    labels: SiteHeaderLabels;
    links: SiteHeaderLinks;
    onlocalechange: (locale: Locale) => void;
  }
</script>

<script lang="ts">
  import { resolve } from '$app/paths';
  import EyeMark from '$lib/brand/EyeMark.svelte';
  import BookOpen from 'lucide-svelte/icons/book-open';
  import Download from 'lucide-svelte/icons/download';
  import GitFork from 'lucide-svelte/icons/git-fork';
  import ButtonLink from './ButtonLink.svelte';
  import Icon from './Icon.svelte';
  import LanguageSwitch from './LanguageSwitch.svelte';

  let { locale, labels, links, onlocalechange }: SiteHeaderProps = $props();
</script>

<header class="site-header" data-pagefind-ignore>
  <nav class="site-header__inner" aria-label={labels.navigation}>
    <a class="brand" href={resolve(links.home)} aria-label={labels.home}>
      <span class="brand-eye"><EyeMark size={38} glow={false} /></span>
      <span>Chron<span aria-hidden="true">a</span>cle</span>
    </a>

    <div class="site-header__links">
      <a href={resolve(links.manual)}>
        <Icon icon={BookOpen} size={16} />
        <span>{labels.manual}</span>
      </a>
      <a href={links.source} rel="external">
        <Icon icon={GitFork} size={16} />
        <span>{labels.source}</span>
      </a>
    </div>

    <div class="site-header__actions">
      <LanguageSwitch
        value={locale}
        label={labels.language}
        englishLabel={labels.english}
        germanLabel={labels.german}
        onchange={onlocalechange}
      />
      <ButtonLink href={links.download} external size="compact">
        <Icon icon={Download} size={16} />
        <span>{labels.download}</span>
      </ButtonLink>
    </div>
  </nav>
</header>

<style>
  .site-header {
    position: sticky;
    z-index: 30;
    top: 0;
    border-bottom: 1px solid var(--line-faint);
    background: rgb(5 6 15 / 76%);
    backdrop-filter: blur(16px);
  }

  .site-header__inner {
    display: flex;
    width: min(100%, calc(var(--content-width) + 3rem));
    min-height: 4.25rem;
    align-items: center;
    gap: var(--s-6);
    margin: 0 auto;
    padding: var(--s-2) var(--s-6);
    font-family: var(--font-sans);
  }

  .brand {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    gap: var(--s-3);
    color: var(--fg-1);
    text-decoration: none;
  }

  .brand-eye {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .brand > span {
    font-family: var(--font-display);
    font-size: 1.125rem;
    font-weight: 780;
    letter-spacing: 0.04em;
  }

  .brand > span > span {
    color: var(--violet-400);
  }

  .site-header__links,
  .site-header__actions {
    display: flex;
    align-items: center;
  }

  .site-header__links {
    gap: var(--s-6);
    margin-left: var(--s-4);
  }

  .site-header__links a {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    color: var(--fg-2);
    font-size: 0.875rem;
    font-weight: 550;
    text-decoration: none;
    transition: color var(--dur);
  }

  .site-header__links a:hover {
    color: var(--fg-1);
  }

  .site-header__actions {
    gap: var(--s-3);
    margin-left: auto;
  }

  @media (max-width: 44rem) {
    .site-header__inner {
      gap: var(--s-3);
      padding-inline: var(--s-4);
    }

    .brand > span:last-child {
      display: none;
    }

    .site-header__links {
      gap: var(--s-3);
      margin-left: 0;
    }
  }

  @media (max-width: 36rem) {
    .site-header__links a > span,
    .site-header__actions :global(.button-link span) {
      position: absolute;
      width: 1px;
      height: 1px;
      padding: 0;
      overflow: hidden;
      clip: rect(0, 0, 0, 0);
      white-space: nowrap;
      border: 0;
    }
  }
</style>
