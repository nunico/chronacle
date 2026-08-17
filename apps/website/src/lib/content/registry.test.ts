import type { Component } from 'svelte';
import { getArticle, getTranslation, validateArticles } from './registry';
import type { ManualArticle, ManualFrontmatter } from './types';

const component = (() => undefined) as unknown as Component;

function article(overrides: Partial<ManualFrontmatter> = {}): ManualArticle {
  const frontmatter: ManualFrontmatter = {
    translationKey: 'manual.overview',
    locale: 'en',
    slug: 'overview',
    title: 'Overview',
    summary: 'Learn what the Chronacle manual covers.',
    section: 'overview',
    order: 1,
    ...overrides,
  };

  return {
    ...frontmatter,
    component,
    href: (frontmatter.slug === 'overview'
      ? `/${frontmatter.locale}/${frontmatter.locale === 'en' ? 'manual' : 'handbuch'}`
      : `/${frontmatter.locale}/${frontmatter.locale === 'en' ? 'manual' : 'handbuch'}/${frontmatter.slug}`) as ManualArticle['href'],
  };
}

const overviewPair = (): ManualArticle[] => [
  article(),
  article({ locale: 'de', slug: 'ueberblick', title: 'Überblick' }),
];

describe('manual content registry', () => {
  it('loads the English overview article', () => {
    expect(getArticle('en', 'overview').translationKey).toBe('manual.overview');
  });

  it('links the English overview to its German translation', () => {
    expect(getTranslation('en', 'overview').href).toBe('/de/handbuch');
  });

  it('rejects duplicate routes within a locale', () => {
    const duplicate = article({ translationKey: 'manual.duplicate' });

    expect(() => validateArticles([...overviewPair(), duplicate])).toThrow(/duplicate route/i);
  });

  it('rejects duplicate slugs within a locale even when routes differ', () => {
    const duplicate = article({
      translationKey: 'manual.duplicate',
      section: 'getting-started',
    });
    duplicate.href = '/en/manual/overview';

    expect(() => validateArticles([...overviewPair(), duplicate])).toThrow(/duplicate slug/i);
  });

  it('rejects unsafe slugs', () => {
    expect(() => validateArticles([article({ slug: '../escape' })])).toThrow(/invalid slug/i);
  });

  it('rejects translation keys without a German counterpart', () => {
    expect(() => validateArticles([article()])).toThrow(/missing de translation/i);
  });

  it('rejects unknown sections', () => {
    const invalidSection = article();
    Object.assign(invalidSection, { section: 'appendix' });

    expect(() => validateArticles([invalidSection, overviewPair()[1]])).toThrow(/unknown section/i);
  });

  it('rejects duplicate heading IDs within an article', () => {
    const invalid = article({
      headings: [
        { id: 'same-heading', text: 'First heading', level: 2 },
        { id: 'same-heading', text: 'Second heading', level: 3 },
      ],
    });

    expect(() => validateArticles([invalid, overviewPair()[1]])).toThrow(/duplicate heading id/i);
  });

  it('rejects unsafe heading IDs', () => {
    const invalid = article({
      headings: [{ id: 'Not a safe ID', text: 'Heading', level: 2 }],
    });

    expect(() => validateArticles([invalid, overviewPair()[1]])).toThrow(/invalid heading id/i);
  });
});
