import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
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

describe('ModelDownload', () => {
  it('uses shared accessible controls for model download', async () => {
    setUiLocalePreference('en');
    render(ModelDownload, { props: { onModelReady: vi.fn() } });

    expect(await screen.findByRole('button', { name: 'Start download' })).toBeTruthy();
  });
});
