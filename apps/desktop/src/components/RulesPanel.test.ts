import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import RulesPanel from './RulesPanel.svelte';
import type { RuleEntry } from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getRuleEntries: vi.fn().mockResolvedValue([]),
  updateRuleNotes: vi.fn().mockResolvedValue(undefined),
  redoRuleEntry: vi.fn().mockResolvedValue(undefined),
}));

import * as commands from '../lib/commands';
const m = vi.mocked(commands);

function rule(id: string, name: string, category: string): RuleEntry {
  return {
    id,
    name,
    category,
    body: `Body text for ${name}`,
    notes: null,
    page_refs: [{ source_name: 'PHB', page_start: 10, page_end: 12 }],
    stale: false,
  };
}

describe('RulesPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getRuleEntries.mockResolvedValue([]);
  });

  it('groups entries by category and filters by search', async () => {
    m.getRuleEntries.mockResolvedValue([
      rule('r1', 'Initiative', 'mechanic'),
      rule('r2', 'Fireball', 'ability'),
    ]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await waitFor(() => expect(screen.getByText('Initiative')).toBeTruthy());
    expect(screen.getByRole('heading', { name: /mechanic/i })).toBeTruthy();
    await fireEvent.input(screen.getByLabelText('Search rules'), { target: { value: 'fire' } });
    await waitFor(() => expect(screen.queryByText('Initiative')).toBeNull());
    expect(screen.getByText('Fireball')).toBeTruthy();
  });

  it('saves table notes on blur and submits objections', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    const notes = screen.getByLabelText('Table notes');
    await fireEvent.input(notes, { target: { value: 'we roll once per round' } });
    await fireEvent.blur(notes);
    await waitFor(() =>
      expect(m.updateRuleNotes).toHaveBeenCalledWith('r1', 'we roll once per round'),
    );
    await fireEvent.click(screen.getByText(/Redo with objections/));
    await fireEvent.input(screen.getByLabelText('Objection'), {
      target: { value: 'range is wrong' },
    });
    await fireEvent.click(screen.getByText('Submit'));
    await waitFor(() => expect(m.redoRuleEntry).toHaveBeenCalledWith('r1', 'range is wrong'));
  });

  it('shows an error message when loading entries fails, not the empty state', async () => {
    m.getRuleEntries.mockRejectedValue(new Error('boom'));
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
    expect(screen.getByText(/Failed to load rule entries/)).toBeTruthy();
    expect(screen.queryByText('No rule entries compiled yet.')).toBeNull();
  });

  it('does not save notes on blur when the draft is unchanged', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    const notes = screen.getByLabelText('Table notes');
    await fireEvent.blur(notes);
    expect(m.updateRuleNotes).not.toHaveBeenCalled();
  });

  it('keeps the notes baseline fresh after a save, so clearing then blurring saves null', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    const notes = screen.getByLabelText('Table notes');

    await fireEvent.input(notes, { target: { value: 'house rule' } });
    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledWith('r1', 'house rule'));

    await fireEvent.input(notes, { target: { value: '' } });
    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledWith('r1', null));
    expect(m.updateRuleNotes).toHaveBeenCalledTimes(2);
  });

  it('collapses page refs to a single page when start equals end', async () => {
    m.getRuleEntries.mockResolvedValue([
      {
        ...rule('r1', 'Initiative', 'mechanic'),
        page_refs: [{ source_name: 'PHB', page_start: 42, page_end: 42 }],
      },
    ]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    expect(screen.getByText(/PHB p\.42/)).toBeTruthy();
    expect(screen.queryByText(/p\.42-42/)).toBeNull();
  });

  it('shows the redo error and keeps the dialog open when redo fails', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.redoRuleEntry.mockRejectedValue(new Error('redo boom'));
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    await fireEvent.click(screen.getByText(/Redo with objections/));
    await fireEvent.input(screen.getByLabelText('Objection'), {
      target: { value: 'range is wrong' },
    });
    await fireEvent.click(screen.getByText('Submit'));
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
    expect(screen.getByText(/Failed to redo: Error: redo boom/)).toBeTruthy();
    expect(screen.getByLabelText('Objection')).toBeTruthy();
  });
});
