import type { EntityKind } from './commands';

/** Node fill per entity kind (aligned with the graph mockup palette). */
export const KIND_COLOR: Record<EntityKind, string> = {
  npc: '#6699cc',
  location: '#99aa88',
  faction: '#cc6699',
  creature: '#cc9966',
  item: '#cc66cc',
  event: '#66cc99',
  player_character: '#ffcc66',
  misc: '#8899aa',
};

export function kindColor(kind: string): string {
  return (KIND_COLOR as Record<string, string>)[kind] ?? '#8899aa';
}
