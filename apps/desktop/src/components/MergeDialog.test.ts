import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import MergeDialog from './MergeDialog.svelte';
import type { GraphNode } from '../lib/commands';
import { i18n } from '../lib/locale.svelte';

vi.mock('../lib/commands', () => ({
  getEntity: vi.fn(),
  getEntityRelations: vi.fn().mockResolvedValue([]),
  mergeEntities: vi.fn().mockResolvedValue(undefined),
}));

import * as commands from '../lib/commands';
const m = vi.mocked(commands);

function node(overrides: Partial<GraphNode> = {}): GraphNode {
  return {
    id: 'a',
    kind: 'faction',
    campaign_id: 'camp1',
    name: 'The Free League',
    aliases: [],
    summary: 'A summary',
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
  };
}

describe('MergeDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getEntityRelations.mockResolvedValue([]);
    m.mergeEntities.mockResolvedValue(undefined);
  });

  it('loads both entities and shows them side by side', async () => {
    m.getEntity.mockImplementation((id) =>
      Promise.resolve(
        id === 'a'
          ? node({ id: 'a', name: 'The Free League' })
          : node({ id: 'b', name: 'Free League' }),
      ),
    );
    render(MergeDialog, { props: { idA: 'a', kindA: 'faction', idB: 'b', kindB: 'faction' } });

    await screen.findByText('The Free League');
    expect(screen.getByText('Free League')).toBeInTheDocument();
  });

  it('shows a plain-language consequence line naming relationship and alternate-name counts', async () => {
    m.getEntity.mockImplementation((id) =>
      Promise.resolve(
        id === 'a'
          ? node({ id: 'a', name: 'The Free League' })
          : node({ id: 'b', name: 'Free League', aliases: ['Freeleague'] }),
      ),
    );
    m.getEntityRelations.mockResolvedValue([
      { id: 'x', kind: 'npc', name: 'X', rel_type: 'allied_with', direction: 'outbound' },
    ]);
    render(MergeDialog, { props: { idA: 'a', kindA: 'faction', idB: 'b', kindB: 'faction' } });

    await screen.findByText('The Free League');
    // survivor defaults to A, so the loser (B) contributes 1 relationship and
    // its own name + 1 existing alternate name = 2 alternate names kept.
    await waitFor(() =>
      expect(
        screen.getByText(/1 relationship merged, 2 alternate names kept/i),
      ).toBeInTheDocument(),
    );
  });

  it('merges with the chosen survivor and per-field choices', async () => {
    m.getEntity.mockImplementation((id) =>
      Promise.resolve(
        id === 'a'
          ? node({ id: 'a', kind: 'faction', name: 'The Free League' })
          : node({ id: 'b', kind: 'faction', name: 'Free League' }),
      ),
    );
    const onmerged = vi.fn();
    render(MergeDialog, {
      props: { idA: 'a', kindA: 'faction', idB: 'b', kindB: 'faction', onmerged },
    });

    await screen.findByText('The Free League');

    // Pick B as the survivor.
    const radios = screen.getAllByRole('radio');
    await fireEvent.click(radios[1]);

    const selects = screen.getAllByRole('combobox') as HTMLSelectElement[];
    await fireEvent.change(selects[0], { target: { value: 'keepLoser' } });
    await fireEvent.change(selects[1], { target: { value: 'keepBoth' } });

    await fireEvent.click(screen.getByRole('button', { name: 'Merge' }));

    await waitFor(() =>
      expect(m.mergeEntities).toHaveBeenCalledWith('faction:b', 'faction:a', {
        summary: 'keepLoser',
        notes: 'keepBoth',
      }),
    );
    await waitFor(() => expect(onmerged).toHaveBeenCalled());
  });

  it('calls onclose when Cancel is clicked', async () => {
    m.getEntity.mockImplementation((id) =>
      Promise.resolve(id === 'a' ? node({ id: 'a' }) : node({ id: 'b', name: 'Free League' })),
    );
    const onclose = vi.fn();
    render(MergeDialog, {
      props: { idA: 'a', kindA: 'faction', idB: 'b', kindB: 'faction', onclose },
    });

    await screen.findByText('The Free League');
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onclose).toHaveBeenCalled();
  });

  it('uses the active display language for dialog controls', async () => {
    m.getEntity.mockImplementation((id) =>
      Promise.resolve(id === 'a' ? node({ id: 'a' }) : node({ id: 'b', name: 'Free League' })),
    );
    i18n.setLocale('de');
    try {
      render(MergeDialog, {
        props: { idA: 'a', kindA: 'faction', idB: 'b', kindB: 'faction' },
      });

      expect(await screen.findByRole('dialog', { name: 'Entitäten zusammenführen' })).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Abbrechen' })).toBeTruthy();
    } finally {
      i18n.setLocale('en');
    }
  });
});
