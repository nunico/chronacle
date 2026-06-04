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

  it('renders 8 entity type tabs', () => {
    render(EntityManager, { props: { campaignId: 'camp1' } });
    for (const label of ['NPC', 'Location', 'Faction', 'Creature', 'Item', 'Event', 'Misc']) {
      expect(screen.getByRole('tab', { name: new RegExp(label, 'i') })).toBeTruthy();
    }
    // Use exact boundary match so "NPC" tab doesn't collide with /PC/i
    expect(screen.getByRole('tab', { name: /^PC$/ })).toBeTruthy();
  });

  it('loads NPC list on mount', async () => {
    vi.mocked(commands.getEntities).mockResolvedValue([mockNpc()]);
    render(EntityManager, { props: { campaignId: 'camp1' } });
    await waitFor(() => expect(screen.getByText('Torvin')).toBeTruthy());
    expect(commands.getEntities).toHaveBeenCalledWith('camp1', 'npc');
  });

  it('shows form when New button is clicked', async () => {
    render(EntityManager, { props: { campaignId: 'camp1' } });
    await waitFor(() => screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    expect(screen.getByLabelText(/name/i)).toBeTruthy();
  });

  it('shows toast on DATABASE error from createEntity', async () => {
    vi.mocked(commands.createEntity).mockRejectedValue({
      code: 'DATABASE', message: 'disk full',
    });
    render(EntityManager, { props: { campaignId: 'camp1' } });
    await waitFor(() => screen.getByRole('button', { name: /new npc/i }));
    await fireEvent.click(screen.getByRole('button', { name: /new npc/i }));
    // submit the form with a name — find the form and its name input
    const nameInput = screen.getByLabelText(/^name$/i);
    await fireEvent.input(nameInput, { target: { value: 'Test NPC' } });
    await fireEvent.submit(screen.getByRole('form'));
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
  });
});
