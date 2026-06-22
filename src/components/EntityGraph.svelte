<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    forceSimulation, forceManyBody, forceLink, forceCenter, forceCollide,
    type Simulation, type SimulationNodeDatum, type SimulationLinkDatum,
  } from 'd3-force';
  import { getEntityGraph, type EntityGraph, type GraphNodeRef, type EntityKind } from '../lib/commands';
  import { kindColor } from '../lib/graph-colors';
  import { mergeGraph } from '../lib/graph-merge';

  interface Props {
    entityId: string;
    entityKind: string;
    width?: number;
    height?: number;
    onClose?: () => void;
    onOpenEntity?: (node: GraphNodeRef) => void; // click-through: open in the manager
  }
  const { entityId, entityKind, width = 720, height = 520, onClose, onOpenEntity }: Props = $props();

  type SimNode = GraphNodeRef & SimulationNodeDatum;
  // d3-force mutates source/target from string ids to node objects after simulation starts.
  interface SimLink extends SimulationLinkDatum<SimNode> { source: string | SimNode; target: string | SimNode; rel_type: string; }

  // Initialized from entityId prop so the center node is sized correctly on first render.
  let centerId = $state(entityId);
  let nodes = $state<SimNode[]>([]);
  let links = $state<SimLink[]>([]);
  let loading = $state(true);
  let sim: Simulation<SimNode, SimLink> | null = null;
  let tick = $state(0); // bumped each simulation tick to drive reactive re-render

  // SVG element reference — captured via bind:this to avoid document.querySelector in the hot path.
  let svgEl: SVGSVGElement | undefined = $state();

  // Pan/zoom state
  let pan = $state({ x: 0, y: 0 });
  let zoom = $state(1);

  // Drag tracking: distinguish a click from a drag on a node.
  // A drag suppresses the re-center click handler.
  let dragNode: SimNode | null = null;
  let dragStartX = 0;
  let dragStartY = 0;
  let wasDrag = false; // set true once pointer moves > 5px from pointerdown origin

  // Canvas pan tracking (drag on empty canvas area)
  let panDragging = false;
  let panStartX = 0;
  let panStartY = 0;
  let panOriginX = 0;
  let panOriginY = 0;

  // Re-derive node positions on every simulation tick so the SVG circles/labels
  // follow the layout (d3-force mutates node objects in place, which Svelte's
  // fine-grained reactivity does not observe — the `tick` counter is the signal).
  const positionedNodes = $derived.by(() => {
    void tick;
    return nodes.map((n) => ({ ...n, x: n.x ?? 0, y: n.y ?? 0 }));
  });

  onMount(() => void recenter(entityId, entityKind));
  onDestroy(() => {
    sim?.stop();
    removeWindowListeners();
  });

  async function recenter(id: string, kind: string) {
    loading = true;
    centerId = id;
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

  async function expand(id: string, kind: string) {
    try {
      const g = await getEntityGraph(id, kind as EntityKind, 1);
      // Reconstruct the current EntityGraph shape from live sim state.
      // links.source/target may be mutated to SimNode objects by d3-force.
      const currentEdges = links.map((l) => ({
        from_id: linkEndId(l.source),
        from_kind: '',
        to_id: linkEndId(l.target),
        to_kind: '',
        rel_type: l.rel_type,
        notes: null as null,
      }));
      const currentGraph: EntityGraph = {
        nodes: nodes.map((n) => ({ id: n.id, kind: n.kind, name: n.name })),
        edges: currentEdges,
      };
      const merged = mergeGraph(currentGraph, g);
      // Warm-start: copy current x/y/fx/fy into merged nodes so already-positioned
      // nodes don't jump and re-settle. Newly-added neighbors (no posMap entry) start fresh.
      const posMap = new Map(nodes.map((n) => [n.id, { x: n.x, y: n.y, fx: n.fx, fy: n.fy }]));
      for (const mn of merged.nodes) {
        const pos = posMap.get(mn.id);
        if (pos) Object.assign(mn, pos);
      }
      buildSimulation(merged);
    } catch (e) {
      console.error('expand failed', e);
    }
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

  // Look up the mutable SimNode (not the snapshot copy in positionedNodes) by id.
  // positionedNodes are snapshot copies; fx/fy must be set on the live node in nodes[].
  function liveNode(id: string): SimNode | undefined {
    return nodes.find((x) => x.id === id);
  }

  // ── Node drag-to-pin ────────────────────────────────────────────────────────

  function onNodePointerDown(event: PointerEvent, id: string) {
    event.stopPropagation(); // prevent canvas-pan handler from firing
    if (dragNode || panDragging) return; // re-entry guard: ignore second pointerdown mid-drag
    const n = liveNode(id);
    if (!n) return;
    dragNode = n;
    dragStartX = event.clientX;
    dragStartY = event.clientY;
    wasDrag = false;
    n.fx = n.x;
    n.fy = n.y;
    sim?.alphaTarget(0.3).restart();
    window.addEventListener('pointermove', onWindowPointerMove);
    window.addEventListener('pointerup', onWindowPointerUp);
  }

  function onWindowPointerMove(event: PointerEvent) {
    if (dragNode) {
      const dx = event.clientX - dragStartX;
      const dy = event.clientY - dragStartY;
      if (Math.abs(dx) > 5 || Math.abs(dy) > 5) wasDrag = true;
      // Map screen coords to SVG coords accounting for pan/zoom.
      // Uses the bound svgEl reference instead of document.querySelector to avoid
      // a costly DOM traversal on every pointermove (60+ Hz).
      if (svgEl) {
        const rect = svgEl.getBoundingClientRect();
        dragNode.fx = (event.clientX - rect.left - pan.x) / zoom;
        dragNode.fy = (event.clientY - rect.top - pan.y) / zoom;
      }
    } else if (panDragging) {
      pan = {
        x: panOriginX + (event.clientX - panStartX),
        y: panOriginY + (event.clientY - panStartY),
      };
    }
  }

  function onWindowPointerUp() {
    if (dragNode) {
      sim?.alphaTarget(0);
      // Leave fx/fy set (node remains pinned at dropped position).
      dragNode = null;
    }
    if (panDragging) {
      panDragging = false;
    }
    removeWindowListeners();
  }

  function removeWindowListeners() {
    window.removeEventListener('pointermove', onWindowPointerMove);
    window.removeEventListener('pointerup', onWindowPointerUp);
  }

  // ── Canvas pan ──────────────────────────────────────────────────────────────

  function onCanvasPointerDown(event: PointerEvent) {
    // Only initiate canvas pan when clicking on the SVG background itself,
    // not on a node (node handlers call stopPropagation).
    if (panDragging || dragNode) return; // re-entry guard: ignore second pointerdown mid-pan
    panDragging = true;
    panStartX = event.clientX;
    panStartY = event.clientY;
    panOriginX = pan.x;
    panOriginY = pan.y;
    window.addEventListener('pointermove', onWindowPointerMove);
    window.addEventListener('pointerup', onWindowPointerUp);
  }

  // ── Wheel zoom ──────────────────────────────────────────────────────────────

  function onWheel(event: WheelEvent) {
    event.preventDefault();
    const delta = event.deltaY > 0 ? 0.9 : 1.1;
    zoom = Math.min(2.5, Math.max(0.4, zoom * delta));
  }

  // ── Node click (re-center) — fires if it wasn't a drag ─────────────────────

  function onNodeClick(event: MouseEvent, id: string) {
    if (wasDrag) {
      // The preceding pointerdown+move was a drag; don't re-center.
      wasDrag = false;
      return;
    }
    event.stopPropagation();
    const n = liveNode(id);
    if (n) void recenter(n.id, n.kind);
  }
</script>

<div class="graph-wrap" data-testid="entity-graph">
  {#if onClose}
    <button class="close" onclick={onClose} aria-label="Close graph" data-autofocus>✕</button>
  {/if}
  {#if loading}
    <p class="muted">Loading graph…</p>
  {:else if nodes.length <= 1}
    <p class="muted" data-testid="graph-empty">No relationships yet for this entity.</p>
  {/if}
  <!--
    role="application" — this SVG is an interactive widget (pan, zoom, drag),
    not a static image; aria-label describes its content.
  -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <svg
    bind:this={svgEl}
    {width}
    {height}
    role="application"
    aria-label="Entity relationship graph"
    onpointerdown={onCanvasPointerDown}
    onwheel={onWheel}
    style="cursor: grab; display: block;"
  >
    <g transform={`translate(${pan.x},${pan.y}) scale(${zoom})`}>
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
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <g
          class="node"
          data-id={n.id}
          data-name={n.name}
          style="cursor: pointer;"
          onpointerdown={(e) => onNodePointerDown(e, n.id)}
          onclick={(e) => onNodeClick(e, n.id)}
        >
          <circle cx={n.x} cy={n.y} r={n.id === centerId ? 16 : 10} fill={kindColor(n.kind)} />
          <!-- Name label: clicking opens the entity, does NOT re-center -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <text
            x={n.x}
            y={n.y + 26}
            class="node-label"
            text-anchor="middle"
            onpointerdown={(e) => e.stopPropagation()}
            onclick={(e) => { e.stopPropagation(); onOpenEntity?.(n); }}
            style="cursor: pointer;"
          >{n.name}</text>
          <!-- Expand affordance: fetches neighbor graph and merges it in -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <text
            x={n.x + (n.id === centerId ? 18 : 12)}
            y={n.y - (n.id === centerId ? 14 : 8)}
            class="expand-btn"
            onpointerdown={(e) => e.stopPropagation()}
            onclick={(e) => { e.stopPropagation(); void expand(n.id, n.kind); }}
            style="cursor: pointer;"
          >＋</text>
        </g>
      {/each}
    </g>
  </svg>
</div>

<style>
  .graph-wrap { position: relative; }
  .close { position: absolute; top: 8px; right: 8px; }
  .edge { stroke: var(--line); stroke-width: 1.5; }
  .edge-label { fill: var(--fg-3); font-size: 10px; text-anchor: middle; pointer-events: none; }
  .node-label { fill: var(--fg-2); font-size: 11px; }
  .node-label:hover { fill: var(--violet-400, #a78bfa); }
  .expand-btn { fill: var(--fg-3); font-size: 13px; user-select: none; }
  .expand-btn:hover { fill: var(--violet-400, #a78bfa); }
  .muted { color: var(--fg-3); }
</style>
