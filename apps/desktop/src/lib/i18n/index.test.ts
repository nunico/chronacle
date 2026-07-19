import { describe, expect, it } from 'vitest';

import { createI18n, localeCatalogs, normalizeLocale, supportedLocales } from './index.svelte';

describe('i18n', () => {
  const i18n = createI18n('en');

  // @ts-expect-error `progress.source` requires the exact `current` and `total` parameters.
  i18n.t('progress.source', { curent: 1, total: 3 });

  // @ts-expect-error messages with placeholders require their parameters.
  i18n.t('progress.source');

  it('normalizes supported locale variants and defaults unsupported values to English', () => {
    expect(normalizeLocale('de-DE')).toBe('de');
    expect(normalizeLocale('fr-CA')).toBe('fr');
    expect(normalizeLocale('es-MX')).toBe('es');
    expect(normalizeLocale('it-IT')).toBe('en');
  });

  it('switches the reactive locale used for progress messages', () => {
    const i18n = createI18n('en');

    expect(i18n.locale).toBe('en');
    expect(i18n.t('progress.source', { current: 1, total: 3 })).toBe('Source 1/3');

    i18n.setLocale('de-DE');

    expect(i18n.locale).toBe('de');
    expect(i18n.t('progress.source', { current: 1, total: 3 })).toBe('Quelle 1/3');
  });

  it('has no missing messages in any shipped locale catalog', () => {
    for (const locale of supportedLocales) {
      expect(createI18n(locale).missingKeys()).toEqual([]);
    }
  });

  it('interpolates named placeholders and falls back to English when a message is absent', () => {
    const germanCatalog = localeCatalogs.de as unknown as {
      progress: { source?: string };
    };
    const germanSource = germanCatalog.progress.source;

    try {
      germanCatalog.progress.source = undefined;

      expect(createI18n('de').t('progress.source', { current: 2, total: 4 })).toBe('Source 2/4');
    } finally {
      germanCatalog.progress.source = germanSource;
    }
  });

  it('leaves prototype property placeholders unresolved when no parameter is provided', () => {
    const germanCatalog = localeCatalogs.de as unknown as {
      progress: { source?: string };
    };
    const germanSource = germanCatalog.progress.source;

    try {
      germanCatalog.progress.source = '{toString}';

      expect(createI18n('de').t('progress.source', { current: 1, total: 3 })).toBe('{toString}');
    } finally {
      germanCatalog.progress.source = germanSource;
    }
  });
});
