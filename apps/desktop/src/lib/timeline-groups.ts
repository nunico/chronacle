import type { GraphNode } from './commands';

export interface EraGroup {
  era: string | null; // null = the "Unordered / no era" trailing bucket
  events: GraphNode[];
}

/**
 * Group already-timeline-ordered events into runs by `era`, preserving order.
 * Events keep the order the backend returned (sequence_index, nulls last); this
 * only buckets consecutive same-era events. Events with `era == null` form a
 * trailing `era: null` group.
 *
 * IMPORTANT — run-based, not merge-based: two events sharing the same era label
 * but separated by an event with a different era intentionally produce TWO
 * separate groups for that era (e.g. [A:Dawn, B:Dusk, C:Dawn] → three groups).
 * This preserves the canonical `sequence_index` ordering: we never reorder
 * events just to merge era labels.
 */
export function groupByEra(ordered: GraphNode[]): EraGroup[] {
  const groups: EraGroup[] = [];
  for (const e of ordered) {
    const era = e.era ?? null;
    const last = groups[groups.length - 1];
    if (last && last.era === era) last.events.push(e);
    else groups.push({ era, events: [e] });
  }
  return groups;
}
