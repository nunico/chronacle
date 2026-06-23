<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
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
    onClose?: () => void;
    onOpenEntity?: (node: GraphNodeRef) => void; // click-through: open in the manager
  }
  const { entityId, entityKind, onClose, onOpenEntity }: Props = $props();

  type SimNode = GraphNodeRef & SimulationNodeDatum;
  // d3-force mutates source/target from string ids to node objects after simulation starts.
  interface SimLink extends SimulationLinkDatum<SimNode> { source: string | SimNode; target: string | SimNode; rel_type: string; }

  // Seeded once from the entityId prop so the center node is sized correctly on first
  // render; thereafter centerId diverges as the user re-centers the graph. untrack makes
  // the one-time capture explicit (silences Svelte's state_referenced_locally warning).
  let centerId = $state(untrack(() => entityId));
  let nodes = $state<SimNode[]>([]);
  let links = $state<SimLink[]>([]);
  let loading = $state(true);
  let sim: Simulation<SimNode, SimLink> | null = null;
  let tick = $state(0); // bumped each simulation tick to drive reactive re-render

  // SVG element reference — captured via bind:this to avoid document.querySelector in the hot path.
  let svgEl: SVGSVGElement | undefined = $state();

  // Responsive container measurement via Svelte bind:clientWidth/bind:clientHeight.
  // Fallback dims are used until the first measurement arrives; the simulation is
  // rebuilt once real dims are known (see $effect below).
  let containerWidth = $state(720);
  let containerHeight = $state(520);
  let dimsReady = $state(false);

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

  // When container dims become available for the first time, rebuild the sim so
  // forceCenter uses the actual panel size rather than the 720×520 fallback.
  $effect(() => {
    if (dimsReady && sim) {
      sim.force('center', forceCenter(containerWidth / 2, containerHeight / 2));
      sim.alpha(0.3).restart();
    }
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
    // Use measured dims if already available, otherwise fall back to defaults.
    const cx = dimsReady ? containerWidth / 2 : 360;
    const cy = dimsReady ? containerHeight / 2 : 260;
    sim = forceSimulation<SimNode, SimLink>(nodes)
      .force('charge', forceManyBody().strength(-280))
      .force('center', forceCenter(cx, cy))
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
      // Map screen coords to graph coords: graph = (screen - pan) / zoom.
      // The SVG transform is translate(pan) scale(zoom); node groups are placed
      // at graph coords and counter-scaled by 1/zoom for constant screen size.
      // This inverse mapping stays correct: the node origin renders at fx*zoom + pan
      // (graph coord scaled by parent zoom + pan offset) = cursor screen position.
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

  // ── Wheel zoom (zoom-to-cursor) ─────────────────────────────────────────────

  function onWheel(event: WheelEvent) {
    event.preventDefault();
    if (!svgEl) return;
    const rect = svgEl.getBoundingClientRect();
    const cx = event.clientX - rect.left;   // cursor in SVG pixel space
    const cy = event.clientY - rect.top;
    const delta = event.deltaY > 0 ? 0.9 : 1.1;
    const oldZoom = zoom;
    const newZoom = Math.min(2.5, Math.max(0.4, oldZoom * delta));
    // Keep the graph-space point under the cursor fixed.
    // Under semantic zoom: screen = graphCoord * zoom + pan.
    // To keep screen position cx,cy fixed: cx = g*newZoom + newPan
    // where g = (cx - pan.x) / oldZoom. Solving: newPan = cx - (cx - pan.x)*(newZoom/oldZoom).
    pan = {
      x: cx - (cx - pan.x) * (newZoom / oldZoom),
      y: cy - (cy - pan.y) * (newZoom / oldZoom),
    };
    zoom = newZoom;
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

  // Called by Svelte when container dimensions change (bind:clientWidth/clientHeight).
  function onDimsChange() {
    if (containerWidth > 0 && containerHeight > 0) {
      dimsReady = true;
    }
  }
</script>

<!--
  Outer div measures the available space via bind:clientWidth/bind:clientHeight.
  The SVG fills this space (width/height set to measured values) so the graph
  uses the full panel instead of a fixed 720×520 viewport.
-->
<div
  class="graph-wrap"
  data-testid="entity-graph"
  bind:clientWidth={containerWidth}
  bind:clientHeight={containerHeight}
  onresize={onDimsChange}
>
  {#if onClose}
    <button class="close-btn" onclick={onClose} aria-label="Close graph" data-autofocus>✕</button>
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
    width={containerWidth}
    height={containerHeight}
    role="application"
    aria-label="Entity relationship graph"
    onpointerdown={onCanvasPointerDown}
    onwheel={onWheel}
    style="cursor: grab; display: block;"
  >
    <!--
      Composited zoom transform: both pan and zoom live in the SVG transform so
      the GPU composites the entire layer rather than repainting individual SVG
      attributes. This eliminates the WebKit paint-invalidation race that caused
      node circles and labels to appear at different positions within a single
      frame (ghosting / flickering).

      All children use RAW GRAPH coordinates — no `* zoom` anywhere in the markup.

      Coordinate mapping: screen = graphCoord * zoom + pan
      Inverse (used in drag): graphCoord = (screen - pan) / zoom

      Node size is kept constant by counter-scaling each node group by 1/zoom:
        net scale on node contents = zoom (parent) * (1/zoom) (node) = 1
      while the node's POSITION scales with `zoom` so nodes spread apart on zoom-in.

      Edges use vector-effect="non-scaling-stroke" to keep stroke width constant.
      Edge labels are also wrapped in a 1/zoom counter-scale group at the midpoint.
    -->
    <g transform={`translate(${pan.x},${pan.y}) scale(${zoom})`}>
      {#each links as l (`${linkEndId(l.source)}->${linkEndId(l.target)}`)}
        {@const a = nodeById(linkEndId(l.source))}
        {@const b = nodeById(linkEndId(l.target))}
        {#if a && b}
          <line
            x1={a.x ?? 0}
            y1={a.y ?? 0}
            x2={b.x ?? 0}
            y2={b.y ?? 0}
            class="edge"
            vector-effect="non-scaling-stroke"
          />
          <!-- Counter-scale the label at the midpoint so it stays constant screen size. -->
          <g transform={`translate(${((a.x ?? 0) + (b.x ?? 0)) / 2},${((a.y ?? 0) + (b.y ?? 0)) / 2}) scale(${1 / zoom})`}>
            <text x={0} y={0} class="edge-label">{l.rel_type}</text>
          </g>
        {/if}
      {/each}
      {#each positionedNodes as n (n.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!--
          Node group is translated to graph coords; its contents are counter-scaled
          so they appear at constant screen size. The group origin (0,0 in local
          space) is the node center — circle is at cx=0,cy=0; offsets for label and
          expand button are constant local pixels (not scaled by zoom).
        -->
        <g
          class="node"
          data-id={n.id}
          data-name={n.name}
          transform={`translate(${n.x},${n.y}) scale(${1 / zoom})`}
          style="cursor: pointer;"
          onpointerdown={(e) => onNodePointerDown(e, n.id)}
          onclick={(e) => onNodeClick(e, n.id)}
        >
          <!-- Node circle: cx/cy at local origin (the node center). -->
          <circle
            cx={0}
            cy={0}
            r={n.id === centerId ? 16 : 10}
            class={n.id === centerId ? 'node-circle node-circle--center' : 'node-circle'}
            fill={kindColor(n.kind)}
          />
          <!-- Name label: offset below the node center in constant local px. -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <text
            x={0}
            y={n.id === centerId ? 30 : 24}
            class="node-label"
            text-anchor="middle"
            onpointerdown={(e) => e.stopPropagation()}
            onclick={(e) => { e.stopPropagation(); onOpenEntity?.(n); }}
            style="cursor: pointer;"
          >{n.name}</text>
          <!-- Expand affordance: constant-size button at fixed local offset from node center. -->
          {#if n.id !== centerId}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <g
              class="expand-btn"
              aria-label="Expand neighbours"
              onpointerdown={(e) => e.stopPropagation()}
              onclick={(e) => { e.stopPropagation(); void expand(n.id, n.kind); }}
              style="cursor: pointer;"
            >
              <title>Expand neighbours</title>
              <circle
                cx={14}
                cy={-14}
                r={9}
                class="expand-circle"
              />
              <text
                x={14}
                y={-14}
                class="expand-glyph"
                text-anchor="middle"
                dominant-baseline="central"
              >＋</text>
            </g>
          {/if}
        </g>
      {/each}
    </g>
  </svg>
</div>

<style>
  .graph-wrap {
    position: relative;
    width: 100%;
    height: 100%;
    /* Ensure the wrapper fills its parent so bind:clientWidth/clientHeight
       capture the full panel dimensions. */
    display: flex;
    flex-direction: column;
  }

  /* ── Close button ──────────────────────────────────────────────────────────── */
  .close-btn {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-sm);
    background: var(--bg-panel-2);
    color: var(--fg-3);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition: background var(--dur-fast), color var(--dur-fast), border-color var(--dur-fast);
    z-index: 1;
  }
  .close-btn:hover {
    background: var(--bg-inset);
    border-color: var(--line-glow);
    color: var(--fg-1);
  }
  .close-btn:focus-visible {
    outline: none;
    box-shadow: var(--glow-focus);
  }

  /* ── Edges ─────────────────────────────────────────────────────────────────── */
  .edge {
    stroke: var(--line-strong);
    stroke-width: 1.5;
  }
  .edge-label {
    fill: var(--fg-3);
    font-size: 9px;
    font-family: var(--font-mono);
    text-anchor: middle;
    pointer-events: none;
    letter-spacing: 0.02em;
  }

  /* ── Node circles ──────────────────────────────────────────────────────────── */
  .node-circle {
    stroke: rgba(124, 148, 255, 0.35);
    stroke-width: 1.5;
    filter: drop-shadow(0 0 4px rgba(91, 120, 255, 0.25));
  }
  .node-circle--center {
    stroke: var(--violet-400);
    stroke-width: 2.5;
    filter: drop-shadow(0 0 8px rgba(123, 92, 255, 0.55));
  }

  /* ── Node labels ───────────────────────────────────────────────────────────── */
  .node-label {
    fill: var(--fg-2);
    font-size: 11px;
    font-family: var(--font-sans);
    font-weight: 500;
    pointer-events: all;
    /* Subtle halo for readability over edges */
    paint-order: stroke fill;
    stroke: var(--bg-panel);
    stroke-width: 3px;
    stroke-linejoin: round;
  }
  .node-label:hover {
    fill: var(--violet-400);
  }

  /* ── Expand button (circular affordance) ───────────────────────────────────── */
  .expand-btn {
    opacity: 0.5;
    transition: opacity 0.15s;
  }
  .expand-btn:hover {
    opacity: 1;
  }
  .expand-circle {
    fill: var(--bg-panel-2);
    stroke: var(--violet-400);
    stroke-width: 1.5;
  }
  .expand-btn:hover .expand-circle {
    fill: var(--violet-500);
    stroke: var(--violet-300);
    filter: drop-shadow(0 0 4px rgba(123, 92, 255, 0.6));
  }
  .expand-glyph {
    fill: var(--fg-2);
    font-size: 12px;
    font-family: var(--font-sans);
    pointer-events: none;
    user-select: none;
  }
  .expand-btn:hover .expand-glyph {
    fill: var(--fg-on-accent);
  }

  .muted { color: var(--fg-3); font-family: var(--font-sans); }
</style>
