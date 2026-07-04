import { describe, it, expect } from 'vitest';
import { groupByEra } from './timeline-groups';
import type { GraphNode } from './commands';

function ev(name: string, sequence_index: number | null, era: string | null): GraphNode {
  return {
    id: name, kind: 'event', campaign_id: null, name,
    summary: null, notes: null, created_at: null, updated_at: null,
    date_start: null, date_end: null, is_ongoing: null,
    sequence_index, era, duration_label: null, session_id: null,
    player_name: null, character_class: null, character_level: null, status: null,
    codex_article: null, codex_stale: null, codex_compiled_at: null,
  };
}

describe('groupByEra', () => {
  it('groups consecutive events by era in first-seen order, nulls last', () => {
    // Caller passes events already in timeline order.
    const ordered = [
      ev('A', 1, 'Dawn'),
      ev('B', 2, 'Dawn'),
      ev('C', 3, 'Dusk'),
      ev('D', null, 'Dusk'),
      ev('E', null, null),
    ];
    const groups = groupByEra(ordered);
    expect(groups.map((g) => g.era)).toEqual(['Dawn', 'Dusk', null]);
    expect(groups[0].events.map((e) => e.name)).toEqual(['A', 'B']);
    expect(groups[1].events.map((e) => e.name)).toEqual(['C', 'D']);
    expect(groups[2].events.map((e) => e.name)).toEqual(['E']);
  });

  it('returns an empty array for no events', () => {
    expect(groupByEra([])).toEqual([]);
  });

  it('interleaved eras form separate groups to preserve sequence_index order', () => {
    // A:Dawn, B:Dusk, C:Dawn — same era but non-consecutive → THREE groups,
    // not two. This documents the intentional run-based (not merge-based) behavior:
    // we never reorder events just to collapse era labels.
    const ordered = [ev('A', 1, 'Dawn'), ev('B', 2, 'Dusk'), ev('C', 3, 'Dawn')];
    const groups = groupByEra(ordered);
    expect(groups.map((g) => g.era)).toEqual(['Dawn', 'Dusk', 'Dawn']);
    expect(groups[0].events.map((e) => e.name)).toEqual(['A']);
    expect(groups[1].events.map((e) => e.name)).toEqual(['B']);
    expect(groups[2].events.map((e) => e.name)).toEqual(['C']);
  });
});
