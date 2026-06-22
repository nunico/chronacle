import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import EntityGraph from './EntityGraph.svelte';
import * as commands from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getEntityGraph: vi.fn(),
}));

const m = vi.mocked(commands);

beforeEach(() => {
  vi.clearAllMocks();
});

const graph = {
  nodes: [
    { id: 'varin', kind: 'npc', name: 'Varin' },
    { id: 'keep', kind: 'location', name: 'The Keep' },
  ],
  edges: [{ from_id: 'varin', from_kind: 'npc', to_id: 'keep', to_kind: 'location', rel_type: 'resides_in', notes: null }],
};

describe('EntityGraph', () => {
  it('renders center + neighbor nodes and the edge label', async () => {
    m.getEntityGraph.mockResolvedValueOnce(graph);
    render(EntityGraph, { entityId: 'varin', entityKind: 'npc' });
    expect(await screen.findByText('Varin')).toBeTruthy();
    expect(screen.getByText('The Keep')).toBeTruthy();
    expect(screen.getByText('resides_in')).toBeTruthy();
  });

  it('shows an empty state for a lonely entity', async () => {
    m.getEntityGraph.mockResolvedValueOnce({ nodes: [{ id: 'h', kind: 'npc', name: 'Hermit' }], edges: [] });
    render(EntityGraph, { entityId: 'h', entityKind: 'npc' });
    expect(await screen.findByTestId('graph-empty')).toBeTruthy();
  });
});
