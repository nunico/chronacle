<script module lang="ts">
  interface ExcerptPart {
    text: string;
    highlighted: boolean;
  }

  function escapeHtml(value: string): string {
    const node = document.createElement('span');
    node.textContent = value;
    return node.innerHTML;
  }

  export function sanitizeExcerptParts(value: string): ExcerptPart[] {
    const documentFragment = new DOMParser().parseFromString(value, 'text/html');
    const elements = Array.from(documentFragment.body.querySelectorAll('*'));
    if (elements.some((element) => element.tagName !== 'MARK')) {
      return [{ text: documentFragment.body.textContent ?? '', highlighted: false }];
    }
    return Array.from(documentFragment.body.childNodes).flatMap<ExcerptPart>((node) => {
      if (node instanceof HTMLElement && node.tagName === 'MARK') {
        return [{ text: node.textContent ?? '', highlighted: true }];
      }
      return [{ text: node.textContent ?? '', highlighted: false }];
    });
  }

  export function sanitizeExcerptHtml(value: string): string {
    return sanitizeExcerptParts(value)
      .map((part) =>
        part.highlighted ? `<mark>${escapeHtml(part.text)}</mark>` : escapeHtml(part.text),
      )
      .join('');
  }
</script>

<script lang="ts">
  import { resolve } from '$app/paths';
  import type { Pathname } from '$app/types';
  import Search from 'lucide-svelte/icons/search';
  import X from 'lucide-svelte/icons/x';
  import type { Attachment } from 'svelte/attachments';
  import Icon from '$lib/components/Icon.svelte';
  import type { Locale } from '$lib/i18n/types';
  import { manualBase } from '$lib/i18n/locale';
  import { pagefindSearch } from './pagefind';
  import type { ManualSearch, SearchResult } from './types';

  interface Props {
    locale: Locale;
    search?: ManualSearch;
    onNavigate?: (url: string) => void;
  }

  type SearchStatus = 'idle' | 'loading' | 'ready' | 'error';

  let { locale, search = pagefindSearch, onNavigate }: Props = $props();
  let dialog: HTMLDialogElement | undefined;
  let input: HTMLInputElement | undefined;
  let opener: HTMLElement | undefined;
  let initialized = false;
  let initialization: Promise<void> | undefined;
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  let requestSequence = 0;
  let query = $state('');
  let status = $state<SearchStatus>('idle');
  let results = $state.raw<SearchResult[]>([]);
  let activeIndex = $state(-1);
  const componentId = $props.id();
  const inputId = `${componentId}-input`;
  const listboxId = `${componentId}-results`;
  const labels = $derived(
    locale === 'de'
      ? {
          title: 'Handbuch durchsuchen',
          placeholder: 'Im Handbuch suchen',
          close: 'Suche schließen',
          emptyTitle: 'Thema ausprobieren',
          overview: 'Handbuchüberblick',
          gettingStarted: 'Erste Schritte',
          troubleshooting: 'Fehlerbehebung',
          noResults: 'Keine Ergebnisse gefunden.',
          unavailable:
            'Die Suche ist derzeit nicht verfügbar. Du kannst das Handbuch weiterhin durchsuchen.',
          result: 'Ergebnis',
          results: 'Ergebnisse',
          loading: 'Suche läuft …',
        }
      : {
          title: 'Search the manual',
          placeholder: 'Search the manual',
          close: 'Close search',
          emptyTitle: 'Try a topic',
          overview: 'Manual overview',
          gettingStarted: 'Getting started',
          troubleshooting: 'Troubleshooting',
          noResults: 'No results found.',
          unavailable: 'Search is unavailable right now. You can still browse the manual.',
          result: 'result',
          results: 'results',
          loading: 'Searching …',
        },
  );
  const overviewHref = $derived(manualBase(locale));
  const gettingStartedHref = $derived(
    locale === 'de' ? '/de/handbuch/#inhalt' : '/en/manual/#what-you-will-find',
  );
  const troubleshootingHref = $derived(
    locale === 'de'
      ? '/de/handbuch/fehlerbehebung/haeufige-probleme'
      : '/en/manual/troubleshooting/common-problems',
  );
  const activeResultId = $derived(
    activeIndex >= 0 && results[activeIndex] ? `${componentId}-result-${activeIndex}` : undefined,
  );
  const resultAnnouncement = $derived.by(() => {
    if (!query.trim() || status === 'idle') {
      return '';
    }
    if (status === 'loading') {
      return labels.loading;
    }
    if (status === 'error') {
      return labels.unavailable;
    }
    const count = results.length;
    return `${count} ${count === 1 ? labels.result : labels.results}`;
  });

  const attachDialog: Attachment<HTMLDialogElement> = (node) => {
    dialog = node;
    return () => {
      dialog = undefined;
    };
  };

  const attachInput: Attachment<HTMLInputElement> = (node) => {
    input = node;
    return () => {
      input = undefined;
    };
  };

  function ensureInitialized(): Promise<void> {
    if (initialized) {
      return Promise.resolve();
    }
    initialization ??= search.init().then(() => {
      initialized = true;
    });
    return initialization;
  }

  export function openSearch(nextOpener?: HTMLElement): void {
    opener =
      nextOpener ??
      (document.activeElement instanceof HTMLElement ? document.activeElement : undefined);
    query = '';
    results = [];
    activeIndex = -1;
    status = 'idle';
    if (dialog && !dialog.open) {
      if (typeof dialog.showModal === 'function') {
        dialog.showModal();
      } else {
        dialog.setAttribute('open', '');
      }
    }
    queueMicrotask(() => input?.focus());
    void ensureInitialized().catch(() => {
      status = 'error';
    });
  }

  export function closeSearch(): void {
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    requestSequence += 1;
    if (dialog?.open && typeof dialog.close === 'function') {
      dialog.close();
    } else {
      dialog?.removeAttribute('open');
      handleNativeClose();
    }
  }

  function handleNativeClose(): void {
    queueMicrotask(() => opener?.focus());
  }

  function scheduleSearch(nextQuery: string): void {
    query = nextQuery;
    results = [];
    activeIndex = -1;
    if (debounceTimer) {
      clearTimeout(debounceTimer);
    }
    const sequence = ++requestSequence;
    const trimmed = nextQuery.trim();
    if (!trimmed) {
      status = 'idle';
      return;
    }
    status = 'loading';
    debounceTimer = setTimeout(() => {
      void runSearch(trimmed, sequence);
    }, 200);
  }

  async function runSearch(searchQuery: string, sequence: number): Promise<void> {
    try {
      await ensureInitialized();
      const nextResults = await search.search(searchQuery);
      if (sequence !== requestSequence) {
        return;
      }
      results = nextResults;
      activeIndex = nextResults.length > 0 ? 0 : -1;
      status = 'ready';
    } catch {
      if (sequence === requestSequence) {
        status = 'error';
        results = [];
        activeIndex = -1;
      }
    }
  }

  function navigate(url: string): void {
    closeSearch();
    if (onNavigate) {
      onNavigate(url);
    } else {
      window.location.assign(url);
    }
  }

  function handleInputKeydown(event: KeyboardEvent): void {
    if (results.length === 0) {
      return;
    }
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      activeIndex = (activeIndex + 1) % results.length;
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      activeIndex = (activeIndex - 1 + results.length) % results.length;
    } else if (event.key === 'Enter' && activeIndex >= 0) {
      event.preventDefault();
      const result = results[activeIndex];
      if (result) {
        navigate(result.url);
      }
    }
  }

  function focusableElements(): HTMLElement[] {
    return dialog
      ? Array.from(
          dialog.querySelectorAll<HTMLElement>(
            'input, a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
          ),
        )
      : [];
  }

  function handleDialogKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeSearch();
      return;
    }
    if (event.key !== 'Tab') {
      return;
    }
    const focusable = focusableElements();
    const first = focusable[0];
    const last = focusable.at(-1);
    if (!first || !last) {
      event.preventDefault();
    } else if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function isEditable(target: EventTarget | null): boolean {
    return (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement ||
      (target instanceof HTMLElement && target.isContentEditable)
    );
  }

  function handleGlobalKeydown(event: KeyboardEvent): void {
    if (
      event.key.toLowerCase() === 'k' &&
      (event.metaKey || event.ctrlKey) &&
      !event.altKey &&
      !event.shiftKey &&
      !isEditable(event.target) &&
      !isEditable(document.activeElement)
    ) {
      event.preventDefault();
      openSearch(
        document.activeElement instanceof HTMLElement ? document.activeElement : undefined,
      );
    }
  }

  function handleDocumentClick(event: MouseEvent): void {
    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }
    const trigger = target.closest<HTMLElement>('[data-manual-search]');
    if (trigger) {
      event.preventDefault();
      openSearch(trigger);
    }
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />
<svelte:document onclick={handleDocumentClick} />

<dialog
  {@attach attachDialog}
  class="search-dialog"
  aria-labelledby={`${componentId}-title`}
  oncancel={(event) => {
    event.preventDefault();
    closeSearch();
  }}
  onclose={handleNativeClose}
  onkeydown={handleDialogKeydown}
  data-pagefind-ignore
>
  <div class="search-dialog__header">
    <div>
      <p class="search-dialog__eyebrow">{locale === 'de' ? 'Referenzsuche' : 'Reference search'}</p>
      <h2 id={`${componentId}-title`}>{labels.title}</h2>
    </div>
    <button
      type="button"
      class="search-dialog__close"
      aria-label={labels.close}
      onclick={closeSearch}
    >
      <Icon icon={X} size={18} />
    </button>
  </div>

  <div class="search-dialog__field">
    <Icon icon={Search} size={18} />
    <label class="visually-hidden" for={inputId}>{labels.title}</label>
    <input
      {@attach attachInput}
      id={inputId}
      type="search"
      role="combobox"
      placeholder={labels.placeholder}
      value={query}
      aria-controls={results.length > 0 ? listboxId : undefined}
      aria-activedescendant={activeResultId}
      aria-autocomplete="list"
      aria-expanded={results.length > 0}
      autocomplete="off"
      oninput={(event) => scheduleSearch(event.currentTarget.value)}
      onkeydown={handleInputKeydown}
    />
    <kbd>⌘/Ctrl K</kbd>
  </div>

  <p class="visually-hidden" role="status" aria-live="polite">{resultAnnouncement}</p>

  <div class="search-dialog__content">
    {#if status === 'error'}
      <div class="search-dialog__message">
        <p>{labels.unavailable}</p>
        <nav aria-label={locale === 'de' ? 'Handbuchlinks' : 'Manual links'}>
          <a href={resolve(overviewHref)}>{labels.overview}</a>
          <a href={resolve(troubleshootingHref)}>{labels.troubleshooting}</a>
        </nav>
      </div>
    {:else if query.trim() === ''}
      <div class="search-dialog__suggestions">
        <h3>{labels.emptyTitle}</h3>
        <a href={resolve(overviewHref)}>{labels.overview}</a>
        <a href={resolve(gettingStartedHref)}>{labels.gettingStarted}</a>
      </div>
    {:else if status === 'loading'}
      <p class="search-dialog__loading">{labels.loading}</p>
    {:else if status === 'ready' && results.length === 0}
      <div class="search-dialog__message">
        <p>{labels.noResults}</p>
        <nav aria-label={locale === 'de' ? 'Handbuchlinks' : 'Manual links'}>
          <a href={resolve(overviewHref)}>{labels.overview}</a>
          <a href={resolve(troubleshootingHref)}>{labels.troubleshooting}</a>
        </nav>
      </div>
    {:else if results.length > 0}
      <ul id={listboxId} class="search-dialog__results" role="listbox" aria-label={labels.results}>
        {#each results as result, index (result.url)}
          <li>
            <a
              id={`${componentId}-result-${index}`}
              href={resolve(result.url as Pathname)}
              role="option"
              aria-selected={index === activeIndex}
              onclick={(event) => {
                event.preventDefault();
                navigate(result.url);
              }}
              onmouseenter={() => (activeIndex = index)}
            >
              <span class="search-dialog__result-meta">{result.section}</span>
              <strong>{result.title || result.url}</strong>
              {#if result.excerptHtml}
                <span class="search-dialog__excerpt">
                  {#each sanitizeExcerptParts(result.excerptHtml) as part (part)}
                    {#if part.highlighted}<mark>{part.text}</mark>{:else}{part.text}{/if}
                  {/each}
                </span>
              {/if}
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</dialog>

<style>
  .search-dialog {
    width: min(43rem, calc(100vw - 2rem));
    max-width: none;
    max-height: min(44rem, calc(100dvh - 2rem));
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-xl);
    background: rgb(10 12 26 / 96%);
    color: var(--fg-1);
    box-shadow: var(--shadow-3), var(--glow-violet);
  }

  .search-dialog::backdrop {
    background: var(--bg-scrim);
    backdrop-filter: blur(7px);
  }

  .search-dialog__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
    padding: var(--s-5) var(--s-6) var(--s-4);
  }

  .search-dialog__eyebrow {
    margin: 0 0 var(--s-1);
    color: var(--arcane-300);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h2,
  h3,
  p {
    margin: 0;
  }

  h2 {
    font-size: 1.35rem;
  }

  .search-dialog__close {
    display: grid;
    width: 2.5rem;
    height: 2.5rem;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--fg-2);
    cursor: pointer;
  }

  .search-dialog__field {
    display: flex;
    align-items: center;
    gap: var(--s-3);
    margin: 0 var(--s-6);
    padding: 0 var(--s-4);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    background: var(--bg-inset);
    color: var(--fg-3);
  }

  .search-dialog__field:focus-within {
    border-color: var(--arcane-400);
    box-shadow: var(--glow-focus);
  }

  input {
    width: 100%;
    min-width: 0;
    padding: var(--s-4) 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg-1);
    font-family: var(--font-sans);
  }

  kbd {
    flex: 0 0 auto;
    color: var(--fg-3);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
  }

  .search-dialog__content {
    min-height: 11rem;
    max-height: min(29rem, calc(100dvh - 13rem));
    padding: var(--s-5) var(--s-6) var(--s-6);
    overflow-y: auto;
  }

  .search-dialog__suggestions,
  .search-dialog__message {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
    align-items: center;
    color: var(--fg-2);
    font-family: var(--font-sans);
  }

  .search-dialog__suggestions h3,
  .search-dialog__message p {
    width: 100%;
    color: var(--fg-2);
    font-family: var(--font-serif);
    font-size: 1rem;
    font-weight: 400;
  }

  .search-dialog__suggestions a,
  .search-dialog__message a {
    display: inline-flex;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--r-full);
    background: var(--bg-panel);
    color: var(--gem);
    font-size: 0.8125rem;
    text-decoration: none;
  }

  .search-dialog__message nav {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-3);
  }

  .search-dialog__loading {
    color: var(--fg-2);
    font-family: var(--font-serif);
  }

  .search-dialog__results {
    display: grid;
    gap: var(--s-2);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .search-dialog__results a {
    display: grid;
    gap: var(--s-1);
    padding: var(--s-3) var(--s-4);
    border: 1px solid transparent;
    border-radius: var(--r-md);
    color: var(--fg-2);
    text-decoration: none;
  }

  .search-dialog__results a[aria-selected='true'] {
    border-color: var(--line-strong);
    background: linear-gradient(135deg, rgb(61 91 255 / 12%), rgb(123 92 255 / 8%));
  }

  .search-dialog__result-meta {
    color: var(--arcane-300);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    letter-spacing: 0.04em;
  }

  .search-dialog__results strong {
    color: var(--gem-bright);
    font-family: var(--font-sans);
  }

  .search-dialog__excerpt {
    font-family: var(--font-serif);
    font-size: 0.875rem;
  }

  .search-dialog__excerpt :global(mark) {
    border-radius: 2px;
    background: rgb(232 184 106 / 20%);
    color: var(--rune-gold);
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  @media (max-width: 36rem) {
    .search-dialog__header,
    .search-dialog__content {
      padding-inline: var(--s-4);
    }

    .search-dialog__field {
      margin-inline: var(--s-4);
    }

    kbd {
      display: none;
    }
  }

  @media (prefers-reduced-motion: no-preference) {
    .search-dialog {
      animation: search-dialog-enter var(--dur) var(--ease-arcane);
    }
  }

  @keyframes search-dialog-enter {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.99);
    }
  }
</style>
