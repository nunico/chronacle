import type { Locale, ManualSegment } from './types';
import type { Pathname } from '$app/types';

const manualSegments: Record<Locale, ManualSegment> = {
  en: 'manual',
  de: 'handbuch',
};

export function browserPreferredLocale(languages: readonly string[]): Locale {
  return languages[0]?.toLowerCase().startsWith('de') ? 'de' : 'en';
}

export function manualBase(locale: Locale): Pathname {
  return `/${locale}/${manualSegments[locale]}`;
}

export function isManualRoutePair(locale: string, segment: string): boolean {
  return (locale === 'en' || locale === 'de') && manualSegments[locale] === segment;
}

export function routeLocale(pathname: string): Locale {
  return pathname === '/de' || pathname.startsWith('/de/') ? 'de' : 'en';
}
