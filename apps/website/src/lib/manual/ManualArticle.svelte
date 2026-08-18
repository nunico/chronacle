<script module lang="ts">
  // mdsvex exposes named exports from its layout to manual Markdown documents.
  export { default as Callout } from './Callout.svelte';
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { Attachment } from 'svelte/attachments';
  import type { Locale } from '$lib/i18n/types';
  import type { ManualHeading, ManualSectionId } from '$lib/content/types';
  import { sectionLabel } from '$lib/content/sections';

  interface Props {
    children: Snippet;
    title: string;
    summary: string;
    locale: Locale;
    section: ManualSectionId;
    headings?: ManualHeading[];
    onheadings?: (headings: ManualHeading[]) => void;
  }

  let { children, title, summary, locale, section, headings = [], onheadings }: Props = $props();

  function safeId(text: string): string {
    const id = text
      .normalize('NFKD')
      .replace(/[\u0300-\u036f]/g, '')
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '');
    return id || 'section';
  }

  function validId(id: string): boolean {
    return /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(id);
  }

  const collectHeadings: Attachment<HTMLElement> = (node) => {
    const used: string[] = [];
    const permalinkCleanups: (() => void)[] = [];
    const headingRows: Array<{ heading: HTMLHeadingElement; row: HTMLDivElement }> = [];
    const tableRegions: HTMLDivElement[] = [];
    const collected = Array.from(node.querySelectorAll<HTMLHeadingElement>('h2, h3')).map(
      (heading) => {
        const headingText = heading.textContent?.trim() || 'Section';
        const base = validId(heading.id) ? heading.id : safeId(headingText);
        let id = base;
        let suffix = 2;
        while (used.includes(id)) {
          id = `${base}-${suffix}`;
          suffix += 1;
        }
        used.push(id);
        heading.id = id;

        const row = document.createElement('div');
        row.className = 'manual-heading-row';
        heading.before(row);
        row.append(heading);
        headingRows.push({ heading, row });

        const link = document.createElement('a');
        const status = document.createElement('span');
        const copyLabel =
          locale === 'de' ? `Link zu ${headingText} kopieren` : `Copy link to ${headingText}`;
        link.className = 'manual-heading-permalink';
        link.dataset.manualPermalink = '';
        link.setAttribute('data-pagefind-ignore', '');
        link.href = `#${id}`;
        link.setAttribute('aria-label', copyLabel);
        link.textContent = '#';
        status.className = 'manual-heading-permalink__status';
        status.setAttribute('aria-live', 'polite');
        link.append(status);

        const copyLink = (event: MouseEvent): void => {
          if (typeof navigator.clipboard?.writeText !== 'function') {
            return;
          }
          event.preventDefault();
          const url = new URL(`#${id}`, window.location.href).href;
          void navigator.clipboard.writeText(url).then(
            () => {
              status.textContent = locale === 'de' ? 'Link kopiert' : 'Link copied';
            },
            () => {
              window.location.hash = id;
            },
          );
        };
        link.addEventListener('click', copyLink);
        heading.after(link);
        permalinkCleanups.push(() => {
          link.removeEventListener('click', copyLink);
          link.remove();
        });

        return {
          id,
          text: headingText,
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
      tableRegions.push(region);
    }

    return () => {
      for (const cleanup of permalinkCleanups) {
        cleanup();
      }
      for (const { heading, row } of headingRows) {
        row.before(heading);
        row.remove();
      }
      for (const region of tableRegions) {
        region.before(...region.childNodes);
        region.remove();
      }
    };
  };

  const indexLabel = $derived(
    locale === 'de' ? 'Abschnitte auf dieser Seite' : 'Sections on this page',
  );
</script>

<article class="manual-article">
  <header>
    <p class="manual-article__eyebrow" data-pagefind-meta="section">
      {sectionLabel(locale, section)}
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

  .manual-article__body :global(.manual-heading-permalink) {
    display: inline-flex;
    min-width: 2.75rem;
    min-height: 2.75rem;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    margin-left: var(--s-2);
    border: 1px solid transparent;
    border-radius: var(--r-md);
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 1rem;
    text-decoration: none;
    vertical-align: middle;
  }

  .manual-article__body :global(.manual-heading-row) {
    display: flex;
    align-items: center;
  }

  .manual-article__body :global(.manual-heading-row > h2),
  .manual-article__body :global(.manual-heading-row > h3) {
    min-width: 0;
  }

  .manual-article__body :global(.manual-heading-permalink:hover) {
    border-color: var(--line);
    color: var(--gem-bright);
  }

  .manual-article__body :global(.manual-heading-permalink__status) {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
    border: 0;
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

  .manual-article__body :global(.manual-search-trigger) {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    margin: var(--s-4) 0;
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    background: linear-gradient(135deg, rgb(61 91 255 / 14%), rgb(123 92 255 / 9%));
    color: var(--gem-bright);
    font-family: var(--font-sans);
    font-weight: 650;
    cursor: pointer;
  }

  .manual-article__body :global(th),
  .manual-article__body :global(td) {
    padding: var(--s-3);
    border: 1px solid var(--line);
    text-align: left;
  }
</style>
