import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
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

  it('re-centers when a neighbor node group is clicked', async () => {
    const keepGraph = {
      nodes: [
        { id: 'keep', kind: 'location', name: 'The Keep' },
        { id: 'town', kind: 'location', name: 'Town' },
      ],
      edges: [{ from_id: 'keep', from_kind: 'location', to_id: 'town', to_kind: 'location', rel_type: 'near', notes: null }],
    };
    m.getEntityGraph
      .mockResolvedValueOnce(graph)        // initial: varin-centered
      .mockResolvedValueOnce(keepGraph);   // after clicking The Keep node group

    const { container } = render(EntityGraph, { entityId: 'varin', entityKind: 'npc' });

    // Wait for initial render
    await screen.findByText('The Keep');

    // Click the node <g> element for "keep" (not the label, which would open entity)
    const nodeGroup = container.querySelector('[data-id="keep"]') as Element;
    expect(nodeGroup).toBeTruthy();
    await fireEvent.click(nodeGroup);

    // After re-center, Town should appear
    expect(await screen.findByText('Town')).toBeTruthy();
    expect(m.getEntityGraph).toHaveBeenCalledWith('keep', 'location', 1);
  });
});
