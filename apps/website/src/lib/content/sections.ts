import type { ManualSectionId } from './types';
import type { Locale } from '$lib/i18n/types';

export const manualSections = [
  'overview',
  'getting-started',
  'ai-providers',
  'source-library',
  'campaigns',
  'codex',
  'notes-and-sessions',
  'vault',
  'settings',
  'troubleshooting',
  'glossary',
] as const satisfies readonly ManualSectionId[];

const labels: Record<Locale, Record<ManualSectionId, string>> = {
  en: {
    overview: 'Overview',
    'getting-started': 'Getting started',
    'ai-providers': 'AI providers',
    'source-library': 'Source library',
    campaigns: 'Campaigns',
    codex: 'Codex',
    'notes-and-sessions': 'Notes and sessions',
    vault: 'Vault',
    settings: 'Settings',
    troubleshooting: 'Troubleshooting',
    glossary: 'Glossary',
  },
  de: {
    overview: 'Überblick',
    'getting-started': 'Erste Schritte',
    'ai-providers': 'KI-Anbieter',
    'source-library': 'Quellenbibliothek',
    campaigns: 'Kampagnen',
    codex: 'Kodex',
    'notes-and-sessions': 'Notizen und Spielrunden',
    vault: 'Vault',
    settings: 'Einstellungen',
    troubleshooting: 'Fehlerbehebung',
    glossary: 'Glossar',
  },
};

export function sectionLabel(locale: Locale, section: ManualSectionId): string {
  return labels[locale][section];
}
