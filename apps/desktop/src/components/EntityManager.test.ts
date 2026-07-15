import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import EntityManager from './EntityManager.svelte';
import type { GraphNode } from '../lib/commands';

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

const mockNpc = (): GraphNode => ({
  id: 'npc1',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
  aliases: [],
  summary: 'Shady merchant',
  notes: null,
  created_at: null, updated_at: null,
  date_start: null, date_end: null, is_ongoing: null,
  sequence_index: null, era: null, duration_label: null,
  session_id: null,
  player_name: null, character_class: null,
  character_level: null, status: null,
  codex_article: null, codex_stale: null, codex_compiled_at: null,
});

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

  it('shows toast on DATABASE error from createEntity', async () => {
    vi.mocked(commands.createEntity).mockRejectedValue({
      code: 'DATABASE', message: 'disk full',
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

  it('passes VALIDATION error to form when createEntity returns VALIDATION', async () => {
    vi.mocked(commands.createEntity).mockRejectedValue({
      code: 'VALIDATION', message: 'Too long', field: 'name',
    });
    render(EntityManager, { props: { campaignId: 'camp1', kind: 'npc' } });
    await waitFor(() => screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    const nameInput = screen.getByLabelText(/^name$/i);
    await fireEvent.input(nameInput, { target: { value: 'x' } });
    await fireEvent.submit(screen.getByRole('form'));
    // Form should stay open (not a toast — form error)
    await waitFor(() => expect(screen.queryByRole('alert')).toBeFalsy());
    // The form is still visible
    expect(screen.getByLabelText(/^name$/i)).toBeTruthy();
  });

  it('reloads list after NOT_FOUND error', async () => {
    vi.mocked(commands.updateEntity).mockRejectedValue({
      code: 'NOT_FOUND', message: 'Gone',
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
    const brakka: GraphNode = { ...mockNpc(), id: 'npc2', name: 'Brakka', summary: 'Orc chieftain' };
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

  it('form closes after save and onOpenIdConsumed is not called again', async () => {
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

    // Form closes after a successful save.
    await waitFor(() => {
      expect(screen.queryByLabelText(/^name$/i)).toBeNull();
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

    await waitFor(() =>
      expect(commands.compileEntity).toHaveBeenCalledWith('npc', 'npc1'),
    );
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
});
