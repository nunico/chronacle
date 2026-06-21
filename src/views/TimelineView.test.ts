import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import TimelineView from './TimelineView.svelte';
import * as commands from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getEventsTimeline: vi.fn().mockResolvedValue([]),
}));

const m = vi.mocked(commands);

function ev(name: string, sequence_index: number | null, era: string | null) {
  return {
    id: name, kind: 'event', campaign_id: null, name, summary: null, notes: null,
    created_at: null, updated_at: null, date_start: null, date_end: null,
    is_ongoing: false, sequence_index, era, duration_label: null, session_id: null,
    player_name: null, character_class: null, character_level: null, status: null,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
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
    expect(screen.getByText('Dawn')).toBeTruthy();
    expect(screen.getByText('Unordered')).toBeTruthy();
    expect(screen.getByText('Lost')).toBeTruthy();
  });

  it('shows an empty state when there are no events', async () => {
    m.getEventsTimeline.mockResolvedValueOnce([]);
    render(TimelineView, { campaignId: 'c1' });
    expect(await screen.findByText(/No events yet/)).toBeTruthy();
  });
});
