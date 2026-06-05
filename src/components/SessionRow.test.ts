import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import SessionRow from './SessionRow.svelte';
import type { Session } from '../lib/commands';

vi.mock('../lib/commands', () => ({
  updateSession: vi.fn(),
  deleteSession: vi.fn(),
  getSessionEntities: vi.fn().mockResolvedValue([]),
}));

import * as commands from '../lib/commands';

const mockSession = (): Session => ({
  id: 'sess1',
  campaign_id: 'camp1',
  session_number: 3,
  title: 'The Battle of Ashfields',
  date_played: '2026-06-05',
  notes: 'The party fought bravely.',
  created_at: null,
  updated_at: null,
});

const emptyEntityMap = new Map<string, { id: string; kind: string }>();

describe('SessionRow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(commands.getSessionEntities).mockResolvedValue([]);
  });

  it('renders collapsed state with session title', () => {
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete: vi.fn(),
      },
    });
    expect(screen.getByText('The Battle of Ashfields')).toBeTruthy();
    // Date is visible in collapsed header (formatted or raw)
    expect(screen.getByText(/2026|Jun/)).toBeTruthy();
    // Notes textarea is not rendered when collapsed
    expect(screen.queryByRole('textbox', { name: /notes/i })).toBeNull();
  });

  it('expands on click and shows notes textarea', async () => {
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete: vi.fn(),
      },
    });

    const header = screen.getByRole('button', { name: /The Battle of Ashfields/i });
    await fireEvent.click(header);

    await waitFor(() => {
      expect(screen.getByRole('textbox', { name: /notes/i })).toBeTruthy();
    });
  });

  it('calls onDelete after confirm', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.mocked(commands.deleteSession).mockResolvedValue(undefined);

    const onDelete = vi.fn();
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete,
      },
    });

    // Expand first
    const header = screen.getByRole('button', { name: /The Battle of Ashfields/i });
    await fireEvent.click(header);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /delete/i })).toBeTruthy();
    });

    await fireEvent.click(screen.getByRole('button', { name: /delete/i }));

    await waitFor(() => {
      expect(commands.deleteSession).toHaveBeenCalledWith('sess1');
      expect(onDelete).toHaveBeenCalledWith('sess1');
    });
  });

  it('renders WikiText preview when notes present', async () => {
    render(SessionRow, {
      props: {
        session: mockSession(),
        entityMap: emptyEntityMap,
        onUpdate: vi.fn(),
        onDelete: vi.fn(),
      },
    });

    // Expand the row
    const header = screen.getByRole('button', { name: /The Battle of Ashfields/i });
    await fireEvent.click(header);

    await waitFor(() => {
      // The wiki-preview div should be present since notes is non-empty
      const preview = document.querySelector('.wiki-preview');
      expect(preview).not.toBeNull();
      // The text from notes is rendered inside the WikiText component
      expect(preview?.textContent).toContain('The party fought bravely.');
    });
  });
});
