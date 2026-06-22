<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    forceSimulation, forceManyBody, forceLink, forceCenter, forceCollide,
    type Simulation, type SimulationNodeDatum, type SimulationLinkDatum,
  } from 'd3-force';
  import { getEntityGraph, type EntityGraph, type GraphNodeRef, type EntityKind } from '../lib/commands';
  import { kindColor } from '../lib/graph-colors';

  interface Props {
    entityId: string;
    entityKind: string;
    width?: number;
    height?: number;
    onClose?: () => void;
    onOpenEntity?: (node: GraphNodeRef) => void; // click-through: open in the manager
  }
  const {
    entityId, entityKind, width = 720, height = 520, onClose,
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- wired in Task 9 (node click → open entity)
    onOpenEntity,
  }: Props = $props();

  type SimNode = GraphNodeRef & SimulationNodeDatum;
  // d3-force mutates source/target from string ids to node objects after simulation starts.
  interface SimLink extends SimulationLinkDatum<SimNode> { source: string | SimNode; target: string | SimNode; rel_type: string; }

  // Initialized from entityId prop so the center node is sized correctly on first render.
  let centerId = $state(entityId);
  // centerKind stored for future Task 9 re-center use; kept to avoid prop contract churn
  let centerKind = $state(''); // eslint-disable-line @typescript-eslint/no-unused-vars
  let nodes = $state<SimNode[]>([]);
  let links = $state<SimLink[]>([]);
  let loading = $state(true);
  let sim: Simulation<SimNode, SimLink> | null = null;
  let tick = $state(0); // bumped each simulation tick to drive reactive re-render

  // Re-derive node positions on every simulation tick so the SVG circles/labels
  // follow the layout (d3-force mutates node objects in place, which Svelte's
  // fine-grained reactivity does not observe — the `tick` counter is the signal).
  const positionedNodes = $derived.by(() => {
    void tick;
    return nodes.map((n) => ({ ...n, x: n.x ?? 0, y: n.y ?? 0 }));
  });

  onMount(() => void recenter(entityId, entityKind));
  onDestroy(() => sim?.stop());

  async function recenter(id: string, kind: string) {
    loading = true;
    centerId = id;
    centerKind = kind;
    try {
      const g: EntityGraph = await getEntityGraph(id, kind as EntityKind, 1);
      buildSimulation(g);
    } catch (e) {
      console.error('Failed to load graph:', e);
      nodes = []; links = [];
    } finally {
      loading = false;
    }
  }

  function buildSimulation(g: EntityGraph) {
    sim?.stop();
    nodes = g.nodes.map((n) => ({ ...n }));
    links = g.edges.map((e) => ({ source: e.from_id, target: e.to_id, rel_type: e.rel_type }));
    sim = forceSimulation<SimNode, SimLink>(nodes)
      .force('charge', forceManyBody().strength(-280))
      .force('center', forceCenter(width / 2, height / 2))
      .force('collide', forceCollide(28))
      .force('link', forceLink<SimNode, SimLink>(links)
        .id((d) => d.id).distance(110))
      .on('tick', () => (tick += 1));
  }

  function nodeById(id: string): SimNode | undefined {
    void tick; // reactive dependency
    return nodes.find((n) => n.id === id);
  }

  // After d3-force runs, link.source/target mutate from string ids to node objects.
  // Resolve back to string id regardless.
  function linkEndId(end: string | SimNode): string {
    return typeof end === 'string' ? end : end.id;
  }
</script>

<div class="graph-wrap" data-testid="entity-graph">
  {#if onClose}
    <button class="close" onclick={onClose} aria-label="Close graph">✕</button>
  {/if}
  {#if loading}
    <p class="muted">Loading graph…</p>
  {:else if nodes.length <= 1}
    <p class="muted" data-testid="graph-empty">No relationships yet for this entity.</p>
  {/if}
  <svg {width} {height} role="img" aria-label="Entity relationship graph">
    {#each links as l (`${linkEndId(l.source)}->${linkEndId(l.target)}`)}
      {@const a = nodeById(linkEndId(l.source))}
      {@const b = nodeById(linkEndId(l.target))}
      {#if a && b}
        <line x1={a.x ?? 0} y1={a.y ?? 0} x2={b.x ?? 0} y2={b.y ?? 0} class="edge" />
        <text x={((a.x ?? 0) + (b.x ?? 0)) / 2} y={((a.y ?? 0) + (b.y ?? 0)) / 2}
          class="edge-label">{l.rel_type}</text>
      {/if}
    {/each}
    {#each positionedNodes as n (n.id)}
      <g class="node" data-id={n.id} data-name={n.name}>
        <circle cx={n.x} cy={n.y} r={n.id === centerId ? 16 : 10} fill={kindColor(n.kind)} />
        <text x={n.x} y={n.y + 26} class="node-label" text-anchor="middle">{n.name}</text>
      </g>
    {/each}
  </svg>
</div>

<style>
  .graph-wrap { position: relative; }
  .close { position: absolute; top: 8px; right: 8px; }
  .edge { stroke: var(--line); stroke-width: 1.5; }
  .edge-label { fill: var(--fg-3); font-size: 10px; text-anchor: middle; }
  .node-label { fill: var(--fg-2); font-size: 11px; }
  .muted { color: var(--fg-3); }
</style>
