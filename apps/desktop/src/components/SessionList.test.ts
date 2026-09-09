import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import SessionList from './SessionList.svelte';
import type { Session } from '../lib/commands';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';

vi.mock('../lib/commands', () => ({
  updateSession: vi.fn(),
  deleteSession: vi.fn(),
  getSessionEntities: vi.fn().mockResolvedValue([]),
}));

import * as commands from '../lib/commands';

function session(id: string, title: string): Session {
  return {
    id,
    campaign_id: 'camp-a',
    session_number: id === 'session-a' ? 1 : 2,
    title,
    date_played: '2026-09-01',
    notes: `${title} saved notes`,
    created_at: null,
    updated_at: null,
  };
}

function renderList(coordinator = new DraftCoordinator()) {
  return render(SessionList, {
    props: {
      campaignId: 'camp-a',
      sessions: [session('session-a', 'Ashes at Dawn'), session('session-b', 'Moonlit Road')],
      entityMap: new Map(),
      onUpdate: vi.fn(),
      onDelete: vi.fn(),
      draftCoordinator: coordinator,
    },
  } as never);
}

function rowFor(title: string): HTMLElement {
  const row = screen.getByRole('button', { name: new RegExp(title) }).closest('.session-row');
  if (!(row instanceof HTMLElement)) throw new Error(`Expected a session row for ${title}`);
  return row;
}

describe('SessionList draft coordination', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(commands.getSessionEntities).mockResolvedValue([]);
  });

  it('keeps a failed save and Retry local to the intended session row', async () => {
    const backendDetail = 'Internal session row 17 vanished during UPDATE.';
    vi.mocked(commands.updateSession).mockRejectedValue({
      code: 'NOT_FOUND',
      message: backendDetail,
    });
    renderList();
    const ashesRow = rowFor('Ashes at Dawn');
    const moonlitRow = rowFor('Moonlit Road');
    await fireEvent.click(within(ashesRow).getByRole('button', { name: /Ashes at Dawn/ }));
    const title = within(ashesRow).getByRole('textbox', { name: 'Name' });
    await fireEvent.input(title, { target: { value: 'Ashes retained locally' } });
    await fireEvent.blur(title);

    const alert = await within(ashesRow).findByRole('alert');
    expect(alert).toHaveTextContent('This record is no longer available.');
    expect(alert).not.toHaveTextContent(backendDetail);
    expect(title).toHaveValue('Ashes retained locally');
    expect(within(ashesRow).getByRole('button', { name: 'Retry' })).toBeVisible();
    expect(within(moonlitRow).queryByRole('alert')).not.toBeInTheDocument();
    expect(within(moonlitRow).getByText('Moonlit Road')).toBeVisible();

    await fireEvent.click(within(ashesRow).getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(commands.updateSession).toHaveBeenCalledTimes(2));
    expect(vi.mocked(commands.updateSession).mock.calls[1]?.[0]).toBe('session-a');
  });

  it('keeps pending state visible after its edited row is collapsed', async () => {
    renderList();
    const ashesRow = rowFor('Ashes at Dawn');
    const header = within(ashesRow).getByRole('button', { name: /Ashes at Dawn/ });
    await fireEvent.click(header);
    await fireEvent.input(within(ashesRow).getByRole('textbox', { name: 'Name' }), {
      target: { value: 'Unwritten Ashes' },
    });
    await fireEvent.click(header);

    expect(within(ashesRow).getByText('Unsaved changes', { exact: true })).toBeVisible();
    expect(within(rowFor('Moonlit Road')).queryByText('Unsaved changes')).not.toBeInTheDocument();
  });
});
