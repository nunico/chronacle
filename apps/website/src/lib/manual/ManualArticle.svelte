<script module lang="ts">
  // mdsvex exposes named exports from its layout to manual Markdown documents.
  export { default as Callout } from './Callout.svelte';
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Attachment } from 'svelte/attachments';
  import type { Locale } from '$lib/i18n/types';
  import type { ManualHeading } from '$lib/content/types';

  interface Props {
    children: Snippet;
    title: string;
    summary: string;
    locale: Locale;
    headings?: ManualHeading[];
    onheadings?: (headings: ManualHeading[]) => void;
  }

  let { children, title, summary, locale, headings = [], onheadings }: Props = $props();

  function safeId(text: string): string {
    const id = text
      .normalize('NFKD')
      .replace(/[\u0300-\u036f]/g, '')
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '');
    return id || 'section';
  }

  const collectHeadings: Attachment<HTMLElement> = (node) => {
    const used: string[] = [];
    const collected = Array.from(node.querySelectorAll<HTMLHeadingElement>('h2, h3')).map(
      (heading) => {
        const base = heading.id || safeId(heading.textContent ?? '');
        let id = base;
        let suffix = 2;
        while (used.includes(id)) {
          id = `${base}-${suffix}`;
          suffix += 1;
        }
        used.push(id);
        heading.id = id;
        return {
          id,
          text: heading.textContent?.trim() || id,
          level: heading.tagName === 'H2' ? 2 : 3,
        } satisfies ManualHeading;
      },
    );
    onheadings?.(collected);

    for (const table of node.querySelectorAll<HTMLTableElement>('table')) {
      if (table.parentElement?.classList.contains('manual-table-scroll')) {
        continue;
      }
      const region = document.createElement('div');
      region.className = 'manual-table-scroll';
      region.setAttribute('role', 'region');
      region.setAttribute(
        'aria-label',
        locale === 'de' ? 'Scrollbare Tabelle' : 'Scrollable table',
      );
      region.tabIndex = 0;
      table.before(region);
      region.append(table);
    }
  };

  const indexLabel = $derived(
    locale === 'de' ? 'Abschnitte auf dieser Seite' : 'Sections on this page',
  );
</script>

<article class="manual-article">
  <header>
    <p class="manual-article__eyebrow">
      {locale === 'de' ? 'Chronacle-Handbuch' : 'Chronacle manual'}
    </p>
    <h1 data-pagefind-body>{title}</h1>
    <p class="manual-article__summary" data-pagefind-body>{summary}</p>
  </header>

  {#if headings.length > 0}
    <details class="manual-article__index">
      <summary>{indexLabel}</summary>
      <ol>
        {#each headings as heading (heading.id)}
          <li class:manual-article__index-nested={heading.level === 3}>
            <a href={`#${heading.id}`}>{heading.text}</a>
          </li>
        {/each}
      </ol>
    </details>
  {/if}

  <div class="prose manual-article__body" data-pagefind-body {@attach collectHeadings}>
    {@render children()}
  </div>
</article>

<style>
  .manual-article {
    min-width: 0;
  }

  header {
    max-width: var(--reading-width);
    padding-bottom: var(--s-8);
    border-bottom: 1px solid var(--line-faint);
  }

  .manual-article__eyebrow {
    margin: 0 0 var(--s-3);
    color: var(--arcane-300);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    font-size: clamp(2rem, 5vw, 3.25rem);
    letter-spacing: -0.025em;
  }

  .manual-article__summary {
    max-width: 58ch;
    margin: var(--s-4) 0 0;
    color: var(--fg-2);
    font-family: var(--font-serif);
    font-size: 1.125rem;
  }

  .manual-article__index {
    max-width: var(--reading-width);
    margin: var(--s-6) 0;
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: rgb(16 19 42 / 52%);
    font-family: var(--font-sans);
  }

  .manual-article__index summary {
    color: var(--gem);
    font-weight: 600;
    cursor: pointer;
  }

  .manual-article__index ol {
    display: grid;
    gap: var(--s-2);
    margin: var(--s-3) 0 0;
    padding-left: var(--s-5);
  }

  .manual-article__index-nested {
    margin-left: var(--s-4);
  }

  .manual-article__body {
    padding-top: var(--s-4);
  }

  .manual-article__body :global(h2) {
    margin: var(--s-12) 0 var(--s-4);
    font-size: clamp(1.5rem, 4vw, 2rem);
  }

  .manual-article__body :global(h3) {
    margin: var(--s-8) 0 var(--s-3);
    font-size: 1.25rem;
  }

  .manual-article__body :global(h4) {
    margin: var(--s-6) 0 var(--s-2);
    color: var(--gem);
    font-family: var(--font-sans);
  }

  .manual-article__body :global(pre) {
    max-width: 100%;
    padding: var(--s-4);
    overflow-x: auto;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg-inset);
  }

  .manual-article__body :global(.manual-table-scroll) {
    max-width: 100%;
    overflow-x: auto;
  }

  .manual-article__body :global(table) {
    width: 100%;
    border-collapse: collapse;
  }

  .manual-article__body :global(th),
  .manual-article__body :global(td) {
    padding: var(--s-3);
    border: 1px solid var(--line);
    text-align: left;
  }
</style>
