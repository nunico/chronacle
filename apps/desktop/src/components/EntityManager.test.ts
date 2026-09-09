import { render, screen, fireEvent, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import EntityManager from './EntityManager.svelte';
import type { GraphNode } from '../lib/commands';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import { entityScope, oracleScope } from '../lib/drafts/draft-state';

vi.mock('../lib/commands', () => ({
  getEntities: vi.fn().mockResolvedValue([]),
  getEntity: vi.fn(),
  createEntity: vi.fn(),
  updateEntity: vi.fn(),
  softDeleteEntity: vi.fn(),
  getSessions: vi.fn().mockResolvedValue([]),
  getEntityRelations: vi.fn().mockResolvedValue([]),
  listVaultConflicts: vi.fn().mockResolvedValue([]),
  compileEntity: vi.fn(),
}));

import * as commands from '../lib/commands';

const mockNpc = (overrides: Partial<GraphNode> = {}): GraphNode => ({
  id: 'npc1',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
  aliases: [],
  summary: 'Shady merchant',
  notes: null,
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
  ...overrides,
});

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function renderManager(
  coordinator: DraftCoordinator,
  overrides: Partial<{
    campaignId: string;
    kind: 'npc' | 'location';
    openId: string | null;
  }> = {},
) {
  return render(EntityManager, {
    props: {
      campaignId: 'camp1',
      kind: 'npc',
      draftCoordinator: coordinator,
      ...overrides,
    },
  });
}

function getEntityRow(name: string): { button: HTMLElement; row: HTMLElement } {
  const button = screen.getByRole('button', { name });
  const row = button.closest('li');
  if (!row) throw new Error(`Expected ${name} to be rendered in an entity row`);
  return { button, row };
}

describe('EntityManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(commands.getEntities).mockResolvedValue([]);
  });

  it('renders New button with the correct label for the given kind', () => {
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    expect(screen.getByRole('button', { name: /new npc/i })).toBeTruthy();
  });

  it('loads NPC list on mount', async () => {
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());
    expect(commands.getEntities).toHaveBeenCalledWith('camp1', 'npc');
  });

  it('shows form when New button is clicked', async () => {
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    expect(screen.getByLabelText('Name', { exact: true })).toBeTruthy();
  });

  it('opens create form with pendingCreate name prefilled and consumes once', async () => {
    const onPendingCreateConsumed = vi.fn();
    render(EntityManager, {
      props: {
        campaignId: 'camp1',
        kind: 'location',
        pendingCreate: { kind: 'location', name: 'Moon Gate' },
        onPendingCreateConsumed,
      },
    });

    await waitFor(() => {
      expect((screen.getByLabelText(/^name$/i) as HTMLInputElement).value).toBe('Moon Gate');
    });
    expect(onPendingCreateConsumed).toHaveBeenCalledTimes(1);
  });

  it('calls onPendingCreateSaved after a Maintenance-origin pending create succeeds', async () => {
    const created = {
      ...mockNpc(),
      id: 'loc1',
      kind: 'location',
      name: 'Moon Gate',
    };
    vi.mocked(commands.createEntity).mockResolvedValue(created);
    const onPendingCreateSaved = vi.fn();
    render(EntityManager, {
      props: {
        campaignId: 'camp1',
        kind: 'location',
        pendingCreate: {
          kind: 'location',
          name: 'Moon Gate',
          sourceFindingId: 'lint_finding:1',
        },
        onPendingCreateSaved,
      },
    });

    await waitFor(() => screen.getByLabelText(/^name$/i));
    await fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => expect(commands.createEntity).toHaveBeenCalled());
    expect(onPendingCreateSaved).toHaveBeenCalledWith('lint_finding:1');
  });

  it('does not replace a dirty open form with a pending create without confirmation', async () => {
    const rendered = render(EntityManager, {
      props: { campaignId: 'camp1', kind: 'npc' },
    });
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.input(screen.getByLabelText(/^name$/i), {
      target: { value: 'Unsaved NPC' },
    });

    await rendered.rerender({
      campaignId: 'camp1',
      kind: 'npc',
      pendingCreate: { kind: 'npc', name: 'Moon Gate' },
    });

    expect((screen.getByLabelText(/^name$/i) as HTMLInputElement).value).toBe('Unsaved NPC');
    expect(screen.getByRole('dialog', { name: /discard unsaved changes/i })).toBeTruthy();
  });

  it('shows toast on DATABASE error from createEntity', async () => {
    vi.mocked(commands.createEntity).mockRejectedValue({
      code: 'DATABASE',
      message: 'disk full',
    });
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    // submit the form with a name — find the form and its name input
    const nameInput = screen.getByLabelText(/^name$/i);
    await fireEvent.input(nameInput, { target: { value: 'Test NPC' } });
    await fireEvent.submit(screen.getByRole('form'));
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
  });

  it('retains a new draft and offers retry when createEntity returns VALIDATION', async () => {
    vi.mocked(commands.createEntity)
      .mockRejectedValueOnce({
        code: 'VALIDATION',
        message: 'Too long',
        field: 'name',
      })
      .mockResolvedValueOnce(
        mockNpc({ id: 'created-npc', name: 'x', notes: 'Validation-safe local content' }),
      );
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    const nameInput = screen.getByLabelText(/^name$/i);
    await fireEvent.input(nameInput, { target: { value: 'x' } });
    await fireEvent.input(screen.getByLabelText('Notes'), {
      target: { value: 'Validation-safe local content' },
    });
    await fireEvent.submit(screen.getByRole('form'));

    const failure = await screen.findByRole('alert');
    expect(failure).toHaveTextContent("Couldn't save");
    expect(failure).toHaveTextContent('Too long');
    expect(screen.getByText('Too long', { selector: '#ef-name-error' })).toBeInTheDocument();
    expect(nameInput).toHaveValue('x');
    expect(screen.getByLabelText('Notes')).toHaveValue('Validation-safe local content');

    await fireEvent.click(within(failure).getByRole('button', { name: 'Retry' }));

    await waitFor(() => expect(commands.createEntity).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.createEntity).mock.calls[1]).toEqual([
      'camp1',
      'npc',
      expect.objectContaining({ name: 'x', notes: 'Validation-safe local content' }),
    ]);
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    expect(screen.queryByText("Couldn't save")).not.toBeInTheDocument();
  });

  it('reloads list after NOT_FOUND error', async () => {
    vi.mocked(commands.updateEntity).mockRejectedValue({
      code: 'NOT_FOUND',
      message: 'Gone',
    });
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());

    // Open edit form
    await fireEvent.click(screen.getByText('Torvin'));
    // Submit with a name
    const nameInput = screen.getByLabelText(/^name$/i);
    await fireEvent.input(nameInput, { target: { value: 'Renamed' } });
    await fireEvent.submit(screen.getByRole('form'));
    // Toast should appear with some message
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
    // getEntities is called for: initial load (1) + buildEntityMap for all 8 kinds (8) + reload after NOT_FOUND (1)
    // Just verify it was called more than the initial load to confirm reload occurred.
    expect(commands.getEntities).toHaveBeenCalledWith('camp1', 'npc');
    expect(vi.mocked(commands.getEntities).mock.calls.length).toBeGreaterThan(1);
  });

  it('updates form fields when switching from one entity to another', async () => {
    const torvin = mockNpc();
    const brakka: GraphNode = {
      ...mockNpc(),
      id: 'npc2',
      name: 'Brakka',
      summary: 'Orc chieftain',
    };
    vi.mocked(commands.getEntities).mockResolvedValue([torvin, brakka]);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => screen.getByText('Torvin'));

    // Edit Torvin
    await fireEvent.click(screen.getByText('Torvin'));
    await waitFor(() => {
      expect((screen.getByLabelText(/^name$/i) as HTMLInputElement).value).toBe('Torvin');
    });

    // Switch to Brakka — fields must follow the newly selected entity
    await fireEvent.click(screen.getByText('Brakka'));
    await waitFor(() => {
      expect((screen.getByLabelText(/^name$/i) as HTMLInputElement).value).toBe('Brakka');
    });
    expect((screen.getByLabelText(/^summary$/i) as HTMLInputElement).value).toBe('Orc chieftain');
  });

  it('opens the edit form for the entity matching openId once entities are loaded', async () => {
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc', openId: 'npc1' } });
    // The edit form should open with Torvin's name populated
    await waitFor(() => {
      const input = screen.queryByLabelText(/^name$/i) as HTMLInputElement | null;
      expect(input).toBeTruthy();
      expect(input?.value).toBe('Torvin');
    });
  });

  it('calls onOpenIdConsumed exactly once when the deep-link form opens', async () => {
    // Regression guard for Fix 1: the deep-link $effect must invoke
    // onOpenIdConsumed immediately so the caller (Shell) can clear pendingOpen
    // before any entity-list mutation (save/delete) re-triggers the effect.
    // Asserting the callback fires exactly once — and only after the form is
    // open — proves the consume-once contract is in place.
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    const onOpenIdConsumed = vi.fn();

    render(EntityManager, {
      props: { campaignId: 'camp1', kind: 'npc', openId: 'npc1', onOpenIdConsumed },
    });

    // Callback must not fire before entities load.
    expect(onOpenIdConsumed).not.toHaveBeenCalled();

    // Once entities load and the form opens, the callback fires exactly once.
    await waitFor(() => {
      expect(screen.queryByLabelText(/^name$/i)).toBeTruthy();
    });
    expect(onOpenIdConsumed).toHaveBeenCalledTimes(1);
  });

  it('form stays open with the acknowledged record after save without consuming a deep link', async () => {
    // Second half of the Fix 1 regression guard: after the caller clears
    // openId (simulated by passing null on rerender), a save that mutates the
    // entity list must NOT re-open the form.
    const updated: GraphNode = { ...mockNpc(), name: 'Torvin Updated' };
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    vi.mocked(commands.updateEntity).mockResolvedValue(updated);

    const onOpenIdConsumed = vi.fn();
    // Render without openId so the form is opened manually (not via deep-link),
    // simulating the state after Shell clears pendingOpen.
    render(EntityManager, {
      props: { campaignId: 'camp1', kind: 'npc', openId: null, onOpenIdConsumed },
    });

    // Wait for the entity list to load, then open the edit form manually.
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());
    await fireEvent.click(screen.getByText('Torvin'));
    expect(screen.queryByLabelText(/^name$/i)).toBeTruthy();

    // Save — mutates `entities` internally via reassignment.
    const nameInput = screen.getByLabelText(/^name$/i);
    await fireEvent.input(nameInput, { target: { value: 'Torvin Updated' } });
    await fireEvent.submit(screen.getByRole('form'));

    // A successful explicit save keeps the acknowledged record visible so the
    // GM can see that it is saved and continue editing without reopening it.
    await waitFor(() => {
      expect(screen.getByLabelText(/^name$/i)).toHaveValue('Torvin Updated');
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
    });
    // openId was null the whole time, so the consumed callback must never fire.
    expect(onOpenIdConsumed).not.toHaveBeenCalled();
  });

  it('calls onViewGraph with the entity when the Graph button is clicked', async () => {
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    const onViewGraph = vi.fn();
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc', onViewGraph } });
    await waitFor(() => screen.getByText('Torvin'));
    const btn = screen.getByTitle('View relationships');
    await fireEvent.click(btn);
    expect(onViewGraph).toHaveBeenCalledWith(expect.objectContaining({ id: 'npc1' }));
  });

  it('Escape closes the delete confirmation without deleting', async () => {
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => screen.getByText('Torvin'));
    await fireEvent.click(screen.getByRole('button', { name: /delete torvin/i }));
    const dialog = await screen.findByRole('dialog');
    await fireEvent.keyDown(dialog, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
    expect(commands.softDeleteEntity).not.toHaveBeenCalled();
  });

  it('confirming delete calls softDeleteEntity, not the hard-delete command', async () => {
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    vi.mocked(commands.softDeleteEntity).mockResolvedValue();
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => screen.getByText('Torvin'));
    await fireEvent.click(screen.getByRole('button', { name: /delete torvin/i }));
    await screen.findByRole('dialog');
    expect(screen.getByText(/it disappears from chronacle and your vault/i)).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: /^delete$/i }));
    await waitFor(() => expect(commands.softDeleteEntity).toHaveBeenCalledWith('npc1', 'npc'));
  });

  it('renders the codex article read-only with a stale chip', async () => {
    const node: GraphNode = {
      ...mockNpc(),
      codex_article: 'Mira runs the [[Gilded Flagon]].',
      codex_stale: true,
    };
    vi.mocked(commands.getEntities).mockResolvedValue([node]);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());
    await fireEvent.click(screen.getByText('Torvin'));

    await waitFor(() => expect(screen.getByText('Codex Article')).toBeTruthy());
    expect(screen.getByText(/Mira runs the/)).toBeTruthy();
    expect(screen.getByText('Stale')).toBeTruthy();
    expect(screen.queryByDisplayValue(/Mira runs the/)).toBeNull();
  });

  it('reports unresolved wikilink clicks from the codex article', async () => {
    const onMissingLinkClick = vi.fn();
    const node: GraphNode = {
      ...mockNpc(),
      codex_article: 'Mira knows the [[Moon Gate]].',
      codex_stale: false,
    };
    vi.mocked(commands.getEntities).mockResolvedValue([node]);
    render(EntityManager, {
      props: { campaignId: 'camp1', kind: 'npc', onMissingLinkClick },
    });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());
    await fireEvent.click(screen.getByText('Torvin'));

    await fireEvent.click(screen.getByRole('button', { name: 'Create article for Moon Gate' }));

    expect(onMissingLinkClick).toHaveBeenCalledWith('Moon Gate');
  });

  it('recompile button calls compileEntity with kind and id', async () => {
    const node: GraphNode = {
      ...mockNpc(),
      codex_article: 'Some prior article.',
      codex_stale: false,
    };
    vi.mocked(commands.getEntities).mockResolvedValue([node]);
    vi.mocked(commands.compileEntity).mockResolvedValue(true);
    vi.mocked(commands.getEntity).mockResolvedValue(node);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());
    await fireEvent.click(screen.getByText('Torvin'));

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Recompile article' })).toBeTruthy(),
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Recompile article' }));

    await waitFor(() => expect(commands.compileEntity).toHaveBeenCalledWith('npc', 'npc1'));
  });

  it('shows no-context toast when recompile finds no source', async () => {
    const node: GraphNode = {
      ...mockNpc(),
      codex_article: 'Some prior article.',
      codex_stale: false,
    };
    vi.mocked(commands.getEntities).mockResolvedValue([node]);
    vi.mocked(commands.compileEntity).mockResolvedValue(false);
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());
    await fireEvent.click(screen.getByText('Torvin'));

    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Recompile article' })).toBeTruthy(),
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Recompile article' }));

    await waitFor(() =>
      expect(screen.getByText('No source context found — article unchanged')).toBeTruthy(),
    );
  });

  describe('retained explicit drafts', () => {
    const mira = (overrides: Partial<GraphNode> = {}) =>
      mockNpc({
        id: 'mira',
        name: 'Mira',
        summary: 'Scout',
        notes: 'Saved Mira note',
        ...overrides,
      });
    const torvin = (overrides: Partial<GraphNode> = {}) =>
      mockNpc({
        id: 'torvin',
        name: 'Torvin',
        summary: 'Merchant',
        notes: 'Saved Torvin note',
        ...overrides,
      });

    beforeEach(() => {
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return [mira(), torvin()];
        return [];
      });
    });

    it('restores existing drafts after switching records and remounting the view', async () => {
      const coordinator = new DraftCoordinator();
      const first = renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      const notes = screen.getByLabelText('Notes');
      await fireEvent.input(notes, { target: { value: 'Mira knows the Moon Gate' } });
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');

      await fireEvent.click(screen.getByText('Torvin'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Saved Torvin note');
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Torvin owes three crowns' },
      });

      await fireEvent.click(screen.getByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Mira knows the Moon Gate');
      first.unmount();

      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Mira knows the Moon Gate');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(commands.updateEntity).not.toHaveBeenCalled();
    });

    it('isolates and restores entity drafts across campaign and kind changes', async () => {
      const coordinator = new DraftCoordinator();
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return [mira()];
        if (campaignId === 'camp2' && kind === 'location') {
          return [
            mockNpc({
              id: 'moon-gate',
              kind: 'location',
              campaign_id: 'camp2',
              name: 'Moon Gate',
              notes: 'Saved gate note',
            }),
          ];
        }
        return [];
      });
      const rendered = renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Mira draft in campaign one' },
      });

      await rendered.rerender({
        campaignId: 'camp2',
        kind: 'location',
        openId: null,
        draftCoordinator: coordinator,
      });
      await fireEvent.click(await screen.findByText('Moon Gate'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Saved gate note');
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Gate draft in campaign two' },
      });

      await rendered.rerender({
        campaignId: 'camp1',
        kind: 'npc',
        openId: null,
        draftCoordinator: coordinator,
      });
      await fireEvent.click(await screen.findByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Mira draft in campaign one');

      await rendered.rerender({
        campaignId: 'camp2',
        kind: 'location',
        openId: null,
        draftCoordinator: coordinator,
      });
      await fireEvent.click(await screen.findByText('Moon Gate'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Gate draft in campaign two');
    });

    it('adopts a clean canonical reload but keeps an at-risk local draft authoritative', async () => {
      const coordinator = new DraftCoordinator();
      let canonical = mira();
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return [canonical];
        return [];
      });

      const first = renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Saved Mira note');
      first.unmount();

      canonical = { ...mira(), notes: 'Reloaded canonical note' };
      const second = renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Reloaded canonical note');
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Local note wins while pending' },
      });
      second.unmount();

      canonical = { ...mira(), notes: 'Newer backend reload' };
      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Local note wins while pending');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    });

    it('keeps a save acknowledgment over a pre-ack list row but accepts a later clean reload', async () => {
      const coordinator = new DraftCoordinator();
      const updateWrite = deferred<GraphNode>();
      const laterListLoad = deferred<GraphNode[]>();
      const acknowledged = mira({
        name: 'Mira Moonshadow',
        notes: 'Canonical acknowledged content',
      });
      let listPhase: 'before-ack' | 'after-ack' = 'before-ack';
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId !== 'camp1' || kind !== 'npc') return [];
        if (listPhase === 'after-ack') return laterListLoad.promise;
        return [mira()];
      });
      vi.mocked(commands.updateEntity).mockReturnValue(updateWrite.promise);

      const managerA = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Mira' }));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision sent for saving' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));
      managerA.unmount();

      const managerB = renderManager(coordinator);
      const cachedMiraRow = await screen.findByRole('button', { name: 'Mira' });
      expect(cachedMiraRow).toHaveAccessibleDescription('Saving…');

      updateWrite.resolve(acknowledged);
      await waitFor(() => expect(cachedMiraRow).not.toHaveAccessibleDescription('Saving…'));

      expect.soft(screen.queryByRole('button', { name: 'Mira' })).not.toBeInTheDocument();
      expect.soft(screen.queryByRole('button', { name: acknowledged.name })).toBeInTheDocument();
      await fireEvent.click(cachedMiraRow);
      expect.soft(screen.getByLabelText('Name', { exact: true })).toHaveValue(acknowledged.name);
      expect.soft(screen.getByLabelText('Notes')).toHaveValue(acknowledged.notes);
      expect.soft(screen.getByText(acknowledged.notes as string, { exact: true })).toBeVisible();
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);
      managerB.unmount();

      listPhase = 'after-ack';
      renderManager(coordinator);
      laterListLoad.resolve([
        mira({
          name: acknowledged.name,
          notes: 'Authoritative content from a later list load',
        }),
      ]);

      await fireEvent.click(await screen.findByRole('button', { name: acknowledged.name }));
      expect(screen.getByLabelText('Notes')).toHaveValue(
        'Authoritative content from a later list load',
      );
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);
    });

    it('ignores a stale list response from an unmounted manager after a newer save acknowledgment', async () => {
      const coordinator = new DraftCoordinator();
      const staleManagerLoad = deferred<GraphNode[]>();
      const acknowledged = mira({
        name: 'Mira Moonshadow',
        notes: 'Canonical content from manager B',
      });
      let npcListRequests = 0;
      let persistedMira = mira();
      vi.mocked(commands.getEntities).mockImplementation((campaignId, kind) => {
        if (campaignId !== 'camp1' || kind !== 'npc') return Promise.resolve([]);
        npcListRequests += 1;
        if (npcListRequests === 1) return staleManagerLoad.promise;
        return Promise.resolve([persistedMira, torvin()]);
      });
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) => {
        expect(id).toBe('mira');
        persistedMira = { ...acknowledged, name: input.name, notes: input.notes ?? null };
        return persistedMira;
      });

      const managerA = renderManager(coordinator);
      await waitFor(() => expect(commands.getEntities).toHaveBeenCalledWith('camp1', 'npc'));
      managerA.unmount();

      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Mira' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: acknowledged.name },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: acknowledged.notes },
      });
      await fireEvent.submit(screen.getByRole('form'));

      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
      expect(screen.getByRole('button', { name: acknowledged.name })).toBeInTheDocument();
      expect(screen.getByText(acknowledged.notes as string, { exact: true })).toBeVisible();
      expect(screen.getByLabelText('Notes')).toHaveValue(acknowledged.notes);
      expect(persistedMira).toMatchObject({
        name: acknowledged.name,
        notes: acknowledged.notes,
      });
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);

      const staleLoadSettled = staleManagerLoad.promise.then(() => undefined);
      staleManagerLoad.resolve([mira(), torvin()]);
      await staleLoadSettled;

      await waitFor(() => {
        expect(coordinator.get(entityScope('camp1', 'npc', 'mira'))).toMatchObject({
          baseline: expect.objectContaining({
            name: acknowledged.name,
            notes: acknowledged.notes,
          }),
          value: expect.objectContaining({
            name: acknowledged.name,
            notes: acknowledged.notes,
          }),
        });
      });
      expect(screen.getByRole('button', { name: acknowledged.name })).toBeInTheDocument();
      expect(screen.getByText(acknowledged.notes as string, { exact: true })).toBeVisible();
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue(acknowledged.name);
      expect(screen.getByLabelText('Notes')).toHaveValue(acknowledged.notes);
      expect(screen.getByRole('status')).toHaveTextContent('Saved');

      await fireEvent.click(screen.getByRole('button', { name: 'Torvin' }));
      await fireEvent.click(screen.getByRole('button', { name: acknowledged.name }));
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue(acknowledged.name);
      expect(screen.getByLabelText('Notes')).toHaveValue(acknowledged.notes);
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(persistedMira).toMatchObject({
        name: acknowledged.name,
        notes: acknowledged.notes,
      });
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);
    });

    it('restores a stable new-record draft without creating a record during navigation', async () => {
      const coordinator = new DraftCoordinator();
      const first = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Carries an unfinished map' },
      });
      first.unmount();

      renderManager(coordinator);
      expect(await screen.findByLabelText('Name', { exact: true })).toHaveValue('Captain Sable');
      expect(screen.getByLabelText('Notes')).toHaveValue('Carries an unfinished map');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(commands.createEntity).not.toHaveBeenCalled();
    });

    it('clears pending state when an edit returns to the saved baseline without saving', async () => {
      const coordinator = new DraftCoordinator();
      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      const notes = screen.getByLabelText('Notes');

      await fireEvent.input(notes, { target: { value: 'Changed locally' } });
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      await fireEvent.input(notes, { target: { value: 'Saved Mira note' } });

      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(screen.queryByText('Unsaved changes')).not.toBeInTheDocument();
      expect(commands.updateEntity).not.toHaveBeenCalled();
    });

    it('keeps a NOT_FOUND edit inline and retries against the same entity', async () => {
      const coordinator = new DraftCoordinator();
      vi.mocked(commands.updateEntity)
        .mockRejectedValueOnce({ code: 'NOT_FOUND', message: 'Gone' })
        .mockResolvedValueOnce(mira());
      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Evidence survives deletion' },
      });
      await fireEvent.submit(screen.getByRole('form'));

      const failure = await screen.findByRole('alert');
      expect(failure).toHaveTextContent("Couldn't save");
      expect(failure).toHaveTextContent('This record is no longer available.');
      expect(screen.getByLabelText('Notes')).toHaveValue('Evidence survives deletion');
      await fireEvent.click(within(failure).getByRole('button', { name: 'Retry' }));

      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(2));
      expect(vi.mocked(commands.updateEntity).mock.calls[1]?.[0]).toBe('mira');
      expect(vi.mocked(commands.updateEntity).mock.calls[1]?.[2]).toMatchObject({
        notes: 'Evidence survives deletion',
      });
      expect(screen.queryByText("Couldn't save")).not.toBeInTheDocument();
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
    });

    it('serializes rapid explicit saves and preserves a newer edit after an older acknowledgment', async () => {
      const coordinator = new DraftCoordinator();
      const firstWrite = deferred<GraphNode>();
      const secondWrite = deferred<GraphNode>();
      let activeWrites = 0;
      let maxActiveWrites = 0;
      vi.mocked(commands.updateEntity).mockImplementation(async (_id, _kind, input) => {
        activeWrites += 1;
        maxActiveWrites = Math.max(maxActiveWrites, activeWrites);
        const result =
          vi.mocked(commands.updateEntity).mock.calls.length === 1
            ? await firstWrite.promise
            : await secondWrite.promise;
        activeWrites -= 1;
        return { ...result, notes: input.notes ?? null };
      });
      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      const notes = screen.getByLabelText('Notes');
      await fireEvent.input(notes, { target: { value: 'Revision one' } });
      await fireEvent.submit(screen.getByRole('form'));
      expect(await screen.findByRole('status')).toHaveTextContent('Saving…');

      await fireEvent.input(notes, { target: { value: 'Revision two' } });
      await fireEvent.submit(screen.getByRole('form'));
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);
      expect(screen.getByLabelText('Notes')).toHaveValue('Revision two');

      firstWrite.resolve({ ...mira(), notes: 'Revision one' });
      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(2));
      expect(maxActiveWrites).toBe(1);
      expect(screen.getByLabelText('Notes')).toHaveValue('Revision two');
      expect(screen.getByRole('status')).not.toHaveTextContent('Saved');

      secondWrite.resolve({ ...mira(), notes: 'Revision two' });
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    });

    it('maps a successful Create to the returned record and keeps the acknowledged form open', async () => {
      const coordinator = new DraftCoordinator();
      const created = mockNpc({
        id: 'sable',
        name: 'Captain Sable',
        notes: 'Canonical map notes',
      });
      vi.mocked(commands.createEntity).mockResolvedValue(created);
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Draft map notes' },
      });
      await fireEvent.submit(screen.getByRole('form'));

      await waitFor(() => expect(commands.createEntity).toHaveBeenCalledTimes(1));
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Captain Sable');
      expect(screen.getByLabelText('Notes')).toHaveValue('Canonical map notes');
      expect(screen.getByRole('status')).toHaveTextContent('Saved');

      await fireEvent.click(screen.getByText('Torvin'));
      await fireEvent.click(screen.getByText('Captain Sable'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Canonical map notes');
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
    });

    it('promotes an acknowledged Create to the returned ID without losing a newer revision', async () => {
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) =>
        mockNpc({ id, name: input.name, notes: input.notes }),
      );
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision one' },
      });

      await fireEvent.submit(screen.getByRole('form'));
      const createAction = screen.getByTestId('entity-form-submit');
      await waitFor(() => expect(createAction).toBeDisabled());
      expect(createAction).toHaveTextContent('Create');

      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision two while creating' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      expect(commands.createEntity).toHaveBeenCalledTimes(1);

      createWrite.resolve(
        mockNpc({ id: 'npc-returned-42', name: 'Captain Sable', notes: 'Revision one' }),
      );

      await waitFor(() => {
        expect(screen.getByTestId('entity-form-submit')).toBeEnabled();
        expect(screen.getByTestId('entity-form-submit')).toHaveTextContent('Save');
      });
      expect(screen.getByLabelText('Notes')).toHaveValue('Revision two while creating');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(commands.createEntity).toHaveBeenCalledTimes(1);

      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(1));
      expect(commands.updateEntity).toHaveBeenCalledWith(
        'npc-returned-42',
        'npc',
        expect.objectContaining({
          name: 'Captain Sable',
          notes: 'Revision two while creating',
        }),
      );
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    });

    it('follows a remounted Create to its returned ID and preserves a newer revision', async () => {
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) =>
        mockNpc({ id, name: input.name, notes: input.notes }),
      );

      const first = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision one' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));

      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision two while away' },
      });
      first.unmount();

      renderManager(coordinator);
      expect(await screen.findByLabelText('Name', { exact: true })).toHaveValue('Captain Sable');
      expect(screen.getByLabelText('Notes')).toHaveValue('Revision two while away');
      expect(screen.getByRole('status')).toHaveTextContent('Saving…');

      createWrite.resolve(
        mockNpc({ id: 'npc-returned-after-nav', name: 'Captain Sable', notes: 'Revision one' }),
      );

      await waitFor(() => {
        expect(screen.getByTestId('entity-form-submit')).toHaveTextContent('Save');
        expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      });
      expect(screen.getByLabelText('Notes')).toHaveValue('Revision two while away');
      const acknowledgedRow = getEntityRow('Captain Sable');
      expect(acknowledgedRow.row).not.toHaveTextContent('This record is no longer available.');

      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(1));
      expect(commands.updateEntity).toHaveBeenCalledWith(
        'npc-returned-after-nav',
        'npc',
        expect.objectContaining({
          name: 'Captain Sable',
          notes: 'Revision two while away',
        }),
      );
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    });

    it('promotes a remounted Create when its committed record was reconciled before acknowledgment', async () => {
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      const returned = mockNpc({
        id: 'npc-committed-before-ack',
        name: 'Captain Sable',
        notes: 'Canonical revision one',
      });
      let backendEntities = [mira(), torvin()];
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return backendEntities;
        return [];
      });
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) =>
        mockNpc({ id, name: input.name, notes: input.notes }),
      );

      const managerA = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision one' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision two while acknowledgment is delayed' },
      });

      backendEntities = [returned, mira(), torvin()];
      managerA.unmount();
      renderManager(coordinator);
      expect(await screen.findByLabelText('Notes')).toHaveValue(
        'Revision two while acknowledgment is delayed',
      );
      expect(getEntityRow('Captain Sable').row).toBeInTheDocument();
      expect(screen.getByRole('status')).toHaveTextContent('Saving…');

      createWrite.resolve(returned);

      await waitFor(() => {
        expect(screen.getByTestId('entity-form-submit')).toHaveTextContent('Save');
        expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      });
      expect(screen.getByLabelText('Notes')).toHaveValue(
        'Revision two while acknowledgment is delayed',
      );
      expect(commands.createEntity).toHaveBeenCalledTimes(1);

      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(1));
      expect(commands.updateEntity).toHaveBeenCalledWith(
        'npc-committed-before-ack',
        'npc',
        expect.objectContaining({
          name: 'Captain Sable',
          notes: 'Revision two while acknowledgment is delayed',
        }),
      );
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    });

    it('preserves a newer acknowledged destination when an older Create acknowledgment arrives', async () => {
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      const returned = mockNpc({
        id: 'npc-created-before-ack',
        name: 'Captain Sable',
        notes: 'Create revision one',
      });
      let backendEntities = [mira(), torvin()];
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return backendEntities;
        return [];
      });
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) => {
        const canonical = mockNpc({ id, name: input.name, notes: input.notes });
        backendEntities = [canonical, ...backendEntities.filter((entity) => entity.id !== id)];
        return canonical;
      });

      const managerA = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Create revision one' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));

      const originalSourceScope = coordinator.listByPrefix('entity-new:camp1:npc:')[0]?.scope;
      if (!originalSourceScope) throw new Error('Expected the in-flight Create draft');

      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Source revision retained through conflict' },
      });

      // The committed Create is visible through a remounted list while its
      // older acknowledgment remains withheld from the initiating editor.
      backendEntities = [returned, mira(), torvin()];
      managerA.unmount();
      renderManager(coordinator);
      const assignedRow = await screen.findByRole('button', { name: 'Captain Sable' });
      await fireEvent.click(assignedRow);
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Destination revision acknowledged later' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));

      const destinationScope = entityScope('camp1', 'npc', 'npc-created-before-ack');
      const newerDestination = coordinator.get(destinationScope);
      expect(newerDestination).toMatchObject({
        value: expect.objectContaining({ notes: 'Destination revision acknowledged later' }),
      });
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);

      createWrite.resolve(returned);

      await waitFor(() => {
        const retainedSource = coordinator.get(originalSourceScope);
        expect(
          retainedSource?.inFlight === null ||
            coordinator.resolveScope(originalSourceScope) !== originalSourceScope,
        ).toBe(true);
      });
      await waitFor(() =>
        expect(screen.getByLabelText('Notes')).toHaveValue(
          'Destination revision acknowledged later',
        ),
      );
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(coordinator.get(destinationScope)).toBe(newerDestination);
      expect(coordinator.get(destinationScope)).toMatchObject({ lastAcknowledgedAttemptId: 2 });
      expect(backendEntities.find(({ id }) => id === returned.id)?.notes).toBe(
        'Destination revision acknowledged later',
      );

      const sourceDrafts = coordinator.listByPrefix('entity-new:camp1:npc:');
      expect(sourceDrafts).toHaveLength(1);
      const sourceScope = sourceDrafts[0]?.scope;
      if (!sourceScope) throw new Error('Expected the original Create draft to remain available');
      expect(coordinator.get(sourceScope)?.value).toMatchObject({
        notes: 'Source revision retained through conflict',
      });
      expect(coordinator.getCreatePromotionIssue(sourceScope)).toMatchObject({
        destinationScope,
        reason: 'destination-newer-acknowledgment',
      });
      const matchingRows = screen.getAllByRole('button', { name: 'Captain Sable' });
      expect(matchingRows).toHaveLength(2);
      const sourceRow = matchingRows[1];
      if (!sourceRow) throw new Error('Expected a row for the retained Create draft');
      await fireEvent.click(sourceRow);
      const conflict = await screen.findByRole('alert');
      expect(conflict).toHaveTextContent('Created, but needs attention');
      expect(screen.getByLabelText('Notes')).toHaveValue(
        'Source revision retained through conflict',
      );
      expect(within(conflict).getByRole('button', { name: 'Keep saved record' })).toBeEnabled();
      expect(within(conflict).getByRole('button', { name: 'Keep my draft' })).toBeEnabled();
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);

      const user = userEvent.setup();
      const keepSavedRecord = within(conflict).getByRole('button', {
        name: 'Keep saved record',
      });
      keepSavedRecord.focus();
      await user.keyboard('{Enter}');

      await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
      expect(screen.getAllByRole('button', { name: 'Captain Sable' })).toHaveLength(1);
      expect(screen.getByLabelText('Notes')).toHaveValue('Destination revision acknowledged later');
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(coordinator.listByPrefix('entity-new:camp1:npc:')).toHaveLength(0);
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);
      expect(screen.getByTestId('entity-form-submit')).toHaveFocus();
      expect(document.activeElement).not.toBe(document.body);
    });

    it('keeps destination presentation authoritative when a settled Create acknowledges last', async () => {
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      const createRevision = mockNpc({
        id: 'npc-created-before-ack',
        name: 'Captain Sable',
        notes: 'Create revision one',
      });
      const destinationRevision = mockNpc({
        id: createRevision.id,
        name: 'Admiral Sable',
        notes: 'Destination revision saved after the record appeared',
      });
      let backendEntities = [mira(), torvin()];
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return backendEntities;
        return [];
      });
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) => {
        const canonical = mockNpc({ id, name: input.name, notes: input.notes });
        backendEntities = [canonical, ...backendEntities.filter((entity) => entity.id !== id)];
        return canonical;
      });

      const rendered = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: createRevision.name },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: createRevision.notes },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));

      // The committed record becomes visible through a remounted list while
      // the initiating Create acknowledgment remains withheld. There is no
      // later source revision, so the coordinator may safely converge on the
      // subsequently acknowledged destination.
      backendEntities = [createRevision, mira(), torvin()];
      await rendered.rerender({
        campaignId: 'camp2',
        kind: 'npc',
        openId: null,
        draftCoordinator: coordinator,
      });
      await rendered.rerender({
        campaignId: 'camp1',
        kind: 'npc',
        openId: null,
        draftCoordinator: coordinator,
      });

      await fireEvent.click(await screen.findByRole('button', { name: createRevision.name }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: destinationRevision.name },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: destinationRevision.notes },
      });
      await fireEvent.submit(screen.getByRole('form'));

      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
      expect(getEntityRow(destinationRevision.name).row).toBeInTheDocument();
      expect(screen.getByText(destinationRevision.notes as string, { exact: true })).toBeVisible();
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);

      createWrite.resolve(createRevision);

      await waitFor(() =>
        expect(coordinator.listByPrefix('entity-new:camp1:npc:')).toHaveLength(0),
      );
      expect(screen.queryByRole('button', { name: createRevision.name })).not.toBeInTheDocument();
      const destinationRow = getEntityRow(destinationRevision.name);
      expect(destinationRow.row).toHaveClass('selected');
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue(destinationRevision.name);
      expect(screen.getByLabelText('Notes')).toHaveValue(destinationRevision.notes);
      expect(screen.getByText(destinationRevision.notes as string, { exact: true })).toBeVisible();
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(backendEntities.filter(({ id }) => id === createRevision.id)).toEqual([
        destinationRevision,
      ]);
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);

      await fireEvent.click(screen.getByRole('button', { name: 'Torvin' }));
      await fireEvent.click(screen.getByRole('button', { name: destinationRevision.name }));
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue(destinationRevision.name);
      expect(screen.getByLabelText('Notes')).toHaveValue(destinationRevision.notes);
      expect(screen.getByRole('status')).toHaveTextContent('Saved');
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);
    });

    it('keeps my pending Create draft without writing until an explicit Update save', async () => {
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      const returned = mockNpc({
        id: 'npc-keep-my-draft',
        name: 'Captain Sable',
        notes: 'Create revision one',
      });
      let backendEntities = [mira(), torvin()];
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return backendEntities;
        return [];
      });
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) => {
        const canonical = mockNpc({ id, name: input.name, notes: input.notes });
        backendEntities = [canonical, ...backendEntities.filter((entity) => entity.id !== id)];
        return canonical;
      });

      const managerA = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Create revision one' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable, Unwritten' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Pending source revision two' },
      });

      backendEntities = [returned, mira(), torvin()];
      managerA.unmount();
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Captain Sable' }));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Newer saved destination' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);

      createWrite.resolve(returned);
      await waitFor(() => {
        expect(screen.getAllByRole('button', { name: 'Captain Sable' })).toHaveLength(1);
        expect(screen.getAllByRole('button', { name: 'Captain Sable, Unwritten' })).toHaveLength(1);
      });
      const sourceRow = screen.getByRole('button', { name: 'Captain Sable, Unwritten' });
      await fireEvent.click(sourceRow);
      const conflict = await screen.findByRole('alert');
      expect(screen.getByLabelText('Notes')).toHaveValue('Pending source revision two');

      const user = userEvent.setup();
      const keepMyDraft = within(conflict).getByRole('button', { name: 'Keep my draft' });
      keepMyDraft.focus();
      await user.keyboard('{Enter}');

      await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
      const selectedRow = document.querySelector('li.entity-row.selected');
      if (!selectedRow) throw new Error('Expected the promoted destination row to stay selected');
      expect.soft(screen.queryByRole('button', { name: 'Captain Sable' })).not.toBeInTheDocument();
      expect.soft(selectedRow).toHaveTextContent('Captain Sable, Unwritten');
      expect
        .soft(screen.getByLabelText('Name', { exact: true }))
        .toHaveValue('Captain Sable, Unwritten');
      expect.soft(screen.getByLabelText('Notes')).toHaveValue('Pending source revision two');
      expect.soft(screen.getByText('Pending source revision two', { exact: true })).toBeVisible();
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(screen.getByTestId('entity-form-submit')).toHaveTextContent('Save');
      expect(backendEntities.find(({ id }) => id === returned.id)).toMatchObject({
        name: 'Captain Sable',
        notes: 'Newer saved destination',
      });
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).toHaveBeenCalledTimes(1);
      expect(screen.getByTestId('entity-form-submit')).toHaveFocus();
      expect(document.activeElement).not.toBe(document.body);

      await fireEvent.click(screen.getByTestId('entity-form-submit'));

      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(2));
      expect(vi.mocked(commands.updateEntity).mock.calls[1]).toEqual([
        'npc-keep-my-draft',
        'npc',
        expect.objectContaining({
          name: 'Captain Sable, Unwritten',
          notes: 'Pending source revision two',
        }),
      ]);
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    });

    it('keeps Create work and exposes an action when promotion meets an at-risk destination', async () => {
      const user = userEvent.setup();
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      const returned = mockNpc({
        id: 'npc-promotion-conflict',
        name: 'Captain Sable',
        notes: 'Canonical revision one',
      });
      let backendEntities = [mira(), torvin()];
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') return backendEntities;
        return [];
      });
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      vi.mocked(commands.updateEntity).mockImplementation(async (id, _kind, input) =>
        mockNpc({ id, name: input.name, notes: input.notes }),
      );

      const managerA = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Source revision retained through conflict' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Newer source revision retained through conflict' },
      });

      backendEntities = [returned, mira(), torvin()];
      managerA.unmount();
      renderManager(coordinator);
      expect(await screen.findByLabelText('Notes')).toHaveValue(
        'Newer source revision retained through conflict',
      );
      await waitFor(() => expect(getEntityRow('Captain Sable').row).toBeInTheDocument());
      const destinationScope = entityScope('camp1', 'npc', 'npc-promotion-conflict');
      const destination = coordinator.get<{ notes: string }>(destinationScope);
      if (!destination) throw new Error('Expected list reconciliation to open the destination');
      coordinator.revise(destinationScope, {
        ...destination.value,
        notes: 'Independent at-risk destination revision',
      });

      createWrite.resolve(returned);

      const conflict = await screen.findByRole('alert');
      expect(screen.getByLabelText('Notes')).toHaveValue(
        'Newer source revision retained through conflict',
      );
      expect(within(conflict).getByRole('button')).toBeEnabled();
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).not.toHaveBeenCalled();

      const sourceScope = coordinator.listByPrefix('entity-new:camp1:npc:')[0]?.scope;
      if (!sourceScope) throw new Error('Expected the blocked Create draft to remain available');
      const sourceBeforeRetry = coordinator.get(sourceScope);
      const destinationBeforeRetry = coordinator.get(destinationScope);

      const stillBlockedRetry = within(conflict).getByRole('button', { name: 'Retry' });
      stillBlockedRetry.focus();
      await user.keyboard('{Enter}');

      expect(screen.getByRole('alert')).toHaveTextContent('Created, but needs attention');
      expect(stillBlockedRetry).toHaveFocus();
      expect(coordinator.get(sourceScope)).toBe(sourceBeforeRetry);
      expect(coordinator.get(destinationScope)).toBe(destinationBeforeRetry);
      expect(coordinator.resolveScope(sourceScope)).toBe(sourceScope);
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).not.toHaveBeenCalled();

      expect(coordinator.discard(destinationScope)).toBe('discarded');
      const successfulRetry = screen.getByRole('button', { name: 'Retry' });
      successfulRetry.focus();
      await user.keyboard('{Enter}');

      await waitFor(() => {
        expect(screen.queryByText('Created, but needs attention')).not.toBeInTheDocument();
        expect(screen.getByTestId('entity-form-submit')).toHaveTextContent('Save');
      });
      expect(screen.getByTestId('entity-form-submit')).toHaveFocus();
      expect(screen.getByLabelText('Notes')).toHaveValue(
        'Newer source revision retained through conflict',
      );
      expect(coordinator.resolveScope(sourceScope)).toBe(destinationScope);
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).not.toHaveBeenCalled();

      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Later revision after local promotion' },
      });
      await fireEvent.submit(screen.getByRole('form'));

      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(1));
      expect(commands.updateEntity).toHaveBeenCalledWith(
        'npc-promotion-conflict',
        'npc',
        expect.objectContaining({ notes: 'Later revision after local promotion' }),
      );
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
    });

    it('shows a remounted Create as Saved when its only revision is acknowledged', async () => {
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);

      const first = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision one' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));
      first.unmount();

      renderManager(coordinator);
      expect(await screen.findByLabelText('Notes')).toHaveValue('Revision one');

      createWrite.resolve(
        mockNpc({
          id: 'npc-clean-after-nav',
          name: 'Captain Sable',
          notes: 'Canonical revision one',
        }),
      );

      await waitFor(() => {
        expect(screen.getByTestId('entity-form-submit')).toHaveTextContent('Save');
        expect(screen.getByRole('status')).toHaveTextContent('Saved');
      });
      expect(screen.getByLabelText('Notes')).toHaveValue('Canonical revision one');
      const acknowledgedRow = getEntityRow('Captain Sable');
      expect(acknowledgedRow.row).not.toHaveTextContent('This record is no longer available.');
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(commands.updateEntity).not.toHaveBeenCalled();
    });

    it('blocks replacing an active Create until it settles without discarding or duplicating work', async () => {
      const user = userEvent.setup();
      const coordinator = new DraftCoordinator();
      const createWrite = deferred<GraphNode>();
      const onPendingCreateConsumed = vi.fn();
      vi.mocked(commands.createEntity).mockReturnValue(createWrite.promise);
      const rendered = render(EntityManager, {
        props: {
          campaignId: 'camp1',
          kind: 'npc',
          draftCoordinator: coordinator,
          onPendingCreateConsumed,
        },
      });

      await user.click(await screen.findByRole('button', { name: 'New NPC' }));
      await user.type(screen.getByLabelText('Name', { exact: true }), 'Captain Sable');
      await user.type(screen.getByLabelText('Notes'), 'Create still in progress');
      await user.click(screen.getByTestId('entity-form-submit'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));
      expect(commands.createEntity).toHaveBeenCalledTimes(1);

      await user.click(screen.getByRole('button', { name: 'Mira' }));
      await rendered.rerender({
        campaignId: 'camp1',
        kind: 'npc',
        openId: null,
        pendingCreate: { kind: 'npc', name: 'Aldric' },
        onPendingCreateConsumed,
        draftCoordinator: coordinator,
      });

      const dialog = await screen.findByRole('dialog', { name: 'Discard unsaved changes?' });
      const discardAndCreate = within(dialog).getByRole('button', {
        name: 'Discard and create',
      });
      const keepEditing = within(dialog).getByRole('button', { name: 'Keep editing' });
      expect(discardAndCreate).toBeDisabled();
      expect
        .soft(discardAndCreate)
        .toHaveAttribute('aria-describedby', 'entity-new-discard-blocked');
      expect(discardAndCreate).toHaveAccessibleDescription(
        'Wait for saving to finish before discarding changes',
      );
      expect(keepEditing).toHaveFocus();

      await fireEvent.click(discardAndCreate);
      expect(dialog).toBeInTheDocument();
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Mira');
      expect(commands.createEntity).toHaveBeenCalledTimes(1);

      createWrite.resolve(
        mockNpc({
          id: 'captain-sable',
          name: 'Captain Sable',
          notes: 'Create still in progress',
        }),
      );
      await waitFor(() => expect(discardAndCreate).toBeEnabled());
      expect(coordinator.get(entityScope('camp1', 'npc', 'captain-sable'))).toMatchObject({
        value: expect.objectContaining({
          name: 'Captain Sable',
          notes: 'Create still in progress',
        }),
      });

      await user.click(discardAndCreate);

      await waitFor(() => expect(dialog).not.toBeInTheDocument());
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Aldric');
      expect(screen.getByLabelText('Notes')).toHaveValue('');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(commands.createEntity).toHaveBeenCalledTimes(1);
      expect(coordinator.get(entityScope('camp1', 'npc', 'captain-sable'))).toMatchObject({
        value: expect.objectContaining({ name: 'Captain Sable' }),
      });
    });

    it('reopens a hidden dirty new draft when create-from-link chooses Keep editing', async () => {
      const coordinator = new DraftCoordinator();
      const rendered = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Carries an unfinished map' },
      });
      await fireEvent.click(screen.getByRole('button', { name: 'Mira' }));
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Mira');

      await rendered.rerender({
        campaignId: 'camp1',
        kind: 'npc',
        openId: null,
        pendingCreate: { kind: 'npc', name: 'Aldric' },
        draftCoordinator: coordinator,
      });

      const dialog = await screen.findByRole('dialog', { name: 'Discard unsaved changes?' });
      await fireEvent.click(within(dialog).getByRole('button', { name: 'Keep editing' }));
      await waitFor(() => expect(dialog).not.toBeInTheDocument());
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Captain Sable');
      expect(screen.getByLabelText('Notes')).toHaveValue('Carries an unfinished map');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(commands.createEntity).not.toHaveBeenCalled();
    });

    it('initially focuses the safe Keep editing action in create-from-link confirmation', async () => {
      const user = userEvent.setup();
      const coordinator = new DraftCoordinator();
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') {
          return [mira({ codex_article: 'The records mention [[Aldric]].' }), torvin()];
        }
        return [];
      });
      const onMissingLinkClick = vi.fn();
      const rendered = render(EntityManager, {
        props: {
          campaignId: 'camp1',
          kind: 'npc',
          draftCoordinator: coordinator,
          onMissingLinkClick,
        },
      });
      await user.click(await screen.findByRole('button', { name: 'New NPC' }));
      await user.type(screen.getByLabelText('Name', { exact: true }), 'Captain Sable');
      await user.type(screen.getByLabelText('Notes'), 'Carries an unfinished map');
      await user.click(screen.getByRole('button', { name: 'Mira' }));

      await user.click(await screen.findByRole('button', { name: 'Create article for Aldric' }));
      expect(onMissingLinkClick).toHaveBeenCalledWith('Aldric');
      await rendered.rerender({
        campaignId: 'camp1',
        kind: 'npc',
        openId: null,
        pendingCreate: { kind: 'npc', name: 'Aldric' },
        onMissingLinkClick,
        draftCoordinator: coordinator,
      });

      const dialog = await screen.findByRole('dialog', { name: 'Discard unsaved changes?' });
      expect(within(dialog).getByRole('button', { name: 'Keep editing' })).toHaveFocus();
      expect(within(dialog).getByRole('button', { name: 'Discard and create' })).not.toHaveFocus();
    });

    it('cancels create-from-link confirmation with Escape and restores invoking control focus', async () => {
      const user = userEvent.setup();
      const coordinator = new DraftCoordinator();
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId === 'camp1' && kind === 'npc') {
          return [mira({ codex_article: 'The records mention [[Aldric]].' }), torvin()];
        }
        return [];
      });
      const onMissingLinkClick = vi.fn();
      const rendered = render(EntityManager, {
        props: {
          campaignId: 'camp1',
          kind: 'npc',
          draftCoordinator: coordinator,
          onMissingLinkClick,
        },
      });
      await user.click(await screen.findByRole('button', { name: 'New NPC' }));
      await user.type(screen.getByLabelText('Name', { exact: true }), 'Captain Sable');
      await user.type(screen.getByLabelText('Notes'), 'Carries an unfinished map');
      await user.click(screen.getByRole('button', { name: 'Mira' }));
      const invokingCreate = await screen.findByRole('button', {
        name: 'Create article for Aldric',
      });
      await user.click(invokingCreate);
      expect(onMissingLinkClick).toHaveBeenCalledWith('Aldric');
      await rendered.rerender({
        campaignId: 'camp1',
        kind: 'npc',
        openId: null,
        pendingCreate: { kind: 'npc', name: 'Aldric' },
        onMissingLinkClick,
        draftCoordinator: coordinator,
      });
      const dialog = await screen.findByRole('dialog', { name: 'Discard unsaved changes?' });

      await user.keyboard('{Escape}');

      await waitFor(() => expect(dialog).not.toBeInTheDocument());
      expect(invokingCreate).toHaveFocus();
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Mira');
      expect(commands.createEntity).not.toHaveBeenCalled();
    });

    it('replaces only a hidden dirty new draft after explicit create-from-link discard', async () => {
      const coordinator = new DraftCoordinator();
      coordinator.open(oracleScope('camp1'), null, '');
      coordinator.revise(oracleScope('camp1'), 'Unsent Oracle question');
      const rendered = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Draft that will be discarded' },
      });
      await fireEvent.click(screen.getByRole('button', { name: 'Mira' }));

      await rendered.rerender({
        campaignId: 'camp1',
        kind: 'npc',
        openId: null,
        pendingCreate: { kind: 'npc', name: 'Aldric' },
        draftCoordinator: coordinator,
      });

      const dialog = await screen.findByRole('dialog', { name: 'Discard unsaved changes?' });
      await fireEvent.click(within(dialog).getByRole('button', { name: 'Discard and create' }));
      await waitFor(() => expect(dialog).not.toBeInTheDocument());
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Aldric');
      expect(screen.getByLabelText('Notes')).toHaveValue('');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(commands.createEntity).not.toHaveBeenCalled();
      expect(coordinator.get<string>(oracleScope('camp1'))?.value).toBe('Unsent Oracle question');

      await fireEvent.click(screen.getByRole('button', { name: 'Mira' }));
      await fireEvent.click(screen.getByRole('button', { name: 'New NPC' }));
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Aldric');
      expect(screen.getByLabelText('Notes')).toHaveValue('');
    });

    it('keeps an unavailable failed draft discoverable after reload and retries its original ID', async () => {
      const coordinator = new DraftCoordinator();
      let miraExists = true;
      vi.mocked(commands.getEntities).mockImplementation(async (campaignId, kind) => {
        if (campaignId !== 'camp1' || kind !== 'npc') return [];
        return miraExists ? [mira(), torvin()] : [torvin()];
      });
      vi.mocked(commands.updateEntity)
        .mockImplementationOnce(async () => {
          miraExists = false;
          throw { code: 'NOT_FOUND', message: 'Mira was removed' };
        })
        .mockResolvedValueOnce(mira({ notes: 'Evidence survives deletion' }));

      const first = renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Mira' }));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Evidence survives deletion' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      expect(await screen.findByRole('alert')).toHaveTextContent(
        'This record is no longer available.',
      );
      first.unmount();

      renderManager(coordinator);
      const unavailable = await screen.findByRole('button', { name: 'Mira' });
      const unavailableRow = getEntityRow('Mira').row;
      expect(unavailableRow).toHaveTextContent("Couldn't save");
      expect(unavailableRow).toHaveTextContent('This record is no longer available.');
      expect(unavailable).toHaveAccessibleDescription(/couldn't save/i);

      await fireEvent.click(unavailable);
      expect(screen.getByLabelText('Notes')).toHaveValue('Evidence survives deletion');
      const failure = await screen.findByRole('alert');
      expect(failure).toHaveTextContent('This record is no longer available.');
      await fireEvent.click(within(failure).getByRole('button', { name: 'Retry' }));

      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(2));
      expect(vi.mocked(commands.updateEntity).mock.calls[1]?.[0]).toBe('mira');
      expect(vi.mocked(commands.updateEntity).mock.calls[1]?.[2]).toMatchObject({
        notes: 'Evidence survives deletion',
      });
      expect(commands.createEntity).not.toHaveBeenCalled();
    });

    it('announces pending, saving, and failed states from a closed existing-entity row', async () => {
      const coordinator = new DraftCoordinator();
      const write = deferred<GraphNode>();
      vi.mocked(commands.updateEntity).mockReturnValue(write.promise);
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Mira' }));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Mira row state remains visible' },
      });
      await fireEvent.click(screen.getByRole('button', { name: 'Torvin' }));

      expect(getEntityRow('Mira').row).toHaveTextContent('Unsaved changes');
      expect(getEntityRow('Mira').button).toHaveAccessibleDescription('Unsaved changes');

      await fireEvent.click(getEntityRow('Mira').button);
      await fireEvent.submit(screen.getByRole('form'));
      await fireEvent.click(screen.getByRole('button', { name: 'Torvin' }));
      await waitFor(() =>
        expect(getEntityRow('Mira').button).toHaveAccessibleDescription('Saving…'),
      );

      write.reject({ code: 'DATABASE', message: 'disk full' });
      await waitFor(() => {
        expect(getEntityRow('Mira').row).toHaveTextContent("Couldn't save");
        expect(getEntityRow('Mira').button).toHaveAccessibleDescription("Couldn't save");
      });
    });

    it('announces pending, saving, and failed states from a closed new-draft row', async () => {
      const coordinator = new DraftCoordinator();
      const write = deferred<GraphNode>();
      vi.mocked(commands.createEntity).mockReturnValue(write.promise);
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'New NPC' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: 'Captain Sable' },
      });
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Retained new-record row' },
      });
      await fireEvent.click(screen.getByRole('button', { name: 'Mira' }));

      expect(getEntityRow('Captain Sable').row).toHaveTextContent('Unsaved changes');
      expect(getEntityRow('Captain Sable').button).toHaveAccessibleDescription('Unsaved changes');

      await fireEvent.click(getEntityRow('Captain Sable').button);
      await fireEvent.submit(screen.getByRole('form'));
      await fireEvent.click(screen.getByRole('button', { name: 'Mira' }));
      await waitFor(() =>
        expect(getEntityRow('Captain Sable').button).toHaveAccessibleDescription('Saving…'),
      );

      write.reject({ code: 'DATABASE', message: 'disk full' });
      await waitFor(() => {
        expect(getEntityRow('Captain Sable').row).toHaveTextContent("Couldn't save");
        expect(getEntityRow('Captain Sable').button).toHaveAccessibleDescription("Couldn't save");
      });
    });

    it('keeps backend VALIDATION as an actionable failed save on the same target', async () => {
      const coordinator = new DraftCoordinator();
      vi.mocked(commands.updateEntity)
        .mockRejectedValueOnce({
          code: 'VALIDATION',
          message: 'That name conflicts with an existing NPC.',
          field: 'name',
        })
        .mockResolvedValueOnce(mira({ notes: 'Validation-safe local content' }));
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Mira' }));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Validation-safe local content' },
      });
      await fireEvent.submit(screen.getByRole('form'));

      const failure = await screen.findByRole('alert');
      expect(failure).toHaveTextContent("Couldn't save");
      expect(failure).toHaveTextContent('That name conflicts with an existing NPC.');
      expect(screen.getByLabelText('Notes')).toHaveValue('Validation-safe local content');
      await fireEvent.click(within(failure).getByRole('button', { name: 'Retry' }));

      await waitFor(() => expect(commands.updateEntity).toHaveBeenCalledTimes(2));
      expect(vi.mocked(commands.updateEntity).mock.calls[1]?.[0]).toBe('mira');
      expect(vi.mocked(commands.updateEntity).mock.calls[1]?.[2]).toMatchObject({
        notes: 'Validation-safe local content',
      });
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    });

    it('moves keyboard focus to the stable Save action while an ordinary retry saves', async () => {
      const user = userEvent.setup();
      const coordinator = new DraftCoordinator();
      const retryWrite = deferred<GraphNode>();
      vi.mocked(commands.updateEntity)
        .mockRejectedValueOnce({ code: 'DATABASE', message: 'The vault is locked.' })
        .mockReturnValueOnce(retryWrite.promise);
      renderManager(coordinator);
      await user.click(await screen.findByRole('button', { name: 'Mira' }));
      await user.clear(screen.getByLabelText('Notes'));
      await user.type(screen.getByLabelText('Notes'), 'Retry-safe content');
      await user.click(screen.getByTestId('entity-form-submit'));
      const failure = await screen.findByRole('alert');
      const retry = within(failure).getByRole('button', { name: 'Retry' });
      retry.focus();

      await user.keyboard('{Enter}');

      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));
      expect(screen.getByTestId('entity-form-submit')).toHaveFocus();

      retryWrite.resolve(mira({ notes: 'Retry-safe content' }));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
      expect(screen.getByTestId('entity-form-submit')).toHaveFocus();
      expect(screen.getByLabelText('Notes')).toHaveValue('Retry-safe content');
      expect(commands.updateEntity).toHaveBeenCalledTimes(2);
    });

    it('restores keyboard focus to Retry when an ordinary retry remains actionable', async () => {
      const user = userEvent.setup();
      const coordinator = new DraftCoordinator();
      vi.mocked(commands.updateEntity)
        .mockRejectedValueOnce({ code: 'DATABASE', message: 'First failure.' })
        .mockRejectedValueOnce({ code: 'DATABASE', message: 'Still unavailable.' });
      renderManager(coordinator);
      await user.click(await screen.findByRole('button', { name: 'Mira' }));
      await user.clear(screen.getByLabelText('Notes'));
      await user.type(screen.getByLabelText('Notes'), 'Retry-safe content');
      await user.click(screen.getByTestId('entity-form-submit'));
      const firstRetry = within(await screen.findByRole('alert')).getByRole('button', {
        name: 'Retry',
      });
      firstRetry.focus();

      await user.keyboard('{Enter}');

      const secondFailure = await screen.findByRole('alert');
      expect(secondFailure).toHaveTextContent('Still unavailable.');
      expect(within(secondFailure).getByRole('button', { name: 'Retry' })).toHaveFocus();
      expect(screen.getByLabelText('Notes')).toHaveValue('Retry-safe content');
      expect(commands.updateEntity).toHaveBeenCalledTimes(2);
    });

    it('keeps a delayed validation failure attached to its original record', async () => {
      const coordinator = new DraftCoordinator();
      const miraWrite = deferred<GraphNode>();
      vi.mocked(commands.updateEntity).mockReturnValue(miraWrite.promise);
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Mira' }));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Mira validation-safe content' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saving…'));

      await fireEvent.click(screen.getByRole('button', { name: 'Torvin' }));
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Torvin');
      expect(screen.getByLabelText('Notes')).toHaveValue('Saved Torvin note');

      miraWrite.reject({
        code: 'VALIDATION',
        message: 'Mira conflicts with an existing NPC.',
        field: 'name',
      });

      await waitFor(() =>
        expect(getEntityRow('Mira').button).toHaveAccessibleDescription("Couldn't save"),
      );
      expect(screen.getByLabelText('Name', { exact: true })).toHaveValue('Torvin');
      expect(screen.queryByText('Mira conflicts with an existing NPC.')).not.toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();

      await fireEvent.click(getEntityRow('Mira').button);
      expect(screen.getByLabelText('Notes')).toHaveValue('Mira validation-safe content');
      const failure = await screen.findByRole('alert');
      expect(failure).toHaveTextContent("Couldn't save");
      expect(failure).toHaveTextContent('Mira conflicts with an existing NPC.');
      expect(within(failure).getByRole('button', { name: 'Retry' })).toBeEnabled();
    });

    it('keeps required-name validation client-side and leaves the draft pending', async () => {
      const coordinator = new DraftCoordinator();
      renderManager(coordinator);
      await fireEvent.click(await screen.findByRole('button', { name: 'Mira' }));
      await fireEvent.input(screen.getByLabelText('Name', { exact: true }), {
        target: { value: '' },
      });
      await fireEvent.submit(screen.getByRole('form'));

      expect(screen.getByText('Name is required.')).toBeInTheDocument();
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
      expect(commands.updateEntity).not.toHaveBeenCalled();
      expect(commands.createEntity).not.toHaveBeenCalled();
    });

    it('requires clear intent to discard one dirty entity and preserves unrelated drafts', async () => {
      const coordinator = new DraftCoordinator();
      coordinator.open(oracleScope('camp1'), null, '');
      coordinator.revise(oracleScope('camp1'), 'Unsent Oracle question');
      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Torvin'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Torvin local note' },
      });
      await fireEvent.click(screen.getByText('Mira'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Mira local note' },
      });

      const cancelOpener = screen.getByTestId('entity-form-cancel');
      cancelOpener.focus();
      await fireEvent.click(cancelOpener);
      const dialog = await screen.findByRole('dialog', { name: 'Unsaved changes' });
      const keepEditing = within(dialog).getByRole('button', { name: 'Cancel' });
      expect(keepEditing).toHaveFocus();

      await fireEvent.keyDown(dialog, { key: 'Escape' });
      await waitFor(() =>
        expect(screen.queryByRole('dialog', { name: 'Unsaved changes' })).not.toBeInTheDocument(),
      );
      expect(cancelOpener).toHaveFocus();
      expect(screen.getByLabelText('Notes')).toHaveValue('Mira local note');

      await fireEvent.click(cancelOpener);
      await fireEvent.click(
        within(await screen.findByRole('dialog', { name: 'Unsaved changes' })).getByRole('button', {
          name: 'Discard changes',
        }),
      );
      expect(screen.queryByLabelText('Notes')).not.toBeInTheDocument();
      await fireEvent.click(screen.getByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Saved Mira note');
      await fireEvent.click(screen.getByText('Torvin'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Torvin local note');
      expect(coordinator.get<string>(oracleScope('camp1'))?.value).toBe('Unsent Oracle question');
    });

    it('closes a clean form immediately without opening a discard dialog', async () => {
      const coordinator = new DraftCoordinator();
      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      await fireEvent.click(screen.getByTestId('entity-form-cancel'));

      expect(screen.queryByRole('dialog', { name: 'Unsaved changes' })).not.toBeInTheDocument();
      expect(screen.queryByLabelText('Notes')).not.toBeInTheDocument();
    });

    it('blocks discard during an active save and enables it after the older revision settles', async () => {
      const coordinator = new DraftCoordinator();
      const firstWrite = deferred<GraphNode>();
      vi.mocked(commands.updateEntity).mockReturnValue(firstWrite.promise);
      renderManager(coordinator);
      await fireEvent.click(await screen.findByText('Mira'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision one' },
      });
      await fireEvent.submit(screen.getByRole('form'));
      await fireEvent.input(screen.getByLabelText('Notes'), {
        target: { value: 'Revision two' },
      });
      await fireEvent.click(screen.getByTestId('entity-form-cancel'));

      const dialog = await screen.findByRole('dialog', { name: 'Unsaved changes' });
      const discard = within(dialog).getByRole('button', { name: 'Discard changes' });
      expect(discard).toBeDisabled();
      expect(discard).toHaveAccessibleDescription(
        'Wait for saving to finish before discarding changes',
      );
      expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();

      firstWrite.resolve({ ...mira(), notes: 'Revision one' });
      await waitFor(() => expect(discard).toBeEnabled());
      expect(screen.getByLabelText('Notes')).toHaveValue('Revision two');
      await fireEvent.click(discard);

      await fireEvent.click(screen.getByText('Mira'));
      expect(screen.getByLabelText('Notes')).toHaveValue('Revision one');
    });
  });

  it('ignores a stale entity load after campaign and kind change', async () => {
    const coordinator = new DraftCoordinator();
    const oldLoad = deferred<GraphNode[]>();
    vi.mocked(commands.getEntities).mockImplementation((campaignId, kind) => {
      if (campaignId === 'camp-a' && kind === 'npc') return oldLoad.promise;
      if (campaignId === 'camp-b' && kind === 'location') {
        return Promise.resolve([
          mockNpc({ id: 'moon-gate', kind: 'location', campaign_id: 'camp-b', name: 'Moon Gate' }),
        ]);
      }
      return Promise.resolve([]);
    });
    const rendered = renderManager(coordinator, { campaignId: 'camp-a', kind: 'npc' });
    await waitFor(() => expect(commands.getEntities).toHaveBeenCalledWith('camp-a', 'npc'));

    await rendered.rerender({
      campaignId: 'camp-b',
      kind: 'location',
      openId: null,
      draftCoordinator: coordinator,
    });
    expect(await screen.findByText('Moon Gate')).toBeInTheDocument();

    oldLoad.resolve([mockNpc({ id: 'old', campaign_id: 'camp-a', name: 'Old Campaign NPC' })]);
    await waitFor(() => expect(screen.queryByText('Old Campaign NPC')).not.toBeInTheDocument());
    expect(screen.getByText('Moon Gate')).toBeInTheDocument();
  });
});
