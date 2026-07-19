import { locale } from '@tauri-apps/plugin-os';
import { untrack } from 'svelte';
import { SvelteDate } from 'svelte/reactivity';
import { getSettings } from './commands';
import { createI18n, normalizeLocale, type SupportedLocale } from './i18n/index.svelte';

export type UiLocalePreference = 'auto' | SupportedLocale;

function navigatorLocale(): string {
  return typeof navigator === 'undefined' ? 'en' : navigator.language;
}

function normalizePreference(value: string | null | undefined): UiLocalePreference {
  return value === 'auto' || value === 'en' || value === 'de' || value === 'fr' || value === 'es'
    ? value
    : 'auto';
}

let detectedLocale = $state<SupportedLocale>(normalizeLocale(navigatorLocale()));
let preference = $state<UiLocalePreference>('auto');
let localeVersion = 0;

export const i18n = createI18n(untrack(() => detectedLocale));
const _dateFormatter = $derived(new Intl.DateTimeFormat(i18n.locale));

function applyLocale(): void {
  i18n.setLocale(preference === 'auto' ? detectedLocale : preference);
}

export function uiLocalePreference(): UiLocalePreference {
  return preference;
}

export function setUiLocalePreference(value: string | null | undefined): void {
  localeVersion += 1;
  preference = normalizePreference(value);
  applyLocale();
}

export async function initLocale(): Promise<void> {
  const requestVersion = ++localeVersion;

  try {
    const osLocale = await locale();
    if (osLocale) detectedLocale = normalizeLocale(osLocale);
  } catch {
    // Keep the navigator.language fallback when Tauri IPC is unavailable.
  }

  applyLocale();

  try {
    const settings = await getSettings();
    if (requestVersion === localeVersion) setUiLocalePreference(settings['ui_locale']);
  } catch {
    // Settings are unavailable during startup failures and browser-based tests.
  }
}

export function formatDate(dateStr: string): string {
  if (!dateStr) return '';
  // Treat YYYY-MM-DD as local noon to avoid UTC midnight off-by-one in western timezones
  const d = new SvelteDate(dateStr.includes('T') ? dateStr : dateStr + 'T12:00:00');
  if (isNaN(d.getTime())) return dateStr;
  return _dateFormatter.format(d);
}

export function formatNumber(n: number, opts?: Intl.NumberFormatOptions): string {
  return new Intl.NumberFormat(i18n.locale, opts).format(n);
}

export function currentLocale(): string {
  return i18n.locale;
}
