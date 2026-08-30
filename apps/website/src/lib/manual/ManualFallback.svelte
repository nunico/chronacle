<script lang="ts">
  import type { Locale } from '$lib/i18n/types';
  import ManualSidebar from './ManualSidebar.svelte';

  let { locale, currentSlug }: { locale: Locale; currentSlug: string } = $props();
  const navigationLabel = $derived(
    locale === 'de' ? 'Handbuchnavigation ohne JavaScript' : 'Manual navigation without JavaScript',
  );
</script>

<aside class="manual-fallback" data-pagefind-ignore>
  <ManualSidebar {locale} {currentSlug} {navigationLabel} />
</aside>

<style>
  .manual-fallback {
    display: none;
  }

  @media (max-width: 62rem) {
    .manual-fallback {
      display: block;
      width: min(100% - 2rem, var(--reading-width));
      margin: var(--s-4) auto 0;
      padding: var(--s-4);
      border: 1px solid var(--line);
      border-radius: var(--r-md);
      background: var(--bg-panel);
    }
  }
</style>
