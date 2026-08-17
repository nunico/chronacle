import type { Component } from 'svelte';
import { getArticle, getTranslation, manualEntries, validateArticles } from './registry';
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

  it('loads the canonical paired common-problems troubleshooting routes', () => {
    const english = getArticle('en', 'troubleshooting/common-problems');
    const german = getArticle('de', 'fehlerbehebung/haeufige-probleme');

    expect(english).toMatchObject({
      translationKey: 'troubleshooting.common',
      section: 'troubleshooting',
      order: 1,
      href: '/en/manual/troubleshooting/common-problems',
    });
    expect(german).toMatchObject({
      translationKey: 'troubleshooting.common',
      section: 'troubleshooting',
      order: 1,
      href: '/de/handbuch/fehlerbehebung/haeufige-probleme',
    });
    expect(english.headings).toHaveLength(2);
    expect(german.headings).toHaveLength(2);
    expect(getTranslation('en', english.slug).href).toBe(german.href);
    expect(manualEntries()).toEqual(
      expect.arrayContaining([
        {
          locale: 'en',
          manual: 'manual',
          slug: 'troubleshooting/common-problems',
        },
        {
          locale: 'de',
          manual: 'handbuch',
          slug: 'fehlerbehebung/haeufige-probleme',
        },
      ]),
    );
  });

  it.each([
    ['getting-started.quick-start', 'getting-started/quick-start', 'erste-schritte/schnellstart'],
    ['getting-started.install', 'getting-started/install', 'erste-schritte/installieren'],
    [
      'getting-started.first-answer',
      'getting-started/first-answer',
      'erste-schritte/erste-antwort',
    ],
    ['providers.choose', 'ai-providers/choose', 'ki-anbieter/auswahl'],
    ['providers.online', 'ai-providers/online', 'ki-anbieter/online'],
    ['providers.local', 'ai-providers/local', 'ki-anbieter/lokal'],
    ['providers.custom', 'ai-providers/custom', 'ki-anbieter/eigene-anbieter'],
    [
      'providers.language-search',
      'ai-providers/language-and-search',
      'ki-anbieter/sprache-und-suche',
    ],
    ['sources.overview', 'source-library/overview', 'quellenbibliothek/ueberblick'],
    ['sources.collections', 'source-library/collections', 'quellenbibliothek/sammlungen'],
    ['sources.upload', 'source-library/upload-pdfs', 'quellenbibliothek/pdfs-importieren'],
    ['sources.ingestion', 'source-library/indexing', 'quellenbibliothek/indizierung'],
    ['campaigns.overview', 'campaigns/overview', 'kampagnen/ueberblick'],
    ['campaigns.manage', 'campaigns/manage', 'kampagnen/verwalten'],
    ['campaigns.sources', 'campaigns/source-access', 'kampagnen/quellenzugriff'],
    ['codex.overview', 'codex/overview', 'kodex/ueberblick'],
    ['codex.compile', 'codex/compile', 'kodex/kompilieren'],
    ['codex.articles-notes', 'codex/articles-and-notes', 'kodex/artikel-und-notizen'],
    ['codex.rules', 'codex/rule-types', 'kodex/regelarten'],
    ['codex.objections', 'codex/redo-with-objections', 'kodex/mit-einwaenden-neu-erstellen'],
    ['codex.identity', 'codex/names-and-duplicates', 'kodex/namen-und-duplikate'],
    ['codex.health', 'codex/maintenance', 'kodex/wartung'],
    ['notes.notes', 'notes-and-sessions/notes', 'notizen-und-sitzungen/notizen'],
    ['notes.sessions', 'notes-and-sessions/session-log', 'notizen-und-sitzungen/sitzungsprotokoll'],
    ['notes.chat-history', 'notes-and-sessions/chat-history', 'notizen-und-sitzungen/chatverlauf'],
    [
      'questions.ask',
      'notes-and-sessions/asking-questions',
      'notizen-und-sitzungen/fragen-stellen',
    ],
    ['questions.citations', 'notes-and-sessions/citations', 'notizen-und-sitzungen/quellenangaben'],
  ])('loads the canonical %s translation pair', (translationKey, englishSlug, germanSlug) => {
    const english = getArticle('en', englishSlug);
    const german = getArticle('de', germanSlug);

    expect(english.translationKey).toBe(translationKey);
    expect(german.translationKey).toBe(translationKey);
    expect(getTranslation('en', englishSlug).href).toBe(german.href);
    expect(getTranslation('de', germanSlug).href).toBe(english.href);
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
