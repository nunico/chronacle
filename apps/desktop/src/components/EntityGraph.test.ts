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

  it('renders parallel edges between the same pair (different rel_type) without crashing', async () => {
    // Real extractions produce multiple edges between the same two entities with
    // different rel_types (e.g. a faction both `located_in` and `related_to` a
    // location). The link `{#each}` key must include rel_type, otherwise Svelte
    // throws each_key_duplicate at render time and blanks the entire graph —
    // including the center node. See entity_service get_entity_graph.
    const parallel = {
      nodes: [
        { id: 'hegemony', kind: 'faction', name: 'Zenithian Hegemony' },
        { id: 'spire', kind: 'location', name: 'The Spire' },
      ],
      edges: [
        { from_id: 'hegemony', from_kind: 'faction', to_id: 'spire', to_kind: 'location', rel_type: 'located_in', notes: null },
        { from_id: 'hegemony', from_kind: 'faction', to_id: 'spire', to_kind: 'location', rel_type: 'related_to', notes: null },
      ],
    };
    m.getEntityGraph.mockResolvedValueOnce(parallel);
    render(EntityGraph, { entityId: 'hegemony', entityKind: 'faction' });
    // Both nodes render and both edge labels are present.
    expect(await screen.findByText('Zenithian Hegemony')).toBeTruthy();
    expect(screen.getByText('The Spire')).toBeTruthy();
    expect(screen.getByText('located_in')).toBeTruthy();
    expect(screen.getByText('related_to')).toBeTruthy();
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

  it('renders missing wikilink nodes distinctly and opens create flow on click', async () => {
    const onMissingLinkClick = vi.fn();
    m.getEntityGraph.mockResolvedValueOnce({
      nodes: [
        { id: 'mira', kind: 'npc', name: 'Mira' },
        {
          id: 'missing_wikilink:npc:mira:moon gate',
          kind: 'missing_wikilink',
          name: 'Moon Gate',
          missing: true,
          source_id: 'mira',
          source_kind: 'npc',
        },
      ],
      edges: [
        {
          from_id: 'mira',
          from_kind: 'npc',
          to_id: 'missing_wikilink:npc:mira:moon gate',
          to_kind: 'missing_wikilink',
          rel_type: 'unresolved',
          notes: null,
        },
      ],
    });

    const { container } = render(EntityGraph, {
      entityId: 'mira',
      entityKind: 'npc',
      onMissingLinkClick,
    });

    expect(await screen.findByText('[[Moon Gate]]')).toBeTruthy();
    const missingNode = container.querySelector('[data-missing="true"]') as Element;
    expect(missingNode).toBeTruthy();

    await fireEvent.click(missingNode);
    expect(onMissingLinkClick).toHaveBeenCalledWith('Moon Gate');
  });
});
