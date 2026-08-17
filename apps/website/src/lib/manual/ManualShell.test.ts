import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { getArticle } from '$lib/content/registry';
import type { ManualArticle } from '$lib/content/types';
import ManualShell from './ManualShell.svelte';

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
    expect(screen.getByRole('link', { name: /deutsch/i })).toHaveAttribute(
      'href',
      '/de/handbuch',
    );
  });

  it('builds an h2/h3 outline without including h1 or h4 headings', async () => {
    render(ManualShell, { article: getArticle('en', 'overview') });

    const outline = await screen.findByRole('navigation', { name: 'On this page' });
    expect(within(outline).getByRole('link', { name: 'What you will find' })).toBeInTheDocument();
    expect(within(outline).getByRole('link', { name: 'At the table' })).toBeInTheDocument();
    expect(within(outline).queryByRole('link', { name: 'Chronacle Manual' })).not.toBeInTheDocument();
    expect(within(outline).queryByRole('link', { name: 'A small detail' })).not.toBeInTheDocument();
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
});
