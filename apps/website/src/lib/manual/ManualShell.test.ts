import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { getArticle } from '$lib/content/registry';
import type { ManualArticle } from '$lib/content/types';
import ManualShell from './ManualShell.svelte';
import ManualArticleLayout from './ManualArticle.svelte';
import ManualFallback from './ManualFallback.svelte';

vi.mock('$lib/content/registry', async (importOriginal) => {
  const registry = await importOriginal<typeof import('$lib/content/registry')>();
  const overview = registry.getArticle('en', 'overview');
  const makeArticle = (values: Partial<ManualArticle>): ManualArticle => ({
    ...overview,
    translationKey: 'manual.fixture',
    slug: 'fixture',
    title: 'Fixture',
    summary: 'Fixture summary',
    section: 'getting-started',
    order: 1,
    href: '/en/manual/fixture',
    ...values,
  });

  return {
    ...registry,
    articlesFor: (locale: 'en' | 'de') =>
      locale === 'en'
        ? [
            makeArticle({
              translationKey: 'manual.before',
              slug: 'before',
              title: 'Before overview',
              href: '/en/manual/before',
              section: 'overview',
              order: 0,
            }),
            overview,
            makeArticle({
              translationKey: 'manual.after',
              slug: 'after',
              title: 'After overview',
              href: '/en/manual/after',
              section: 'ai-providers',
              order: 1,
            }),
          ]
        : registry.articlesFor(locale),
  };
});

beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function showModal() {
    this.setAttribute('open', '');
  };
  HTMLDialogElement.prototype.close = function close() {
    this.removeAttribute('open');
    this.dispatchEvent(new Event('close'));
  };
});

describe('ManualShell', () => {
  it('renders localized reference navigation and registry-ordered article links', async () => {
    render(ManualShell, { article: getArticle('en', 'overview') });

    const navigation = screen.getByRole('navigation', { name: 'Manual navigation' });
    expect(within(navigation).getByRole('link', { name: 'Chronacle Manual' })).toHaveAttribute(
      'aria-current',
      'page',
    );

    const breadcrumbs = screen.getByRole('navigation', { name: 'Breadcrumbs' });
    expect(within(breadcrumbs).getByText('Overview')).toBeInTheDocument();
    expect(within(breadcrumbs).getByText('Chronacle Manual')).toBeInTheDocument();

    expect(screen.getByRole('link', { name: /previous.*before overview/i })).toHaveAttribute(
      'href',
      '/en/manual/before',
    );
    expect(screen.getByRole('link', { name: /next.*after overview/i })).toHaveAttribute(
      'href',
      '/en/manual/after',
    );
    expect(screen.getByRole('link', { name: /deutsch/i })).toHaveAttribute('href', '/de/handbuch');
    expect(screen.getByRole('link', { name: /deutsch/i })).toHaveAttribute('data-sveltekit-reload');
  });

  it('prerenders a complete no-JavaScript manual navigation fallback', () => {
    const { container } = render(ManualShell, { article: getArticle('en', 'overview') });

    expect(container.querySelector('noscript')).not.toBeNull();

    const fallback = render(ManualFallback, {
      locale: 'en',
      currentSlug: 'overview',
    }).container;
    expect(within(fallback).getByRole('navigation')).toHaveAttribute(
      'aria-label',
      'Manual navigation without JavaScript',
    );
    expect(fallback.querySelector('a[aria-current="page"]')).toHaveAttribute('href', '/en/manual');
    expect(fallback.querySelectorAll('a')).toHaveLength(3);
  });

  it('builds an h2/h3 outline without including h1 or h4 headings', async () => {
    render(ManualShell, { article: getArticle('en', 'overview') });

    const outline = await screen.findByRole('navigation', { name: 'On this page' });
    expect(within(outline).getByRole('link', { name: 'What you will find' })).toBeInTheDocument();
    expect(within(outline).getByRole('link', { name: 'At the table' })).toBeInTheDocument();
    expect(
      within(outline).queryByRole('link', { name: 'Chronacle Manual' }),
    ).not.toBeInTheDocument();
    expect(within(outline).queryByRole('link', { name: 'A small detail' })).not.toBeInTheDocument();
  });

  it('marks only the title, summary, and article body for Pagefind', () => {
    const { container } = render(ManualShell, { article: getArticle('en', 'overview') });

    const indexed = Array.from(container.querySelectorAll('[data-pagefind-body]'));
    expect(indexed).toHaveLength(3);
    expect(indexed[0]).toBe(screen.getByRole('heading', { level: 1, name: 'Chronacle Manual' }));
    expect(indexed[1]).toHaveTextContent('Learn what this manual covers');
    expect(indexed[2]).toHaveClass('manual-article__body');
    expect(container.querySelector('.manual-article__eyebrow')).not.toHaveAttribute(
      'data-pagefind-body',
    );
  });

  it('rewrites colliding discovered heading IDs to their unique TOC targets', () => {
    const onheadings = vi.fn();
    const children = createRawSnippet(() => ({
      render: () => '<div><h2 id="duplicate">First</h2><h3 id="duplicate">Second</h3></div>',
    }));
    const { container } = render(ManualArticleLayout, {
      children,
      title: 'Collision fixture',
      summary: 'Heading collision fixture',
      locale: 'en',
      section: 'overview',
      onheadings,
    });

    expect(container.querySelectorAll('h2, h3')[0]).toHaveAttribute('id', 'duplicate');
    expect(container.querySelectorAll('h2, h3')[1]).toHaveAttribute('id', 'duplicate-2');
    expect(onheadings).toHaveBeenCalledWith([
      { id: 'duplicate', text: 'First', level: 2 },
      { id: 'duplicate-2', text: 'Second', level: 3 },
    ]);
  });

  it('links the German manual back to the single landing route', () => {
    render(ManualShell, { article: getArticle('de', 'ueberblick') });

    expect(screen.getByRole('link', { name: 'Zur Chronacle-Startseite' })).toHaveAttribute(
      'href',
      '/',
    );
    expect(screen.getByRole('link', { name: 'Startseite' })).toHaveAttribute('href', '/');
  });

  it('opens and closes the native mobile drawer while containing and restoring focus', async () => {
    const user = userEvent.setup();
    render(ManualShell, { article: getArticle('en', 'overview') });

    const trigger = screen.getByRole('button', { name: 'Open manual navigation' });
    await user.click(trigger);

    const drawer = screen.getByRole('dialog', { name: 'Manual navigation' });
    const close = within(drawer).getByRole('button', { name: 'Close manual navigation' });
    expect(drawer).toHaveAttribute('open');
    expect(close).toHaveFocus();

    await user.tab({ shift: true });
    expect(within(drawer).getAllByRole('link').at(-1)).toHaveFocus();
    await user.tab();
    expect(close).toHaveFocus();

    await user.click(close);
    expect(trigger).toHaveFocus();

    await user.click(trigger);
    await fireEvent.keyDown(drawer, { key: 'Escape' });
    expect(drawer).not.toHaveAttribute('open');
    expect(trigger).toHaveFocus();
  });

  it('opens search from both manual header and overview triggers', async () => {
    const user = userEvent.setup();
    render(ManualShell, { article: getArticle('en', 'overview') });

    const triggers = screen.getAllByRole('button', { name: 'Search the manual' });
    expect(triggers).toHaveLength(2);
    await user.click(triggers[0]);
    const dialog = screen.getByRole('dialog', { name: 'Search the manual' });
    expect(dialog).toHaveAttribute('open');
    await fireEvent.keyDown(dialog, { key: 'Escape' });

    await user.click(triggers[1]);
    expect(dialog).toHaveAttribute('open');
  });

  it('excludes manual chrome and exposes localized Pagefind section metadata', () => {
    const { container } = render(ManualShell, { article: getArticle('de', 'ueberblick') });

    expect(container.querySelector('.manual-header')).toHaveAttribute('data-pagefind-ignore');
    expect(container.querySelector('.manual-shell__sidebar')).toHaveAttribute(
      'data-pagefind-ignore',
    );
    expect(container.querySelector('.manual-shell__toc')).toHaveAttribute('data-pagefind-ignore');
    expect(container.querySelector('[data-pagefind-meta="section"]')).toHaveTextContent(
      'Überblick',
    );
  });
});
