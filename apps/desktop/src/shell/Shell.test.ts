import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import Shell from './Shell.svelte';
import { open } from '@tauri-apps/plugin-dialog';
import { clearToasts } from '../lib/toast.svelte';
import { i18n } from '../lib/locale.svelte';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import type { WindowClosePort } from '../lib/drafts/window-close';

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
const getMaintenanceCounts = vi.fn();
const getProposals = vi.fn();
const onEmbeddingModelMismatch = vi.fn();
const getEntities = vi.fn();
const chatSend = vi.fn().mockResolvedValue(undefined);
const createEntity = vi.fn();
const updateEntity = vi.fn();
const updateSession = vi.fn();
const updateRuleNotes = vi.fn();

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
  chatSend: (...a: unknown[]) => chatSend(...a),
  getChunkForCitation: vi.fn().mockResolvedValue(null),
  getMruCollectionId: vi.fn().mockReturnValue(null),
  setMruCollectionId: vi.fn(),
  getEntities: (...a: unknown[]) => getEntities(...a),
  createEntity: (...a: unknown[]) => createEntity(...a),
  updateEntity: (...a: unknown[]) => updateEntity(...a),
  deleteEntity: vi.fn(),
  getEntityRelations: vi.fn().mockResolvedValue([]),
  listVaultConflicts: vi.fn().mockResolvedValue([]),
  getMaintenanceCounts: (...a: unknown[]) => getMaintenanceCounts(...a),
  getProposals: (...a: unknown[]) => getProposals(...a),
  acceptProposal: vi.fn(),
  rejectProposal: vi.fn(),
  saveChatToCodex: vi.fn().mockResolvedValue(0),
  getSources: vi.fn().mockResolvedValue([]),
  updateSession: (...a: unknown[]) => updateSession(...a),
  updateRuleNotes: (...a: unknown[]) => updateRuleNotes(...a),
}));

vi.mock('../lib/events', () => ({
  onEmbeddingModelMismatch: (...a: unknown[]) => onEmbeddingModelMismatch(...a),
}));

async function openPicker() {
  vi.mocked(open).mockResolvedValue('/tmp/rules.pdf');
  const uploadBtn = await screen.findByRole('button', { name: /Upload PDF/i });
  await fireEvent.click(uploadBtn);
  return screen.findByRole('dialog');
}

class FakeWindowClosePort implements WindowClosePort {
  private handlers = new Set<(event: { preventDefault(): void }) => void>();
  readonly unlisten = vi.fn();
  readonly destroy = vi.fn().mockResolvedValue(undefined);
  readonly onCloseRequested = vi.fn(
    async (handler: (event: { preventDefault(): void }) => void) => {
      this.handlers.add(handler);
      return () => {
        this.handlers.delete(handler);
        this.unlisten();
      };
    },
  );

  requestClose() {
    const event = { preventDefault: vi.fn() };
    for (const handler of this.handlers) handler(event);
    return event;
  }
}

describe('Shell upload flow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    i18n.setLocale('en');
    eventHandlers.clear();
    clearToasts();
    globalThis.localStorage?.clear();
    getCampaigns.mockResolvedValue([{ id: 'camp-1', name: 'Test Campaign', system: 'D&D 5e' }]);
    onEmbeddingModelMismatch.mockImplementation(async () => () => {});
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
    getMaintenanceCounts.mockResolvedValue({ pending_proposals: 0, unresolved_findings: 0 });
    getProposals.mockResolvedValue([]);
    getEntities.mockResolvedValue([]);
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

  it('translates note categories and shortcut help with the active locale', async () => {
    i18n.setLocale('de');
    render(Shell);
    await screen.findByRole('button', { name: /NSCs/i });

    await fireEvent.keyDown(document.body, { key: '?' });
    expect(await screen.findByText('Orakel (Chat)')).toBeTruthy();
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
      expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '40');
    });
  });

  it('shows the explicit re-index workflow after a live model-mismatch event', async () => {
    let mismatchCallback:
      | ((payload: {
          active_model: string;
          stale: Array<{ embed_model: string; source_count: number }>;
        }) => void)
      | undefined;
    onEmbeddingModelMismatch.mockImplementation(async (callback) => {
      mismatchCallback = callback;
      return () => {};
    });
    getEmbeddingModelMismatch.mockResolvedValue({
      active_model: 'multilingual-e5-base',
      stale: [],
    });
    render(Shell);

    await waitFor(() => expect(onEmbeddingModelMismatch).toHaveBeenCalled());
    mismatchCallback?.({
      active_model: 'multilingual-e5-base',
      stale: [{ embed_model: 'nomic-embed-text-v1.5', source_count: 2 }],
    });

    expect(await screen.findByTestId('mismatch-banner')).toBeTruthy();
    expect(screen.getByTestId('mismatch-reindex')).toBeTruthy();
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

  it('rail shows Maintenance item with badge when counts are non-zero', async () => {
    getMaintenanceCounts.mockResolvedValue({ pending_proposals: 3, unresolved_findings: 1 });
    render(Shell);
    const maintenanceItem = await screen.findByRole('button', { name: /Maintenance/i });
    await waitFor(() => {
      expect(maintenanceItem.textContent).toContain('4');
    });
  });

  it('clicking Maintenance renders MaintenanceView', async () => {
    getMaintenanceCounts.mockResolvedValue({ pending_proposals: 1, unresolved_findings: 0 });
    render(Shell);
    const maintenanceItem = await screen.findByRole('button', { name: /Maintenance/i });
    await fireEvent.click(maintenanceItem);
    await waitFor(() => {
      expect(getProposals).toHaveBeenCalledWith('pending');
    });
  });
});

describe('Shell keyboard shortcuts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    eventHandlers.clear();
    clearToasts();
    globalThis.localStorage?.clear();
    getCampaigns.mockResolvedValue([{ id: 'camp-1', name: 'Test Campaign', system: 'D&D 5e' }]);
    getCollections.mockResolvedValue([{ id: 'col-1', name: 'Core Books' }]);
    getChatHistory.mockResolvedValue([]);
    getEmbeddingModelMismatch.mockResolvedValue({ active_model: 'mock', stale: [] });
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
    getMaintenanceCounts.mockResolvedValue({ pending_proposals: 0, unresolved_findings: 0 });
    getProposals.mockResolvedValue([]);
    getEntities.mockResolvedValue([]);
  });

  it('? opens the shortcuts help overlay and Escape closes it', async () => {
    render(Shell);
    await screen.findByRole('button', { name: /NPCs/i });

    await fireEvent.keyDown(document.body, { key: '?' });
    expect(await screen.findByText(/Keyboard shortcuts/i)).toBeTruthy();

    await fireEvent.keyDown(document.body, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByText(/Keyboard shortcuts/i)).toBeNull());
  });

  it('g n navigates to the NPC manager', async () => {
    render(Shell);
    await screen.findByRole('button', { name: /NPCs/i });

    await fireEvent.keyDown(document.body, { key: 'g' });
    await fireEvent.keyDown(document.body, { key: 'n' });

    expect(await screen.findByRole('button', { name: /New NPC/i })).toBeTruthy();
  });

  it('c opens the create form inside an entity manager', async () => {
    render(Shell);
    await screen.findByRole('button', { name: /NPCs/i });
    await fireEvent.keyDown(document.body, { key: 'g' });
    await fireEvent.keyDown(document.body, { key: 'n' });
    await screen.findByRole('button', { name: /New NPC/i });

    await fireEvent.keyDown(document.body, { key: 'c' });
    // The create form (EntityForm) exposes a labelled name field.
    expect(await screen.findByLabelText('Name', { exact: true })).toBeTruthy();
  });

  it('does not trigger shortcuts while typing in a field', async () => {
    render(Shell);
    await screen.findByRole('button', { name: /NPCs/i });
    await fireEvent.keyDown(document.body, { key: 'g' });
    await fireEvent.keyDown(document.body, { key: 'n' });
    await fireEvent.keyDown(document.body, { key: 'c' });
    const nameField = await screen.findByLabelText('Name', { exact: true });

    // Pressing ? while focused in the name field must NOT open the overlay.
    await fireEvent.keyDown(nameField, { key: '?' });
    expect(screen.queryByText(/Keyboard shortcuts/i)).toBeNull();
  });

  it('keeps one Oracle draft coordinator alive while the view is unmounted', async () => {
    render(Shell);
    const composer = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(composer, { target: { value: 'Remember the silver key' } });

    await fireEvent.click(screen.getByRole('button', { name: /Campaign & sources/i }));
    expect(screen.queryByPlaceholderText('Ask a rule, a name, a place…')).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: /^Oracle$/i }));

    expect(await screen.findByPlaceholderText('Ask a rule, a name, a place…')).toHaveValue(
      'Remember the silver key',
    );
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
  });

  it('keeps Oracle drafts isolated while switching campaigns in the shell', async () => {
    getCampaigns.mockResolvedValue([
      { id: 'camp-1', name: 'Campaign A', system: 'D&D 5e' },
      { id: 'camp-2', name: 'Campaign B', system: 'D&D 5e' },
    ]);
    render(Shell);
    const composer = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(composer, { target: { value: 'Campaign A question' } });

    await fireEvent.click(screen.getByRole('button', { name: 'Switch campaign' }));
    await fireEvent.click(await screen.findByRole('button', { name: /Campaign B/ }));
    expect(screen.getByPlaceholderText('Ask a rule, a name, a place…')).toHaveValue('');
    await fireEvent.input(screen.getByPlaceholderText('Ask a rule, a name, a place…'), {
      target: { value: 'Campaign B question' },
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Switch campaign' }));
    await fireEvent.click(await screen.findByRole('button', { name: /Campaign A/ }));
    expect(screen.getByPlaceholderText('Ask a rule, a name, a place…')).toHaveValue(
      'Campaign A question',
    );
  });

  it('restores an entity draft after navigating through another shell view', async () => {
    getEntities.mockImplementation(async (_campaignId, kind) => {
      if (kind !== 'npc') return [];
      return [
        {
          id: 'mira',
          kind: 'npc',
          campaign_id: 'camp-1',
          name: 'Mira',
          aliases: [],
          summary: 'Scout',
          notes: 'Saved note',
          created_at: null,
          updated_at: null,
          date_start: null,
          date_end: null,
          is_ongoing: null,
          sequence_index: null,
          era: null,
          duration_label: null,
          session_id: null,
          player_name: null,
          character_class: null,
          character_level: null,
          status: null,
          codex_article: null,
          codex_stale: null,
          codex_compiled_at: null,
        },
      ];
    });
    render(Shell);
    await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.click(screen.getByRole('button', { name: /NPCs/i }));
    await fireEvent.click(await screen.findByText('Mira'));
    await fireEvent.input(screen.getByLabelText('Notes'), {
      target: { value: 'Mira has the silver key' },
    });

    await fireEvent.click(screen.getByRole('button', { name: /^Oracle$/i }));
    await fireEvent.click(screen.getByRole('button', { name: /NPCs/i }));
    await fireEvent.click(await screen.findByText('Mira'));

    expect(screen.getByLabelText('Notes')).toHaveValue('Mira has the silver key');
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
  });
});

describe('Shell native close protection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    i18n.setLocale('en');
    eventHandlers.clear();
    clearToasts();
    globalThis.localStorage?.clear();
    getCampaigns.mockResolvedValue([{ id: 'camp-1', name: 'Test Campaign', system: 'D&D 5e' }]);
    onEmbeddingModelMismatch.mockImplementation(async () => () => {});
    getCollections.mockResolvedValue([{ id: 'col-1', name: 'Core Books' }]);
    getChatHistory.mockResolvedValue([]);
    getEmbeddingModelMismatch.mockResolvedValue({ active_model: 'mock', stale: [] });
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
    getMaintenanceCounts.mockResolvedValue({ pending_proposals: 0, unresolved_findings: 0 });
    getProposals.mockResolvedValue([]);
    getEntities.mockResolvedValue([]);
  });

  it('registers once, leaves a clean native close unprevented, and unlistens on teardown', async () => {
    const port = new FakeWindowClosePort();
    const coordinator = new DraftCoordinator();
    const rendered = render(Shell, {
      props: { windowClosePort: port, draftCoordinator: coordinator },
    });
    await waitFor(() => expect(port.onCloseRequested).toHaveBeenCalledOnce());

    const event = port.requestClose();

    expect(event.preventDefault).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog', { name: 'Unsaved changes' })).toBeNull();
    expect(port.destroy).not.toHaveBeenCalled();
    rendered.unmount();
    expect(port.unlisten).toHaveBeenCalledOnce();
  });

  it('unlistens if asynchronous registration finishes after Shell is destroyed', async () => {
    let resolveRegistration!: (unlisten: () => void) => void;
    const unlisten = vi.fn();
    const port: WindowClosePort = {
      onCloseRequested: vi.fn(
        () =>
          new Promise<() => void>((resolve) => {
            resolveRegistration = resolve;
          }),
      ),
      destroy: vi.fn().mockResolvedValue(undefined),
    };
    const rendered = render(Shell, {
      props: { windowClosePort: port, draftCoordinator: new DraftCoordinator() },
    });
    expect(port.onCloseRequested).toHaveBeenCalledOnce();

    rendered.unmount();
    resolveRegistration(unlisten);

    await waitFor(() => expect(unlisten).toHaveBeenCalledOnce());
  });

  it.each([
    {
      label: 'pending Oracle',
      arrange(coordinator: DraftCoordinator) {
        coordinator.open('oracle:camp-1', null, '');
        coordinator.revise('oracle:camp-1', 'Unsent question');
      },
    },
    {
      label: 'pending entity',
      arrange(coordinator: DraftCoordinator) {
        coordinator.open('entity:camp-1:npc:mira', 'entity:npc:mira', { name: 'Mira' });
        coordinator.revise('entity:camp-1:npc:mira', { name: 'Mira the Bold' });
      },
    },
    {
      label: 'pending rule note',
      arrange(coordinator: DraftCoordinator) {
        coordinator.open('rule:camp-1:book:r1', 'rule:r1', { notes: 'Saved note' });
        coordinator.revise('rule:camp-1:book:r1', { notes: 'Changed note' });
      },
    },
    {
      label: 'saving session',
      arrange(coordinator: DraftCoordinator) {
        coordinator.open('session:camp-1:s1', 'session:s1', 'Saved');
        coordinator.revise('session:camp-1:s1', 'Changed');
        void coordinator.requestSave<string>(
          'session:camp-1:s1',
          () => new Promise<string>(() => {}),
        );
      },
    },
    {
      label: 'failed rule note',
      async arrange(coordinator: DraftCoordinator) {
        coordinator.open('rule:camp-1:book:r1', 'rule:r1', { notes: 'Saved note' });
        coordinator.revise('rule:camp-1:book:r1', { notes: 'Changed note' });
        await coordinator.requestSave<{ notes: string }>(
          'rule:camp-1:book:r1',
          async (): Promise<{ notes: string }> => {
            throw new Error('Database unavailable');
          },
        );
      },
    },
  ])('prevents close without replacing the opener for $label work', async ({ arrange }) => {
    const port = new FakeWindowClosePort();
    const coordinator = new DraftCoordinator();
    await arrange(coordinator);
    render(Shell, { props: { windowClosePort: port, draftCoordinator: coordinator } });
    const composer = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    composer.focus();
    await waitFor(() => expect(port.onCloseRequested).toHaveBeenCalledOnce());

    const first = port.requestClose();
    const duplicate = port.requestClose();

    expect(first.preventDefault).toHaveBeenCalledOnce();
    expect(duplicate.preventDefault).toHaveBeenCalledOnce();
    expect(screen.getAllByRole('dialog', { name: 'Unsaved changes' })).toHaveLength(1);
    expect(port.onCloseRequested).toHaveBeenCalledOnce();
    await fireEvent.keyDown(screen.getByRole('dialog', { name: 'Unsaved changes' }), {
      key: 'Escape',
    });
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Unsaved changes' })).toBeNull(),
    );
    expect(document.activeElement).toBe(composer);
    expect(coordinator.atRiskCount()).toBe(1);
  });

  it('cancels back to the exact opener and only explicit discard destroys without saving', async () => {
    const port = new FakeWindowClosePort();
    const coordinator = new DraftCoordinator();
    coordinator.open('oracle:camp-1', null, '');
    coordinator.revise('oracle:camp-1', 'Where is the silver key?');
    render(Shell, { props: { windowClosePort: port, draftCoordinator: coordinator } });
    const composer = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    composer.focus();
    await waitFor(() => expect(port.onCloseRequested).toHaveBeenCalledOnce());

    port.requestClose();
    await fireEvent.keyDown(screen.getByRole('dialog', { name: 'Unsaved changes' }), {
      key: 'Escape',
    });
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Unsaved changes' })).toBeNull(),
    );
    expect(document.activeElement).toBe(composer);
    expect(coordinator.atRiskCount()).toBe(1);

    port.requestClose();
    const discard = screen.getByRole('button', { name: 'Discard and close' });
    discard.focus();
    await fireEvent.keyDown(discard, { key: 'Enter' });

    await waitFor(() => expect(port.destroy).toHaveBeenCalledOnce());
    expect(coordinator.atRiskCount()).toBe(0);
    expect(chatSend).not.toHaveBeenCalled();
    expect(createEntity).not.toHaveBeenCalled();
    expect(updateEntity).not.toHaveBeenCalled();
    expect(updateSession).not.toHaveBeenCalled();
    expect(updateRuleNotes).not.toHaveBeenCalled();
  });
});
