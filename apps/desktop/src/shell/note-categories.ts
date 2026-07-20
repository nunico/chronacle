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
  labelKey: `noteCategories.${NoteCategoryId}.label`;
  icon: string; // Lucide kebab-case name
  group: 'Notebook' | 'Entities';
  folder: string;
  subKey: `noteCategories.${NoteCategoryId}.description`;
}

export const NOTE_CATEGORIES: NoteCategory[] = [
  {
    id: 'sessions',
    labelKey: 'noteCategories.sessions.label',
    icon: 'history',
    group: 'Notebook',
    folder: 'sessions',
    subKey: 'noteCategories.sessions.description',
  },
  {
    id: 'player_characters',
    labelKey: 'noteCategories.player_characters.label',
    icon: 'users-round',
    group: 'Entities',
    folder: 'entities/player_characters',
    subKey: 'noteCategories.player_characters.description',
  },
  {
    id: 'npcs',
    labelKey: 'noteCategories.npcs.label',
    icon: 'drama',
    group: 'Entities',
    folder: 'entities/npcs',
    subKey: 'noteCategories.npcs.description',
  },
  {
    id: 'locations',
    labelKey: 'noteCategories.locations.label',
    icon: 'map-pin',
    group: 'Entities',
    folder: 'entities/locations',
    subKey: 'noteCategories.locations.description',
  },
  {
    id: 'factions',
    labelKey: 'noteCategories.factions.label',
    icon: 'flag',
    group: 'Entities',
    folder: 'entities/factions',
    subKey: 'noteCategories.factions.description',
  },
  {
    id: 'creatures',
    labelKey: 'noteCategories.creatures.label',
    icon: 'paw-print',
    group: 'Entities',
    folder: 'entities/creatures',
    subKey: 'noteCategories.creatures.description',
  },
  {
    id: 'items',
    labelKey: 'noteCategories.items.label',
    icon: 'gem',
    group: 'Entities',
    folder: 'entities/items',
    subKey: 'noteCategories.items.description',
  },
  {
    id: 'events',
    labelKey: 'noteCategories.events.label',
    icon: 'milestone',
    group: 'Entities',
    folder: 'entities/events',
    subKey: 'noteCategories.events.description',
  },
  {
    id: 'misc',
    labelKey: 'noteCategories.misc.label',
    icon: 'shapes',
    group: 'Entities',
    folder: 'entities/misc',
    subKey: 'noteCategories.misc.description',
  },
];

export function findCategory(id: NoteCategoryId): NoteCategory {
  const cat = NOTE_CATEGORIES.find((c) => c.id === id);
  if (!cat) throw new Error(`Unknown note category: ${id}`);
  return cat;
}
