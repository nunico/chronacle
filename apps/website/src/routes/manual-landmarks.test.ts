import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ManualArticlePage from './[locale=locale]/[manual=manual]/[...slug]/+page.svelte';
import { load as loadManualArticle } from './[locale=locale]/[manual=manual]/[...slug]/+page';
import ManualRootPage from './[locale=locale]/[manual=manual]/+page.svelte';

const data = {
  locale: 'en' as const,
  slug: 'overview',
  title: 'Overview',
  summary: 'Manual overview',
};

describe('manual route landmarks', () => {
  it('provides one main landmark between the manual header and footer on the manual root', () => {
    render(ManualRootPage, {
      data,
      params: { locale: 'en', manual: 'manual' },
    });

    expect(screen.getAllByRole('main')).toHaveLength(1);
    expect(screen.getByRole('main').contains(document.querySelector('.manual-header'))).toBe(false);
    expect(screen.getByRole('contentinfo')).not.toBe(screen.getByRole('main'));
    expect(screen.getByRole('heading', { name: 'Chronacle Manual' })).toBeInTheDocument();
    expect(document.title).toBe('Overview');
    expect(document.querySelector('link[rel="canonical"]')).toHaveAttribute('href', '/en/manual');
    expect(document.querySelector('meta[property="og:title"]')).toHaveAttribute(
      'content',
      'Overview',
    );
    expect(document.querySelector('meta[property="og:description"]')).toHaveAttribute(
      'content',
      'Manual overview',
    );
    expect(document.querySelector('meta[property="og:image"]')).toHaveAttribute(
      'content',
      '/brand/chronacle-icon.png',
    );
    expect(document.head.innerHTML).not.toMatch(/localhost|127\.0\.0\.1/);
  });

  it('provides one main landmark on an article route', () => {
    render(ManualArticlePage, {
      data: {
        locale: 'en',
        slug: 'getting-started/install',
        title: 'Install Chronacle',
        summary: 'Install the current release.',
      },
      params: { locale: 'en', manual: 'manual', slug: 'getting-started/install' },
    });

    expect(screen.getAllByRole('main')).toHaveLength(1);
    expect(screen.getByRole('heading', { name: 'Install Chronacle' })).toBeInTheDocument();
    expect(document.title).toBe('Install Chronacle — Chronacle Manual');
    expect(document.querySelector('link[rel="canonical"]')).toHaveAttribute(
      'href',
      '/en/manual/getting-started/install',
    );
  });

  it('localizes German overview metadata without duplicating the manual name', () => {
    render(ManualRootPage, {
      data: {
        locale: 'de',
        slug: 'ueberblick',
        title: 'Chronacle-Handbuch',
        summary: 'Der Überblick zum Handbuch.',
      },
      params: { locale: 'de', manual: 'handbuch' },
    });

    expect(document.title).toBe('Chronacle-Handbuch');
    expect(document.querySelector('link[rel="canonical"]')).toHaveAttribute('href', '/de/handbuch');
    expect(document.querySelector('meta[property="og:locale"]')).toHaveAttribute(
      'content',
      'de_DE',
    );
  });

  it('loads nested article slugs when the trailing slash is captured by the rest parameter', () => {
    const result = loadManualArticle({
      params: {
        locale: 'de',
        manual: 'handbuch',
        slug: 'fehlerbehebung/haeufige-probleme/',
      },
    } as Parameters<typeof loadManualArticle>[0]);

    expect(result).toMatchObject({
      locale: 'de',
      slug: 'fehlerbehebung/haeufige-probleme',
      title: 'Häufige Probleme',
    });
  });
});
