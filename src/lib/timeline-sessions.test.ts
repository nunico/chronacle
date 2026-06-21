import { describe, it, expect } from 'vitest';
import { groupBySession } from './timeline-sessions';
import type { GraphNode, Session } from './commands';

function ev(name: string, session_id: string | null): GraphNode {
  return {
    id: name, kind: 'event', campaign_id: null, name, summary: null, notes: null,
    created_at: null, updated_at: null, date_start: null, date_end: null,
    is_ongoing: false, sequence_index: null, era: null, duration_label: null,
    session_id, player_name: null, character_class: null, character_level: null, status: null,
  };
}
function sess(id: string, title: string): Session {
  return { id, title } as unknown as Session; // fill remaining required fields per Step 1
}

describe('groupBySession', () => {
  it('nests events under their session in session order, with Unscheduled last', () => {
    const sessions = [sess('s1', 'Session 1'), sess('s2', 'Session 2')];
    const events = [ev('A', 's1'), ev('B', 's2'), ev('C', 's1'), ev('D', null)];
    const lanes = groupBySession(sessions, events);
    expect(lanes.map((l) => l.session?.id ?? null)).toEqual(['s1', 's2', null]);
    expect(lanes[0].events.map((e) => e.name)).toEqual(['A', 'C']);
    expect(lanes[2].events.map((e) => e.name)).toEqual(['D']);
  });

  it('omits the Unscheduled lane when every event has a session', () => {
    const lanes = groupBySession([sess('s1', 'S1')], [ev('A', 's1')]);
    expect(lanes.map((l) => l.session?.id ?? null)).toEqual(['s1']);
  });
});
