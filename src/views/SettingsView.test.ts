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
  getCustomProviders: vi.fn().mockResolvedValue([]),
  createCustomProvider: vi.fn().mockResolvedValue(undefined),
  deleteCustomProvider: vi.fn().mockResolvedValue(undefined),
  getProviderModels: vi.fn().mockResolvedValue([]),
  addProviderModel: vi.fn().mockResolvedValue(undefined),
  removeProviderModel: vi.fn().mockResolvedValue(undefined),
  reindexAllSources: vi.fn().mockResolvedValue(0),
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
});
