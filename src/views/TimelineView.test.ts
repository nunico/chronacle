import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TimelineView from './TimelineView.svelte';
import * as commands from '../lib/commands';
import type { Session } from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getEventsTimeline: vi.fn().mockResolvedValue([]),
  getSessions: vi.fn().mockResolvedValue([]),
}));

const m = vi.mocked(commands);

function ev(name: string, sequence_index: number | null, era: string | null, session_id: string | null = null) {
  return {
    id: name, kind: 'event', campaign_id: null, name, summary: null, notes: null,
    created_at: null, updated_at: null, date_start: null, date_end: null,
    is_ongoing: false, sequence_index, era, duration_label: null, session_id,
    player_name: null, character_class: null, character_level: null, status: null,
  };
}

function sess(id: string, title: string): Session {
  return { id, title } as unknown as Session;
}

beforeEach(() => {
  vi.clearAllMocks();
  m.getEventsTimeline.mockResolvedValue([]);
  m.getSessions.mockResolvedValue([]);
});

describe('TimelineView — chronicle mode', () => {
  it('renders era headers and events in order, with an Unordered group last', async () => {
    m.getEventsTimeline.mockResolvedValueOnce([
      ev('Siege', 1, 'Dawn'),
      ev('Pact', 2, 'Dawn'),
      ev('Lost', null, null),
    ]);
    render(TimelineView, { campaignId: 'c1' });
    expect(await screen.findByText('Siege')).toBeTruthy();
    expect(screen.getByText('Pact')).toBeTruthy();
    expect(screen.getByText('Dawn')).toBeTruthy();
    expect(screen.getByText('Unordered')).toBeTruthy();
    expect(screen.getByText('Lost')).toBeTruthy();
  });

  it('shows an empty state when there are no events', async () => {
    m.getEventsTimeline.mockResolvedValueOnce([]);
    render(TimelineView, { campaignId: 'c1' });
    expect(await screen.findByText(/No events yet/)).toBeTruthy();
  });

  it('shows an error banner and suppresses the empty state on fetch failure', async () => {
    m.getEventsTimeline.mockRejectedValueOnce(new Error('IPC error'));
    render(TimelineView, { campaignId: 'c1' });
    expect(await screen.findByText('Failed to load timeline')).toBeTruthy();
    expect(screen.queryByText(/No events yet/)).toBeFalsy();
  });
});

describe('TimelineView — sessions mode', () => {
  it('renders session lane headers and nested events after switching to Sessions tab', async () => {
    m.getEventsTimeline.mockResolvedValueOnce([
      ev('Battle of Ash', 1, 'Dawn', 's1'),
      ev('Pact Signed', 2, 'Dawn', 's2'),
      ev('Lost Soul', null, null, null),
    ]);
    m.getSessions.mockResolvedValueOnce([
      sess('s1', 'Session 1'),
      sess('s2', 'Session 2'),
    ]);
    render(TimelineView, { campaignId: 'c1' });
    // Wait for data to load
    await screen.findByText('Battle of Ash');
    // Click the Sessions tab
    await fireEvent.click(screen.getByRole('tab', { name: 'Sessions' }));
    // Lane headers
    expect(screen.getByText('Session 1')).toBeTruthy();
    expect(screen.getByText('Session 2')).toBeTruthy();
    // Events nested correctly
    expect(screen.getByText('Battle of Ash')).toBeTruthy();
    expect(screen.getByText('Pact Signed')).toBeTruthy();
    // Unscheduled lane for event with no session_id
    expect(screen.getByText('Unscheduled')).toBeTruthy();
    expect(screen.getByText('Lost Soul')).toBeTruthy();
  });

  it('omits the Unscheduled lane when all events belong to a session', async () => {
    m.getEventsTimeline.mockResolvedValueOnce([
      ev('First Light', 1, null, 's1'),
    ]);
    m.getSessions.mockResolvedValueOnce([
      sess('s1', 'Session 1'),
    ]);
    render(TimelineView, { campaignId: 'c1' });
    await screen.findByText('First Light');
    await fireEvent.click(screen.getByRole('tab', { name: 'Sessions' }));
    expect(screen.getByText('Session 1')).toBeTruthy();
    expect(screen.queryByText('Unscheduled')).toBeFalsy();
  });
});
