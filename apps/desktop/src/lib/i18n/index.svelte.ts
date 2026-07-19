import de from './locales/de';
import en from './locales/en';
import es from './locales/es';
import fr from './locales/fr';
import type {
  MessageCatalog,
  MessageKey,
  MessageParametersFor,
  TranslationArguments,
} from './messages';
import { supportedLocales, type MessageParameters, type SupportedLocale } from './types';

export { supportedLocales };
export type {
  MessageCatalog,
  MessageKey,
  MessageParameters,
  MessageParametersFor,
  SupportedLocale,
  TranslationArguments,
};

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

function interpolate(message: string, parameters: object): string {
  return message.replace(/\{([^}]+)\}/g, (placeholder, name: string) =>
    Object.hasOwn(parameters, name)
      ? String((parameters as Record<string, unknown>)[name])
      : placeholder,
  );
}

export interface I18n {
  readonly locale: SupportedLocale;
  setLocale(locale: string | null | undefined): void;
  t<Key extends MessageKey>(key: Key, ...args: TranslationArguments<Key>): string;
  missingKeys(): MessageKey[];
}

export function createI18n(initialLocale: string | null | undefined): I18n {
  let currentLocale = $state(normalizeLocale(initialLocale));

  function t<Key extends MessageKey>(key: Key, ...args: TranslationArguments<Key>): string {
    const parameters = args[0] ?? {};

    return interpolate(
      messageAt(localeCatalogs[currentLocale], key) ?? messageAt(en, key) ?? key,
      parameters,
    );
  }

  return {
    get locale() {
      return currentLocale;
    },
    setLocale(locale) {
      currentLocale = normalizeLocale(locale);
    },
    t,
    missingKeys() {
      return messageKeys(en).filter((key) => !messageAt(localeCatalogs[currentLocale], key));
    },
  };
}
