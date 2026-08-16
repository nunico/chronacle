import { browserPreferredLocale, isManualRoutePair, manualBase, routeLocale } from './locale';

describe('locale rules', () => {
  it('uses German only when a German browser preference is present first', () => {
    expect(browserPreferredLocale(['de-DE', 'en-US'])).toBe('de');
    expect(browserPreferredLocale(['fr-FR', 'en-US'])).toBe('en');
  });

  it('maps explicit manual roots', () => {
    expect(manualBase('en')).toBe('/en/manual');
    expect(manualBase('de')).toBe('/de/handbuch');
    expect(isManualRoutePair('en', 'manual')).toBe(true);
    expect(isManualRoutePair('en', 'handbuch')).toBe(false);
  });

  it('derives the output language from the route', () => {
    expect(routeLocale('/de/handbuch/codex/ueberblick')).toBe('de');
    expect(routeLocale('/')).toBe('en');
  });
});
