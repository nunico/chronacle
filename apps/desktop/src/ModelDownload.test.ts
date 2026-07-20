import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ModelDownload from './ModelDownload.svelte';
import { setUiLocalePreference } from './lib/locale.svelte';

vi.mock('./lib/commands', () => ({
  downloadEmbeddingModel: vi.fn(),
  getEmbeddingProviderStatus: vi.fn().mockResolvedValue({
    backend: 'local',
    local_available: true,
    local_cached: false,
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

import * as commands from './lib/commands';

describe('ModelDownload', () => {
  beforeEach(() => {
    vi.mocked(commands.getEmbeddingProviderStatus).mockResolvedValue({
      backend: 'local',
      model: 'nomic-embed-text-v1.5',
      dimension: 768,
      api_key_configured: false,
      local_available: true,
      local_cached: false,
    });
  });

  it('uses shared accessible controls for model download', async () => {
    setUiLocalePreference('en');
    render(ModelDownload, { props: { onModelReady: vi.fn() } });

    expect(await screen.findByRole('button', { name: 'Start download' })).toBeTruthy();
  });

  it('identifies the selected multilingual local model before downloading it', async () => {
    vi.mocked(commands.getEmbeddingProviderStatus).mockResolvedValue({
      backend: 'local',
      mode: 'local_multilingual',
      model: 'multilingual-e5-base',
      dimension: 768,
      api_key_configured: false,
      local_available: true,
      local_cached: false,
    });
    setUiLocalePreference('en');
    render(ModelDownload, { props: { onModelReady: vi.fn() } });

    expect(await screen.findByText(/multilingual-e5-base/)).toBeTruthy();
  });
});
