import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import Shell from './Shell.svelte';
import { open } from '@tauri-apps/plugin-dialog';
import { clearToasts } from '../lib/toast.svelte';

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

type Handler = (event: { payload: unknown }) => void;
const eventHandlers = new Map<string, Set<Handler>>();

function emitTauriEvent(name: string, payload: unknown) {
  for (const h of eventHandlers.get(name) ?? []) h({ payload });
}

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((name: string, handler: Handler) => {
    const handlers = eventHandlers.get(name) ?? new Set<Handler>();
    eventHandlers.set(name, handlers);
    handlers.add(handler);
    return Promise.resolve(() => handlers.delete(handler));
  }),
}));

const getCampaigns = vi.fn();
const getEntityCounts = vi.fn();
const getSessions = vi.fn();
const getCollections = vi.fn();
const getChatHistory = vi.fn();
const getEmbeddingModelMismatch = vi.fn();
const reindexAllSources = vi.fn();
const uploadSource = vi.fn();
const createCollection = vi.fn();

vi.mock('../lib/commands', () => ({
  getCampaigns: (...a: unknown[]) => getCampaigns(...a),
  getEntityCounts: (...a: unknown[]) => getEntityCounts(...a),
  getSessions: (...a: unknown[]) => getSessions(...a),
  getCollections: (...a: unknown[]) => getCollections(...a),
  getChatHistory: (...a: unknown[]) => getChatHistory(...a),
  getEmbeddingModelMismatch: (...a: unknown[]) => getEmbeddingModelMismatch(...a),
  reindexAllSources: (...a: unknown[]) => reindexAllSources(...a),
  uploadSource: (...a: unknown[]) => uploadSource(...a),
  createCollection: (...a: unknown[]) => createCollection(...a),
  chatSend: vi.fn().mockResolvedValue(undefined),
  getChunkForCitation: vi.fn().mockResolvedValue(null),
  getMruCollectionId: vi.fn().mockReturnValue(null),
  setMruCollectionId: vi.fn(),
}));

vi.mock('../lib/events', () => ({
  onEmbeddingModelMismatch: vi.fn().mockResolvedValue(() => {}),
}));

async function openPicker() {
  vi.mocked(open).mockResolvedValue('/tmp/rules.pdf');
  const uploadBtn = await screen.findByRole('button', { name: /Upload PDF/i });
  await fireEvent.click(uploadBtn);
  return screen.findByRole('dialog');
}

describe('Shell upload flow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    eventHandlers.clear();
    clearToasts();
    globalThis.localStorage?.clear();
    getCampaigns.mockResolvedValue([
      { id: 'camp-1', name: 'Test Campaign', system: 'D&D 5e' },
    ]);
    getCollections.mockResolvedValue([{ id: 'col-1', name: 'Core Books' }]);
    getChatHistory.mockResolvedValue([]);
    getEmbeddingModelMismatch.mockResolvedValue({ active_model: 'mock', stale: [] });
    uploadSource.mockResolvedValue({});
    getEntityCounts.mockResolvedValue({
      npc: 0,
      location: 0,
      faction: 0,
      creature: 0,
      item: 0,
      event: 0,
      player_character: 0,
      misc: 0,
    });
    getSessions.mockResolvedValue([]);
  });

  it('shows real entity and session counts in the rail', async () => {
    getEntityCounts.mockResolvedValue({
      npc: 2,
      location: 5,
      faction: 0,
      creature: 0,
      item: 0,
      event: 0,
      player_character: 0,
      misc: 0,
    });
    getSessions.mockResolvedValue([{ id: 'sess-1' }]);
    render(Shell);
    const npcItem = await screen.findByRole('button', { name: /NPCs/i });
    await waitFor(() => {
      expect(npcItem.textContent).toContain('2');
    });
    expect(screen.getByRole('button', { name: /Locations/i }).textContent).toContain('5');
    expect(screen.getByRole('button', { name: /^Sessions/i }).textContent).toContain('1');
    expect(getEntityCounts).toHaveBeenCalledWith('camp-1');
  });

  it('Escape closes the collection picker dialog', async () => {
    render(Shell);
    const dialog = await openPicker();
    await fireEvent.keyDown(dialog, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('blocks a second upload while one is in progress', async () => {
    // uploadSource hangs to keep the upload active.
    uploadSource.mockReturnValue(new Promise(() => {}));
    render(Shell);
    await openPicker();
    await fireEvent.click(screen.getByRole('button', { name: /^Upload$/ }));
    // Picker is gone, upload is active. Try to start another upload.
    const uploadBtn = screen.getByRole('button', { name: /Upload PDF/i });
    await fireEvent.click(uploadBtn);
    await waitFor(() => {
      expect(screen.getByText(/already in progress/i)).toBeTruthy();
    });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(vi.mocked(open)).toHaveBeenCalledTimes(1);
  });

  it('shows an error strip and toast when ingestion fails', async () => {
    uploadSource.mockImplementation(async () => {
      emitTauriEvent('ingestion-error', { source_id: 's1', error: 'corrupt PDF' });
      throw new Error('PDF ingestion failed: corrupt PDF');
    });
    render(Shell);
    await openPicker();
    await fireEvent.click(screen.getByRole('button', { name: /^Upload$/ }));
    await waitFor(() => {
      expect(screen.getAllByText(/corrupt PDF/).length).toBeGreaterThan(0);
    });
    // Error strip persists with a dismiss control.
    const dismissButtons = screen.getAllByRole('button', { name: /dismiss/i });
    expect(dismissButtons.length).toBeGreaterThan(0);
  });

  it('shows reindex progress in the mismatch banner', async () => {
    getEmbeddingModelMismatch.mockResolvedValue({
      active_model: 'new-model',
      stale: [{ embed_model: 'old-model', source_count: 3 }],
    });
    reindexAllSources.mockImplementation(async () => {
      emitTauriEvent('reindex-progress', {
        source_id: 's1',
        current: 2,
        total: 5,
        progress: 0.4,
        step: 'Embedding chunks',
      });
      return new Promise(() => {});
    });
    render(Shell);
    const reindexBtn = await screen.findByTestId('mismatch-reindex');
    await fireEvent.click(reindexBtn);
    await waitFor(() => {
      expect(screen.getByTestId('mismatch-banner').textContent).toMatch(/2\s*\/\s*5/);
      expect(screen.getByTestId('mismatch-banner').textContent).toContain('Embedding chunks');
    });
  });

  it('surfaces a reindex failure in the mismatch banner', async () => {
    getEmbeddingModelMismatch.mockResolvedValue({
      active_model: 'new-model',
      stale: [{ embed_model: 'old-model', source_count: 3 }],
    });
    reindexAllSources.mockRejectedValue(new Error('embedding backend offline'));
    render(Shell);
    const reindexBtn = await screen.findByTestId('mismatch-reindex');
    await fireEvent.click(reindexBtn);
    await waitFor(() => {
      expect(screen.getByTestId('mismatch-banner').textContent).toMatch(
        /embedding backend offline/i,
      );
    });
    // Button is usable again for a retry.
    expect((screen.getByTestId('mismatch-reindex') as HTMLButtonElement).disabled).toBe(false);
  });

  it('shows Ready! when ingestion completes', async () => {
    uploadSource.mockImplementation(async () => {
      emitTauriEvent('ingestion-progress', {
        source_id: 's1',
        status: 'indexing',
        progress: 0.5,
        step: 'Embedding chunks',
      });
      emitTauriEvent('ingestion-progress', { source_id: 's1', status: 'done', progress: 1.0 });
      return {};
    });
    render(Shell);
    await openPicker();
    await fireEvent.click(screen.getByRole('button', { name: /^Upload$/ }));
    await waitFor(() => {
      expect(screen.getByText('Ready!')).toBeTruthy();
    });
  });
});
