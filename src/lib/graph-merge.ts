import type { EntityGraph } from './commands';

// Edge dedup key is direction-sensitive: "A->B:knows" ≠ "B->A:knows".
// This matches the directed edges the backend returns (from_id is always the subject).
const edgeKey = (e: EntityGraph['edges'][number]) =>
  `${e.from_id}->${e.to_id}:${e.rel_type}`;

/** Union two graphs, deduping nodes by id and edges by (from, to, rel_type). */
export function mergeGraph(a: EntityGraph, b: EntityGraph): EntityGraph {
  const nodes = new Map(a.nodes.map((n) => [n.id, n]));
  for (const n of b.nodes) if (!nodes.has(n.id)) nodes.set(n.id, n);
  const edges = new Map(a.edges.map((e) => [edgeKey(e), e]));
  for (const e of b.edges) if (!edges.has(edgeKey(e))) edges.set(edgeKey(e), e);
  return { nodes: [...nodes.values()], edges: [...edges.values()] };
}
