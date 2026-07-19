import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SettingsView from './SettingsView.svelte';

vi.mock('../lib/commands', () => ({
  getSettings: vi.fn().mockResolvedValue({}),
  updateSetting: vi.fn().mockResolvedValue(undefined),
  getLlmProviderStatus: vi.fn().mockResolvedValue({
    provider_type: 'openai',
    model: 'gpt-4o-mini',
    api_key_configured: true,
  }),
  reconfigureLlmProvider: vi.fn().mockResolvedValue('openai'),
  getEmbeddingProviderStatus: vi.fn().mockResolvedValue({
    backend: 'local',
    model: 'nomic-embed-text-v1.5',
    dimension: 768,
    api_key_configured: false,
    local_available: true,
    local_cached: true,
  }),
  reconfigureEmbeddingProvider: vi.fn().mockResolvedValue('nomic-embed-text-v1.5'),
  getCustomProviders: vi.fn().mockResolvedValue([]),
  createCustomProvider: vi.fn().mockResolvedValue(undefined),
  deleteCustomProvider: vi.fn().mockResolvedValue(undefined),
  getProviderModels: vi.fn().mockResolvedValue([]),
  addProviderModel: vi.fn().mockResolvedValue(undefined),
  removeProviderModel: vi.fn().mockResolvedValue(undefined),
  reindexAllSources: vi.fn().mockResolvedValue(0),
  resyncWikilinks: vi.fn().mockResolvedValue(0),
  getVaultPath: vi.fn().mockResolvedValue(null),
  setVaultPath: vi.fn().mockResolvedValue(undefined),
  vaultSyncNow: vi.fn().mockResolvedValue({
    exported: 0,
    unchanged: 0,
    adopted: 0,
    applied: 0,
    conflicts: 0,
    resolved: 0,
    soft_deleted: 0,
    swept: 0,
    invalid: 0,
    failed: 0,
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import * as commands from '../lib/commands';

describe('SettingsView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(commands.getSettings).mockResolvedValue({});
    vi.mocked(commands.getLlmProviderStatus).mockResolvedValue({
      provider_type: 'openai',
      model: 'gpt-4o-mini',
      api_key_configured: true,
    });
    vi.mocked(commands.getCustomProviders).mockResolvedValue([]);
    vi.mocked(commands.getProviderModels).mockResolvedValue([]);
    vi.mocked(commands.reconfigureLlmProvider).mockResolvedValue('openai');
    vi.mocked(commands.updateSetting).mockResolvedValue(undefined);
  });

  it('renders the settings heading', () => {
    render(SettingsView);
    expect(screen.getByRole('heading', { name: /settings/i })).toBeTruthy();
  });

  it('uses shared buttons for provider actions', () => {
    render(SettingsView);
    expect(screen.getByRole('button', { name: /save settings/i })).toHaveClass('button');
    expect(screen.getByRole('button', { name: /save & connect/i })).toHaveClass('button');
  });

  it('uses a saved French locale for the settings heading', async () => {
    vi.mocked(commands.getSettings).mockResolvedValue({ ui_locale: 'fr' });

    render(SettingsView);

    expect(await screen.findByRole('heading', { name: 'Paramètres' })).toBeTruthy();
  });

  it('persists a selected German display language immediately', async () => {
    render(SettingsView);

    const language = await screen.findByLabelText(/display language|langue d’affichage/i);
    await fireEvent.change(language, { target: { value: 'de' } });

    await waitFor(() => {
      expect(commands.updateSetting).toHaveBeenCalledWith('ui_locale', 'de');
    });
  });

  it('rolls back the display language when saving it fails', async () => {
    vi.mocked(commands.getSettings).mockResolvedValue({ ui_locale: 'en' });
    vi.mocked(commands.updateSetting).mockRejectedValueOnce(new Error('write failed'));
    render(SettingsView);

    const language = await screen.findByLabelText('Display language');
    await fireEvent.change(language, { target: { value: 'de' } });

    await waitFor(() => expect(screen.getByText(/failed to save language/i)).toBeTruthy());
    expect((language as HTMLSelectElement).value).toBe('en');
    expect(screen.getByRole('heading', { name: 'Settings' })).toBeTruthy();
  });

  it('restores the last persisted language when rapid saves both fail', async () => {
    let rejectGerman: ((reason?: unknown) => void) | undefined;
    let rejectFrench: ((reason?: unknown) => void) | undefined;
    vi.mocked(commands.getSettings).mockResolvedValue({ ui_locale: 'en' });
    vi.mocked(commands.updateSetting)
      .mockImplementationOnce(
        () =>
          new Promise<void>((_resolve, reject) => {
            rejectGerman = reject;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise<void>((_resolve, reject) => {
            rejectFrench = reject;
          }),
      );
    render(SettingsView);

    const language = await screen.findByLabelText('Display language');
    await fireEvent.change(language, { target: { value: 'de' } });
    await fireEvent.change(language, { target: { value: 'fr' } });
    await waitFor(() => expect(commands.updateSetting).toHaveBeenCalledTimes(1));
    rejectGerman?.(new Error('German write failed'));
    await waitFor(() => expect(commands.updateSetting).toHaveBeenCalledTimes(2));
    rejectFrench?.(new Error('French write failed'));

    await waitFor(() => expect(screen.getByText(/failed to save language/i)).toBeTruthy());
    expect((language as HTMLSelectElement).value).toBe('en');
    expect(screen.getByRole('heading', { name: 'Settings' })).toBeTruthy();
  });

  it('serializes locale saves and restores the backend locale after a newer failure', async () => {
    let persistedLocale = 'en';
    let resolveGerman: (() => void) | undefined;
    vi.mocked(commands.getSettings).mockResolvedValue({ ui_locale: persistedLocale });
    vi.mocked(commands.updateSetting)
      .mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            resolveGerman = () => {
              persistedLocale = 'de';
              resolve();
            };
          }),
      )
      .mockRejectedValueOnce(new Error('French write failed'))
      .mockImplementationOnce(async (_key, value) => {
        persistedLocale = value;
      });
    render(SettingsView);

    const language = await screen.findByLabelText('Display language');
    await fireEvent.change(language, { target: { value: 'de' } });
    await fireEvent.change(language, { target: { value: 'fr' } });
    await waitFor(() => expect(commands.updateSetting).toHaveBeenCalledTimes(1));
    resolveGerman?.();
    await waitFor(() => expect(commands.updateSetting).toHaveBeenCalledTimes(3));

    expect(commands.updateSetting).toHaveBeenNthCalledWith(1, 'ui_locale', 'de');
    expect(commands.updateSetting).toHaveBeenNthCalledWith(2, 'ui_locale', 'fr');
    expect(commands.updateSetting).toHaveBeenNthCalledWith(3, 'ui_locale', 'en');
    expect(persistedLocale).toBe('en');
    expect((language as HTMLSelectElement).value).toBe('en');
  });

  it('displays current provider status after mount', async () => {
    render(SettingsView);
    await waitFor(() => expect(commands.getLlmProviderStatus).toHaveBeenCalled());
    await waitFor(() => {
      expect(screen.getByText('openai')).toBeTruthy();
    });
  });

  it('shows custom providers section', async () => {
    render(SettingsView);
    await waitFor(() => expect(commands.getCustomProviders).toHaveBeenCalled());
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /custom providers/i })).toBeTruthy();
    });
  });

  it('lists a custom provider when one exists', async () => {
    vi.mocked(commands.getCustomProviders).mockResolvedValue([
      {
        id: 'cp1',
        name: 'My Ollama',
        provider_type: 'openai',
        base_url: 'http://localhost:11434',
        api_key: '',
      },
    ]);
    render(SettingsView);
    await waitFor(() => expect(screen.getByText('My Ollama')).toBeTruthy());
  });

  it('blocks Connect with an inline error when the API key is empty', async () => {
    render(SettingsView);
    const apiKey = await screen.findByLabelText(/api key/i);
    await fireEvent.input(apiKey, { target: { value: '   ' } });
    await fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => {
      expect(screen.getByText(/api key is required/i)).toBeTruthy();
    });
    expect(commands.reconfigureLlmProvider).not.toHaveBeenCalled();
    expect(commands.updateSetting).not.toHaveBeenCalled();
  });

  it('blocks Connect with an inline error when the base URL is malformed', async () => {
    vi.mocked(commands.getSettings).mockResolvedValue({
      llm_provider: 'ollama',
      llm_base_url: 'not a url',
    });
    render(SettingsView);
    await waitFor(() => {
      expect((screen.getByLabelText(/base url/i) as HTMLInputElement).value).toBe('not a url');
    });
    await fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => {
      expect(screen.getByText(/not a valid url/i)).toBeTruthy();
    });
    expect(commands.reconfigureLlmProvider).not.toHaveBeenCalled();
  });

  it('calls updateSetting for all four setting fields when Save Settings is clicked', async () => {
    render(SettingsView);
    // Wait for mount to complete
    await waitFor(() => expect(commands.getSettings).toHaveBeenCalled());
    const saveButton = screen.getByRole('button', { name: /save settings/i });
    await fireEvent.click(saveButton);
    await waitFor(() => {
      expect(commands.updateSetting).toHaveBeenCalledTimes(4);
    });
    expect(commands.updateSetting).toHaveBeenCalledWith('llm_provider', expect.any(String));
  });

  it('renders the Rebuild relationship links button', async () => {
    render(SettingsView);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /rebuild relationship links/i })).toBeTruthy();
    });
  });

  it('calls resyncWikilinks and shows success feedback with the entity count', async () => {
    vi.mocked(commands.resyncWikilinks).mockResolvedValue(42);
    render(SettingsView);
    const button = await screen.findByRole('button', { name: /rebuild relationship links/i });
    await fireEvent.click(button);
    await waitFor(() => {
      expect(commands.resyncWikilinks).toHaveBeenCalledTimes(1);
    });
    await waitFor(() => {
      expect(screen.getByText(/rebuilt links across 42 entities/i)).toBeTruthy();
    });
  });

  it('shows an error message when resyncWikilinks rejects', async () => {
    vi.mocked(commands.resyncWikilinks).mockRejectedValue(new Error('graph error'));
    render(SettingsView);
    const button = await screen.findByRole('button', { name: /rebuild relationship links/i });
    await fireEvent.click(button);
    await waitFor(() => {
      expect(screen.getByText(/rebuild failed/i)).toBeTruthy();
    });
    await waitFor(() => {
      expect(screen.getByText(/graph error/i)).toBeTruthy();
    });
  });
});
