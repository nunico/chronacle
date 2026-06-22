import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import EntityGraph from './EntityGraph.svelte';
import * as commands from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getEntityGraph: vi.fn(),
}));

// jsdom does not implement ResizeObserver, which Svelte uses internally for
// bind:clientWidth / bind:clientHeight. Stub it so the component mounts cleanly.
// The disconnect/observe/unobserve stubs are no-ops; component rendering
// and interaction tests do not depend on actual dimension measurements.
vi.stubGlobal('ResizeObserver', class {
  observe() {}
  unobserve() {}
  disconnect() {}
});

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

  it('drag suppresses re-center: pointerdown+move(>5px)+pointerup+click does not call getEntityGraph again', async () => {
    // jsdom does not implement getBoundingClientRect on SVG elements (returns all zeros),
    // but the drag-suppress logic (wasDrag flag) is purely coordinate-delta-based and does
    // not need a real SVG rect. We dispatch pointermove on window directly per the spec.
    m.getEntityGraph.mockResolvedValueOnce(graph);
    const { container } = render(EntityGraph, { entityId: 'varin', entityKind: 'npc' });
    await screen.findByText('The Keep');

    const nodeGroup = container.querySelector('[data-id="keep"]') as Element;
    expect(nodeGroup).toBeTruthy();

    // Simulate pointerdown on node
    fireEvent.pointerDown(nodeGroup, { clientX: 100, clientY: 100, bubbles: true });

    // Simulate pointermove on window exceeding the 5px threshold
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 110, clientY: 110, bubbles: true }));

    // Simulate pointerup on window
    window.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }));

    // Now fire a click — it should be suppressed because wasDrag was set
    await fireEvent.click(nodeGroup);

    // getEntityGraph was called exactly once (initial load); the drag-click did NOT trigger recenter
    expect(m.getEntityGraph).toHaveBeenCalledTimes(1);
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
