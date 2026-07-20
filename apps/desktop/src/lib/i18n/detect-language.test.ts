import { describe, expect, it } from 'vitest';
import { detectSupportedLanguage, resolveResponseLanguage } from './detect-language';

describe('resolveResponseLanguage', () => {
  it.each([
    ['Quelle est la règle ?', 'de', 'fr'],
    ['Wie funktioniert Grappling?', 'fr', 'de'],
    ['grapple?', 'es', 'es'],
    ['How does cover work?', 'de', 'en'],
  ] as const)(
    'uses the message language when detected, otherwise the fallback',
    (message, fallback, expected) => {
      expect(resolveResponseLanguage(message, fallback)).toBe(expected);
    },
  );

  it('rejects very short and ambiguous input', () => {
    expect(detectSupportedLanguage('oui')).toBeNull();
    expect(detectSupportedLanguage('regel')).toBeNull();
  });
});
