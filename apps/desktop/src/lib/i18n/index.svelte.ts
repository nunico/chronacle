import de from './locales/de';
import en from './locales/en';
import es from './locales/es';
import fr from './locales/fr';
import type { MessageCatalog, MessageKey } from './messages';
import { supportedLocales, type MessageParameters, type SupportedLocale } from './types';

export { supportedLocales };
export type { MessageCatalog, MessageKey, MessageParameters, SupportedLocale };

export const localeCatalogs = { en, de, fr, es } satisfies Record<SupportedLocale, MessageCatalog>;

export function normalizeLocale(value: string | null | undefined): SupportedLocale {
  const language = value?.trim().replace('_', '-').split('-')[0]?.toLowerCase();
  return supportedLocales.includes(language as SupportedLocale)
    ? (language as SupportedLocale)
    : 'en';
}

function messageAt(catalog: MessageCatalog, key: MessageKey): string | undefined {
  let value: unknown = catalog;
  for (const segment of key.split('.')) {
    if (!value || typeof value !== 'object') return undefined;
    value = (value as Record<string, unknown>)[segment];
  }
  return typeof value === 'string' ? value : undefined;
}

function messageKeys(catalog: object, prefix = ''): MessageKey[] {
  return Object.entries(catalog).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof value === 'string'
      ? [path as MessageKey]
      : value && typeof value === 'object'
        ? messageKeys(value, path)
        : [];
  });
}

function interpolate(message: string, parameters: MessageParameters): string {
  return message.replace(/\{([^}]+)\}/g, (placeholder, name: string) =>
    name in parameters ? String(parameters[name]) : placeholder,
  );
}

export interface I18n {
  readonly locale: SupportedLocale;
  setLocale(locale: string | null | undefined): void;
  t(key: MessageKey, parameters?: MessageParameters): string;
  missingKeys(): MessageKey[];
}

export function createI18n(initialLocale: string | null | undefined): I18n {
  let currentLocale = $state(normalizeLocale(initialLocale));
  return {
    get locale() {
      return currentLocale;
    },
    setLocale(locale) {
      currentLocale = normalizeLocale(locale);
    },
    t(key, parameters = {}) {
      return interpolate(
        messageAt(localeCatalogs[currentLocale], key) ?? messageAt(en, key) ?? key,
        parameters,
      );
    },
    missingKeys() {
      return messageKeys(en).filter((key) => !messageAt(localeCatalogs[currentLocale], key));
    },
  };
}
