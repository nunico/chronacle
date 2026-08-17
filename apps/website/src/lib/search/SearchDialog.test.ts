import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import SearchDialog, { sanitizeExcerptHtml } from './SearchDialog.svelte';
import type { ManualSearch, SearchResult } from './types';

const results: SearchResult[] = [
  {
    url: '/en/manual/getting-started#install',
    title: 'Install Chronacle',
    section: 'Getting started',
    excerptHtml: 'Open the <mark data-source="bad">installer</mark>.',
  },
  {
    url: '/en/manual/providers',
    title: 'Choose an AI provider',
    section: 'AI providers',
    excerptHtml: 'Connect a <mark>provider</mark>.',
  },
];

function searchFixture(overrides: Partial<ManualSearch> = {}): ManualSearch {
  return {
    init: vi.fn().mockResolvedValue(undefined),
    search: vi.fn().mockResolvedValue(results),
    ...overrides,
  };
}

beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function showModal() {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function close() {
    this.removeAttribute('open');
    this.dispatchEvent(new Event('close'));
  };
});

afterEach(() => {
  vi.useRealTimers();
});

describe('SearchDialog', () => {
  it('opens with localized empty-query suggestions and help', async () => {
    const { component } = render(SearchDialog, { locale: 'en', search: searchFixture() });

    component.openSearch();

    const dialog = await screen.findByRole('dialog', { name: 'Search the manual' });
    expect(within(dialog).getByRole('combobox', { name: 'Search the manual' })).toHaveFocus();
    expect(within(dialog).getByText('Try a topic')).toBeInTheDocument();
    expect(within(dialog).getByRole('link', { name: 'Manual overview' })).toHaveAttribute(
      'href',
      '/en/manual',
    );
    expect(within(dialog).getByRole('link', { name: 'Getting started' })).toBeInTheDocument();
  });

  it('debounces queries and renders safe result details', async () => {
    vi.useFakeTimers();
    const search = searchFixture();
    const { component } = render(SearchDialog, { locale: 'en', search });
    component.openSearch();
    await vi.runAllTicks();

    const input = screen.getByRole('combobox', { name: 'Search the manual' });
    await fireEvent.input(input, { target: { value: 'install' } });
    await vi.advanceTimersByTimeAsync(199);
    expect(search.search).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    await vi.runAllTicks();

    expect(search.search).toHaveBeenCalledWith('install');
    const firstResult = screen.getByRole('option', { name: /install chronacle/i });
    expect(firstResult).toBeInTheDocument();
    expect(screen.getByText('Getting started')).toBeInTheDocument();
    const excerpt = firstResult.querySelector('.search-dialog__excerpt');
    expect(excerpt).not.toBeNull();
    if (!excerpt) {
      throw new Error('Expected a search result excerpt');
    }
    expect(excerpt.querySelector('mark')).toHaveTextContent('installer');
    expect(excerpt.querySelector('mark')).not.toHaveAttribute('data-source');
    expect(excerpt.querySelector('img')).toBeNull();
  });

  it('discards an in-flight response after the query is cleared', async () => {
    vi.useFakeTimers();
    let resolveSearch: ((value: SearchResult[]) => void) | undefined;
    const pending = new Promise<SearchResult[]>((resolve) => {
      resolveSearch = resolve;
    });
    const search = searchFixture({ search: vi.fn().mockReturnValue(pending) });
    const { component } = render(SearchDialog, { locale: 'en', search });
    component.openSearch();
    await vi.runAllTicks();
    const input = screen.getByRole('combobox', { name: 'Search the manual' });
    await fireEvent.input(input, { target: { value: 'install' } });
    await vi.advanceTimersByTimeAsync(200);
    await fireEvent.input(input, { target: { value: '' } });
    resolveSearch?.(results);
    await vi.runAllTicks();
    await Promise.resolve();
    await tick();

    expect(input).toHaveAttribute('aria-expanded', 'false');
    expect(input).not.toHaveAttribute('aria-controls');
    expect(screen.queryByRole('option')).not.toBeInTheDocument();
    expect(screen.getByText('Try a topic')).toBeInTheDocument();
  });

  it('falls back to plain text for unexpected excerpt elements', () => {
    expect(sanitizeExcerptHtml('Read <mark>this</mark><img src=x onerror=alert(1)>')).toBe(
      'Read this',
    );
    expect(sanitizeExcerptHtml('<mark class="foreign">safe words</mark>')).toBe(
      '<mark>safe words</mark>',
    );
  });

  it('moves the active result with arrow keys and selects it with Enter', async () => {
    vi.useFakeTimers();
    const onNavigate = vi.fn();
    const { component } = render(SearchDialog, {
      locale: 'en',
      search: searchFixture(),
      onNavigate,
    });
    component.openSearch();
    await vi.runAllTicks();
    const input = screen.getByRole('combobox', { name: 'Search the manual' });
    await fireEvent.input(input, { target: { value: 'provider' } });
    await vi.advanceTimersByTimeAsync(200);

    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(screen.getByRole('option', { name: /choose an ai provider/i })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(screen.getByRole('option', { name: /install chronacle/i })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await fireEvent.keyDown(input, { key: 'Enter' });

    expect(onNavigate).toHaveBeenCalledWith('/en/manual/getting-started#install');
  });

  it('announces the localized result count', async () => {
    vi.useFakeTimers();
    const { component } = render(SearchDialog, { locale: 'de', search: searchFixture() });
    component.openSearch();
    await vi.runAllTicks();
    await fireEvent.input(screen.getByRole('combobox', { name: 'Handbuch durchsuchen' }), {
      target: { value: 'Chronacle' },
    });
    await vi.advanceTimersByTimeAsync(200);

    expect(screen.getByRole('status')).toHaveTextContent('2 Ergebnisse');
  });

  it('shows localized no-results help with direct manual links', async () => {
    vi.useFakeTimers();
    const search = searchFixture({ search: vi.fn().mockResolvedValue([]) });
    const { component } = render(SearchDialog, { locale: 'de', search });
    component.openSearch();
    await vi.runAllTicks();
    await fireEvent.input(screen.getByRole('combobox', { name: 'Handbuch durchsuchen' }), {
      target: { value: 'unbekannt' },
    });
    await vi.advanceTimersByTimeAsync(200);

    expect(screen.getByText('Keine Ergebnisse gefunden.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Handbuchüberblick' })).toHaveAttribute(
      'href',
      '/de/handbuch',
    );
    expect(screen.getByRole('link', { name: 'Am Spieltisch' })).toHaveAttribute(
      'href',
      '/de/handbuch/#am-spieltisch',
    );
  });

  it('shows a localized load failure while retaining direct links', async () => {
    const search = searchFixture({ init: vi.fn().mockRejectedValue(new Error('missing bundle')) });
    const { component } = render(SearchDialog, { locale: 'en', search });
    component.openSearch();

    expect(
      await screen.findByText('Search is unavailable right now. You can still browse the manual.'),
    ).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Manual overview' })).toBeInTheDocument();
  });

  it('closes on Escape and restores focus to its opener', async () => {
    const user = userEvent.setup();
    const opener = document.createElement('button');
    opener.textContent = 'Open search';
    document.body.append(opener);
    opener.focus();
    const { component } = render(SearchDialog, { locale: 'en', search: searchFixture() });
    component.openSearch(opener);

    const dialog = await screen.findByRole('dialog', { name: 'Search the manual' });
    await fireEvent.keyDown(dialog, { key: 'Escape' });

    expect(dialog).not.toHaveAttribute('open');
    expect(opener).toHaveFocus();
    opener.remove();
    await user.keyboard('{Escape}');
  });

  it('opens from a declarative trigger and the global shortcut outside editable fields', async () => {
    const trigger = document.createElement('button');
    trigger.dataset.manualSearch = '';
    trigger.textContent = 'Find guidance';
    document.body.append(trigger);
    const editable = document.createElement('textarea');
    document.body.append(editable);
    render(SearchDialog, { locale: 'en', search: searchFixture() });

    await fireEvent.click(trigger);
    const dialog = screen.getByRole('dialog', { name: 'Search the manual' });
    expect(dialog).toHaveAttribute('open');
    await fireEvent.keyDown(dialog, { key: 'Escape' });

    editable.focus();
    await fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    expect(dialog).not.toHaveAttribute('open');

    trigger.focus();
    await fireEvent.keyDown(window, { key: 'k', metaKey: true });
    expect(dialog).toHaveAttribute('open');
    trigger.remove();
    editable.remove();
  });
});
