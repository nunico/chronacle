import { describe, expect, it } from 'vitest';
import { buildWikiLinkEntityMap, normalizeWikiLinkKey } from './wikilinks';

type WikiLinkNode = Parameters<typeof buildWikiLinkEntityMap>[0][number];

const node = (overrides: Partial<WikiLinkNode>): WikiLinkNode => ({
  id: 'id1',
  kind: 'npc',
  name: 'The Moon Gates',
  aliases: [],
  ...overrides,
});

describe('wikilinks', () => {
  it('normalizes case, leading the, possessives, plurals, and punctuation', () => {
    expect(normalizeWikiLinkKey(" The Moon Gate's ")).toBe('moon gate');
    expect(normalizeWikiLinkKey('The Moon Gates')).toBe('moon gate');
    expect(normalizeWikiLinkKey('Moon--Gate')).toBe('moon gate');
  });

  it('indexes primary names and aliases by exact and normalized keys', () => {
    const map = buildWikiLinkEntityMap([
      node({
        id: 'loc1',
        kind: 'location',
        name: 'The Moon Gates',
        aliases: ['Selene Door'],
      }),
    ]);

    expect(map.get('the moon gates')).toEqual({
      id: 'loc1',
      kind: 'location',
    });
    expect(map.get('selene door')).toEqual({ id: 'loc1', kind: 'location' });
    expect(map.get('moon gate')).toEqual({ id: 'loc1', kind: 'location' });
  });

  it('drops colliding keys instead of picking a winner', () => {
    const map = buildWikiLinkEntityMap([
      node({ id: 'a', name: 'The Free League' }),
      node({ id: 'b', name: 'Free Leagues' }),
    ]);

    expect(map.has('free league')).toBe(false);
  });
});
