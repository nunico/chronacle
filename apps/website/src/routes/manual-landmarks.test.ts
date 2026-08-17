import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ManualArticlePage from './[locale=locale]/[manual=manual]/[...slug]/+page.svelte';
import ManualRootPage from './[locale=locale]/[manual=manual]/+page.svelte';

const data = {
  locale: 'en' as const,
  slug: 'overview',
  title: 'Overview',
  summary: 'Manual overview',
};

describe('manual route landmarks', () => {
  it('leaves the main landmark to the universal layout on the manual root', () => {
    render(ManualRootPage, {
      data,
      params: { locale: 'en', manual: 'manual' },
    });

    expect(screen.queryByRole('main')).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Chronacle Manual' })).toBeInTheDocument();
  });

  it('leaves the main landmark to the universal layout on an article route', () => {
    render(ManualArticlePage, {
      data,
      params: { locale: 'en', manual: 'manual', slug: 'overview' },
    });

    expect(screen.queryByRole('main')).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Chronacle Manual' })).toBeInTheDocument();
  });
});
