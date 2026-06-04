import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import App from './App.svelte';

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const checkEmbeddingModel = vi.fn();
const downloadEmbeddingModel = vi.fn();
const getCampaigns = vi.fn();
const getCollections = vi.fn();
const getChatHistory = vi.fn();
const getSettings = vi.fn();
const getLlmProviderStatus = vi.fn();
const getCustomProviders = vi.fn();
const getEmbeddingModelMismatch = vi.fn();
const reindexAllSources = vi.fn();

vi.mock('./lib/commands', () => ({
  checkEmbeddingModel: (...a: unknown[]) => checkEmbeddingModel(...a),
  downloadEmbeddingModel: (...a: unknown[]) => downloadEmbeddingModel(...a),
  getCampaigns: (...a: unknown[]) => getCampaigns(...a),
  getCollections: (...a: unknown[]) => getCollections(...a),
  getChatHistory: (...a: unknown[]) => getChatHistory(...a),
  getSettings: (...a: unknown[]) => getSettings(...a),
  getLlmProviderStatus: (...a: unknown[]) => getLlmProviderStatus(...a),
  getCustomProviders: (...a: unknown[]) => getCustomProviders(...a),
  getEmbeddingModelMismatch: (...a: unknown[]) => getEmbeddingModelMismatch(...a),
  reindexAllSources: (...a: unknown[]) => reindexAllSources(...a),
  getMruCollectionId: vi.fn().mockReturnValue(null),
  setMruCollectionId: vi.fn(),
}));

vi.mock('./lib/events', () => ({
  onChatToken: vi.fn().mockResolvedValue(() => {}),
  onEmbeddingModelMismatch: vi.fn().mockResolvedValue(() => {}),
}));

describe('App — model-download gate', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    checkEmbeddingModel.mockResolvedValue(false);
    downloadEmbeddingModel.mockResolvedValue(undefined);
    getCampaigns.mockResolvedValue([]);
    getCollections.mockResolvedValue([]);
    getChatHistory.mockResolvedValue([]);
    getSettings.mockResolvedValue({});
    getLlmProviderStatus.mockResolvedValue({
      provider_type: 'openai',
      model: 'gpt-4o-mini',
      api_key_configured: false,
    });
    getCustomProviders.mockResolvedValue([]);
    getEmbeddingModelMismatch.mockResolvedValue({ active_model: 'mock', stale: [] });
    reindexAllSources.mockResolvedValue(0);
  });

  it('shows the ModelDownload gate before the model is ready', async () => {
    render(App);
    // ModelDownload renders some recognizable text; either way the rail
    // is not yet rendered.
    await waitFor(() => {
      expect(screen.queryByLabelText('Campaign rail')).toBeNull();
    });
  });

  it('renders the Shell once the model is ready', async () => {
    checkEmbeddingModel.mockResolvedValue(true);
    render(App);
    await waitFor(() => {
      expect(screen.getByLabelText('Campaign rail')).toBeTruthy();
    });
    // Oracle nav item is present
    expect(screen.getByRole('button', { name: /Oracle/i })).toBeTruthy();
    // Campaign & sources footer button
    expect(
      screen.getByRole('button', { name: /Campaign.*&.*sources/i }),
    ).toBeTruthy();
    // Settings icon-only button by aria-label
    expect(screen.getByRole('button', { name: /^Settings$/i })).toBeTruthy();
  });
});
