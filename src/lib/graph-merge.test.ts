import { describe, it, expect } from 'vitest';
import { mergeGraph } from './graph-merge';
import type { EntityGraph } from './commands';

const base: EntityGraph = {
  nodes: [{ id: 'a', kind: 'npc', name: 'A' }, { id: 'b', kind: 'npc', name: 'B' }],
  edges: [{ from_id: 'a', from_kind: 'npc', to_id: 'b', to_kind: 'npc', rel_type: 'knows', notes: null }],
};

describe('mergeGraph', () => {
  it('adds new nodes/edges and dedupes existing ones', () => {
    const extra: EntityGraph = {
      nodes: [{ id: 'b', kind: 'npc', name: 'B' }, { id: 'c', kind: 'npc', name: 'C' }],
      edges: [
        { from_id: 'a', from_kind: 'npc', to_id: 'b', to_kind: 'npc', rel_type: 'knows', notes: null }, // dup
        { from_id: 'b', from_kind: 'npc', to_id: 'c', to_kind: 'npc', rel_type: 'rivals', notes: null }, // new
      ],
    };
    const merged = mergeGraph(base, extra);
    expect(merged.nodes.map((n) => n.id).sort()).toEqual(['a', 'b', 'c']);
    expect(merged.edges.length).toBe(2);
  });

  it('returns a when b is empty', () => {
    const empty: EntityGraph = { nodes: [], edges: [] };
    const merged = mergeGraph(base, empty);
    expect(merged.nodes.map((n) => n.id).sort()).toEqual(['a', 'b']);
    expect(merged.edges.length).toBe(1);
  });

  it('returns b when a is empty', () => {
    const empty: EntityGraph = { nodes: [], edges: [] };
    const merged = mergeGraph(empty, base);
    expect(merged.nodes.map((n) => n.id).sort()).toEqual(['a', 'b']);
    expect(merged.edges.length).toBe(1);
  });
});
