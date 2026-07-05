import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import MaintenanceView from './MaintenanceView.svelte';
import type { CodexProposal } from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getProposals: vi.fn().mockResolvedValue([]),
  acceptProposal: vi.fn().mockResolvedValue(undefined),
  rejectProposal: vi.fn().mockResolvedValue(undefined),
}));

import * as commands from '../lib/commands';
const m = vi.mocked(commands);

function proposal(overrides: Partial<CodexProposal> = {}): CodexProposal {
  return {
    id: 'codex_proposal:1',
    kind: 'entity_article_update',
    target: 'npc:1',
    target_name: 'Grix the Elder',
    current_text: 'Grix is a merchant.',
    payload: {
      proposed_text: 'Grix is a merchant who now trades in relics.',
      rationale: 'The party learned Grix deals in ancient relics.',
      name: null,
      entity_kind: null,
      category: null,
    },
    origin_kind: 'session',
    status: 'pending',
    created_at: '2026-07-05T00:00:00Z',
    ...overrides,
  };
}

describe('MaintenanceView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getProposals.mockResolvedValue([]);
  });

  it('renders pending proposals with kind label, target name, rationale', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await waitFor(() => expect(screen.getByText('Grix the Elder')).toBeTruthy());
    expect(screen.getByText('Article update')).toBeTruthy();
    expect(screen.getByText('The party learned Grix deals in ancient relics.')).toBeTruthy();
  });

  it('shows current vs proposed text side-by-side', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await waitFor(() => expect(screen.getByText('Grix is a merchant.')).toBeTruthy());
    expect(screen.getByText('Grix is a merchant who now trades in relics.')).toBeTruthy();
  });

  it('invokes accept_proposal with the row id, then refetches', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await screen.findByText('Grix the Elder');
    m.getProposals.mockResolvedValue([]);
    await fireEvent.click(screen.getByLabelText('Accept proposal'));
    await waitFor(() => expect(m.acceptProposal).toHaveBeenCalledWith('codex_proposal:1'));
    await waitFor(() => expect(m.getProposals).toHaveBeenCalledTimes(2));
  });

  it('invokes reject_proposal with the row id', async () => {
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: {} });
    await screen.findByText('Grix the Elder');
    await fireEvent.click(screen.getByLabelText('Reject proposal'));
    await waitFor(() => expect(m.rejectProposal).toHaveBeenCalledWith('codex_proposal:1'));
  });

  it('renders "No pending proposals" for the empty state', async () => {
    m.getProposals.mockResolvedValue([]);
    render(MaintenanceView, { props: {} });
    await waitFor(() => expect(screen.getByText('No pending proposals')).toBeTruthy());
  });

  it('calls onCountsChanged after resolving a proposal', async () => {
    const onCountsChanged = vi.fn();
    m.getProposals.mockResolvedValue([proposal()]);
    render(MaintenanceView, { props: { onCountsChanged } });
    await screen.findByText('Grix the Elder');
    await fireEvent.click(screen.getByLabelText('Reject proposal'));
    await waitFor(() => expect(onCountsChanged).toHaveBeenCalled());
  });
});
