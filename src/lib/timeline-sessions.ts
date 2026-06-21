import type { GraphNode, Session } from './commands';

export interface SessionLane {
  session: Session | null; // null = the trailing "Unscheduled" lane
  events: GraphNode[];
}

/**
 * Lay out events into one lane per session (in the given session order),
 * matching `event.session_id` to `session.id`. Events with no `session_id`
 * (or a dangling one) collect into a trailing `session: null` lane, which is
 * omitted when empty.
 */
export function groupBySession(sessions: Session[], events: GraphNode[]): SessionLane[] {
  const lanes: SessionLane[] = sessions.map((s) => ({ session: s, events: [] }));
  const byId = new Map(sessions.map((s, i) => [s.id, lanes[i]]));
  const unscheduled: GraphNode[] = [];
  for (const e of events) {
    const lane = e.session_id ? byId.get(e.session_id) : undefined;
    if (lane) lane.events.push(e);
    else unscheduled.push(e);
  }
  if (unscheduled.length > 0) lanes.push({ session: null, events: unscheduled });
  return lanes;
}
