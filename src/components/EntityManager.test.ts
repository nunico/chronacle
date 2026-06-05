import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import EntityManager from './EntityManager.svelte';
import type { GraphNode } from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getEntities: vi.fn().mockResolvedValue([]),
  createEntity: vi.fn(),
  updateEntity: vi.fn(),
  deleteEntity: vi.fn(),
}));

import * as commands from '../lib/commands';

const mockNpc = (): GraphNode => ({
  id: 'npc1',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
  summary: 'Shady merchant',
  notes: null,
  created_at: null, updated_at: null,
  date_start: null, date_end: null, is_ongoing: null,
  sequence_index: null, era: null, duration_label: null,
  player_name: null, character_class: null,
  character_level: null, status: null,
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
    expect(screen.getByLabelText(/name/i)).toBeTruthy();
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
    // getEntities should be called again (reload)
    expect(commands.getEntities).toHaveBeenCalledTimes(2);
  });
});
