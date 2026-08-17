<script lang="ts">
  import type { ManualHeading } from '$lib/content/types';

  let { headings, locale }: { headings: ManualHeading[]; locale: 'en' | 'de' } = $props();
  const label = $derived(locale === 'de' ? 'Auf dieser Seite' : 'On this page');
</script>

{#if headings.length > 0}
  <nav class="toc" aria-label={label}>
    <h2>{label}</h2>
    <ol>
      {#each headings as heading (heading.id)}
        <li class:toc__nested={heading.level === 3}>
          <a href={`#${heading.id}`}>{heading.text}</a>
        </li>
      {/each}
    </ol>
  </nav>
{/if}

<style>
  .toc h2 {
    margin: 0 0 var(--s-3);
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  ol {
    display: grid;
    gap: var(--s-2);
    margin: 0;
    padding: 0 0 0 var(--s-3);
    border-left: 1px solid var(--line);
    list-style: none;
  }

  li.toc__nested {
    padding-left: var(--s-3);
  }

  a {
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 0.8125rem;
    line-height: 1.35;
    text-decoration: none;
  }

  a:hover {
    color: var(--gem-bright);
  }
</style>
