<script module lang="ts">
  import type { Pathname } from '$app/types';

  export interface SiteFooterLabels {
    home: string;
    tagline: string;
    navigation: string;
    manual: string;
    source: string;
    license: string;
    copyright: string;
  }

  export interface SiteFooterLinks {
    home: Pathname;
    manual: Pathname;
    source: string;
    license: Pathname;
  }

  export interface SiteFooterProps {
    labels: SiteFooterLabels;
    links: SiteFooterLinks;
  }
</script>

<script lang="ts">
  import { resolve } from '$app/paths';
  import EyeMark from '$lib/brand/EyeMark.svelte';

  let { labels, links }: SiteFooterProps = $props();
</script>

<footer class="site-footer">
  <div class="site-footer__inner">
    <div class="site-footer__brand">
      <a href={resolve(links.home)} aria-label={labels.home}>
        <EyeMark size={31} glow={false} />
        <span>Chronacle</span>
      </a>
      <p>{labels.tagline}</p>
    </div>

    <nav aria-label={labels.navigation}>
      <a href={resolve(links.manual)}>{labels.manual}</a>
      <a href={links.source} rel="external">{labels.source}</a>
      <a href={resolve(links.license)}>{labels.license}</a>
    </nav>

    <p class="site-footer__legal">{labels.copyright}</p>
  </div>
</footer>

<style>
  .site-footer {
    position: relative;
    border-top: 1px solid var(--line-faint);
    color: var(--fg-3);
  }

  .site-footer__inner {
    display: grid;
    width: min(100%, calc(var(--content-width) + 3rem));
    grid-template-columns: minmax(14rem, 1fr) auto;
    gap: var(--s-8) var(--s-12);
    margin: 0 auto;
    padding: var(--s-10) var(--s-6) var(--s-12);
  }

  .site-footer__brand a {
    display: inline-flex;
    align-items: center;
    gap: var(--s-3);
    color: var(--fg-1);
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.04em;
    text-decoration: none;
  }

  .site-footer__brand p {
    max-width: 34rem;
    margin: var(--s-3) 0 0;
    font-family: var(--font-serif);
  }

  nav {
    display: flex;
    flex-wrap: wrap;
    align-content: start;
    justify-content: flex-end;
    gap: var(--s-5);
    padding-top: var(--s-2);
    font-family: var(--font-sans);
    font-size: 0.875rem;
  }

  nav a {
    color: var(--fg-2);
    text-decoration: none;
  }

  nav a:hover {
    color: var(--fg-1);
  }

  .site-footer__legal {
    grid-column: 1 / -1;
    margin: 0;
    padding-top: var(--s-5);
    border-top: 1px solid var(--line-faint);
    color: var(--fg-3);
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }

  @media (max-width: 40rem) {
    .site-footer__inner {
      grid-template-columns: 1fr;
      padding-inline: var(--s-4);
    }

    nav {
      justify-content: flex-start;
    }
  }
</style>
