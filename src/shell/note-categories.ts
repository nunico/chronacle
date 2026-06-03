export type NoteCategoryId =
  | 'sessions'
  | 'player_characters'
  | 'npcs'
  | 'locations'
  | 'factions'
  | 'creatures'
  | 'items'
  | 'events'
  | 'misc';

export interface NoteCategory {
  id: NoteCategoryId;
  label: string;
  icon: string; // Lucide kebab-case name
  group: 'Notebook' | 'Entities';
  folder: string;
  sub: string;
}

export const NOTE_CATEGORIES: NoteCategory[] = [
  {
    id: 'sessions',
    label: 'Sessions',
    icon: 'history',
    group: 'Notebook',
    folder: 'sessions',
    sub: 'Your campaign timeline — recaps, rewards, and open threads.',
  },
  {
    id: 'player_characters',
    label: 'Player Characters',
    icon: 'users-round',
    group: 'Entities',
    folder: 'entities/player_characters',
    sub: 'The party — sheets, hooks, and where each one stands.',
  },
  {
    id: 'npcs',
    label: 'NPCs',
    icon: 'drama',
    group: 'Entities',
    folder: 'entities/npcs',
    sub: "Everyone the party has met, and a few they haven't yet.",
  },
  {
    id: 'locations',
    label: 'Locations',
    icon: 'map-pin',
    group: 'Entities',
    folder: 'entities/locations',
    sub: "Places your party has been — and the ones they're avoiding.",
  },
  {
    id: 'factions',
    label: 'Factions',
    icon: 'flag',
    group: 'Entities',
    folder: 'entities/factions',
    sub: 'The powers moving behind your campaign.',
  },
  {
    id: 'creatures',
    label: 'Creatures',
    icon: 'paw-print',
    group: 'Entities',
    folder: 'entities/creatures',
    sub: 'Beasts and horrors stalking the world.',
  },
  {
    id: 'items',
    label: 'Items',
    icon: 'gem',
    group: 'Entities',
    folder: 'entities/items',
    sub: 'Artifacts, relics, and loot worth noting.',
  },
  {
    id: 'events',
    label: 'Events',
    icon: 'milestone',
    group: 'Entities',
    folder: 'entities/events',
    sub: 'The moments that shaped the campaign.',
  },
  {
    id: 'misc',
    label: 'Misc',
    icon: 'shapes',
    group: 'Entities',
    folder: 'entities/misc',
    sub: 'Everything else worth keeping.',
  },
];

export function findCategory(id: NoteCategoryId): NoteCategory {
  const cat = NOTE_CATEGORIES.find((c) => c.id === id);
  if (!cat) throw new Error(`Unknown note category: ${id}`);
  return cat;
}
