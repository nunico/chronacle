import { locale } from '@tauri-apps/plugin-os';
import { SvelteDate } from 'svelte/reactivity';

let _locale = $state<string>(navigator.language);
const _dateFormatter = $derived(new Intl.DateTimeFormat(_locale));

export async function initLocale(): Promise<void> {
  try {
    const osLocale = await locale();
    if (osLocale) _locale = osLocale;
  } catch {
    // Keep navigator.language fallback — Tauri IPC unavailable (e.g. in tests)
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
  return new Intl.NumberFormat(_locale, opts).format(n);
}

export function currentLocale(): string {
  return _locale;
}
