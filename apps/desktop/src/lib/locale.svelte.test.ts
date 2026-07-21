import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  locale: vi.fn<() => Promise<string | null>>(),
  getSettings: vi.fn<() => Promise<Record<string, string>>>(),
}));

vi.mock('@tauri-apps/plugin-os', () => ({ locale: mocks.locale }));
vi.mock('./commands', () => ({ getSettings: mocks.getSettings }));

import { currentLocale, initLocale, setUiLocalePreference } from './locale.svelte';

describe('locale preferences', () => {
  beforeEach(() => {
    mocks.locale.mockClear();
    mocks.getSettings.mockClear();
    mocks.locale.mockResolvedValue('en-US');
    mocks.getSettings.mockResolvedValue({});
  });

  it('uses English when the OS locale is unsupported', async () => {
    mocks.locale.mockResolvedValue('pt-BR');

    await initLocale();

    expect(currentLocale()).toBe('en');
  });

  it('keeps the detected locale when the saved preference is automatic', async () => {
    mocks.locale.mockResolvedValue('es-MX');
    mocks.getSettings.mockResolvedValue({ ui_locale: 'auto' });

    await initLocale();

    expect(currentLocale()).toBe('es');
  });

  it('applies a valid saved locale over the OS locale', async () => {
    mocks.locale.mockResolvedValue('de-DE');
    mocks.getSettings.mockResolvedValue({ ui_locale: 'fr' });

    await initLocale();

    expect(currentLocale()).toBe('fr');
  });

  it('does not let a stale settings load override a newer preference', async () => {
    let resolveSettings: ((settings: Record<string, string>) => void) | undefined;
    mocks.getSettings.mockImplementation(
      () => new Promise<Record<string, string>>((resolve) => { resolveSettings = resolve; }),
    );

    const initialization = initLocale();
    await vi.waitFor(() => expect(mocks.getSettings).toHaveBeenCalledOnce());
    setUiLocalePreference('de');
    if (!resolveSettings) {
      throw new Error('Expected settings request to be pending');
    }
    resolveSettings({ ui_locale: 'fr' });
    await initialization;

    expect(currentLocale()).toBe('de');
  });

  it('changes the application locale for an explicit preference', () => {
    setUiLocalePreference('de');

    expect(currentLocale()).toBe('de');
  });
});
