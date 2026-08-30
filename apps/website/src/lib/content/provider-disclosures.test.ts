import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { landingCopy } from '$lib/i18n/landing-copy';

const manualDisclosurePairs = [
  ['ai-providers/choose.md', 'ki-anbieter/auswahl.md'],
  ['ai-providers/online.md', 'ki-anbieter/online.md'],
  ['ai-providers/language-and-search.md', 'ki-anbieter/sprache-und-suche.md'],
  ['source-library/overview.md', 'quellenbibliothek/ueberblick.md'],
  ['notes-and-sessions/asking-questions.md', 'notizen-und-sitzungen/fragen-stellen.md'],
  ['settings/overview.md', 'einstellungen/ueberblick.md'],
  ['glossary.md', 'glossar.md'],
] as const;

const readManual = (locale: 'en' | 'de', path: string): string =>
  readFileSync(`src/content/manual/${locale}/${path}`, 'utf8');

const answerContextCategories = {
  en: [
    'source excerpts',
    'entity names',
    'summaries',
    'notes',
    'compiled Codex articles',
    'player names and character class, level, and status',
    'event start and end dates',
    'session numbers, titles, played dates, and notes',
    'compiled rules',
  ],
  de: [
    'Quellenauszüge',
    'Namen von Entitäten',
    'Zusammenfassungen',
    'Notizen',
    'kompilierte Codex-Artikel',
    'Spielernamen sowie Klasse, Stufe und Status',
    'Start- und Enddaten von Ereignissen',
    'Sitzungsnummern, -titel, Spieldaten und -notizen',
    'kompilierte Regeln',
  ],
} as const;

describe('remote-provider disclosures', () => {
  it.each(['en', 'de'] as const)(
    'describes answer context and remote embedding separately on the %s landing page',
    (locale) => {
      const disclosure = landingCopy[locale].provider.body;

      expect(disclosure).toMatch(locale === 'en' ? /question/ : /Frage/);
      for (const category of answerContextCategories[locale]) {
        expect(disclosure).toContain(category);
      }
      expect(disclosure).toMatch(
        locale === 'en' ? /not relevance-filtered/ : /nicht (?:als )?relevanzgefilterte?/,
      );
      expect(disclosure).toMatch(locale === 'en' ? /remote embedding/ : /entfernte Einbettung/);
      expect(disclosure).toMatch(
        locale === 'en' ? /question\/search text/ : /Frage- oder Suchtext/,
      );
    },
  );

  it.each(manualDisclosurePairs)(
    'does not reduce answer-provider context to retrieved excerpts in %s or %s',
    (englishPath, germanPath) => {
      const english = readManual('en', englishPath);
      const german = readManual('de', germanPath);

      expect(english).not.toMatch(/question and (?:the )?(?:relevant )?retrieved excerpts/i);
      expect(english).not.toMatch(/question and relevant source excerpts/i);
      expect(english).not.toMatch(/passages and instructions Chronacle supplies/i);
      expect(german).not.toMatch(
        /Frage und (?:die )?(?:dafür )?(?:passende )?(?:gefundenen )?Quellenauszüge/i,
      );
      expect(german).not.toMatch(/Frage und (?:die )?(?:gefundenen )?relevanten Auszüge/i);
      expect(german).not.toMatch(/bereitgestellten Passagen und Anweisungen/i);
      for (const category of answerContextCategories.en) {
        expect(english).toContain(category);
      }
      for (const category of answerContextCategories.de) {
        expect(german).toContain(category);
      }
    },
  );

  it.each(['en', 'de'] as const)(
    'lists every remote answer-context category in the %s settings disclosure',
    (locale) => {
      const path = locale === 'en' ? 'settings/overview.md' : 'einstellungen/ueberblick.md';
      const disclosure = readManual(locale, path);

      for (const category of answerContextCategories[locale]) {
        expect(disclosure).toContain(category);
      }
      expect(disclosure).toMatch(
        locale === 'en'
          ? /full campaign-scoped context/
          : /vollständiger kampagnenbezogener Kontext/,
      );
      expect(disclosure).toMatch(
        locale === 'en' ? /question or search text/ : /Frage- oder Suchtext/,
      );
    },
  );
});
