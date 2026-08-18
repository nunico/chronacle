import { render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { landingCopy } from '$lib/i18n/landing-copy';
import ProductExample from '$lib/landing/ProductExample.svelte';

const legalPath = '/legal/open-game-license';

// Named Product Identity from the official SRD 5.1 legal information.
const prohibitedProductIdentityTerms = [
  'Dungeons & Dragons',
  `D${'&D'}`,
  'Player’s Handbook',
  `Dungeon${' Master'}`,
  'Monster Manual',
  'd20',
  'd20 System',
  'Forgotten Realms',
  'Faerûn',
  'Wizards of the Coast',
  'Underdark',
  'Red Wizard of Thay',
  'the City of Union',
  'Heroic Domains of Ysgard',
  'Ever-Changing Chaos of Limbo',
  'Windswept Depths of Pandemonium',
  'Infinite Layers of the Abyss',
  'Tarterian Depths of Carceri',
  'Gray Waste of Hades',
  'Bleak Eternity of Gehenna',
  'Nine Hells of Baator',
  'Infernal Battlefield of Acheron',
  'Clockwork Nirvana of Mechanus',
  'Peaceable Kingdoms of Arcadia',
  'Seven Mounting Heavens of Celestia',
  'Twin Paradises of Bytopia',
  'Blessed Fields of Elysium',
  'Wilderness of the Beastlands',
  'Olympian Glades of Arborea',
  'Concordant Domain of the Outlands',
  'Sigil',
  'Lady of Pain',
  'Book of Exalted Deeds',
  'Book of Vile Darkness',
  'beholder',
  'gauth',
  'carrion crawler',
  'tanar’ri',
  'baatezu',
  'displacer beast',
  'githyanki',
  'githzerai',
  'mind flayer',
  'illithid',
  'umber hulk',
  'yuan-ti',
] as const;

describe('SRD Open Game Content treatment', () => {
  it.each(['en', 'de'] as const)('marks and links the %s example', (locale) => {
    const { container } = render(ProductExample, {
      copy: landingCopy[locale].productExample,
    });
    const markedContent = [...container.querySelectorAll('[data-open-game-content]')];

    expect(markedContent).toHaveLength(1);
    for (const element of markedContent) {
      const marker = within(element as HTMLElement).getByRole('link', {
        name: 'SRD 5.1 example · Open Game Content',
      });
      expect(marker).toBeVisible();
      expect(marker).toHaveAttribute('href', legalPath);
    }
  });

  it.each(['en', 'de'] as const)('keeps Product Identity out of the %s example', (locale) => {
    render(ProductExample, { copy: landingCopy[locale].productExample });
    const exampleText = screen.getByRole('group').textContent ?? '';

    for (const term of prohibitedProductIdentityTerms) {
      expect(exampleText.toLocaleLowerCase('en-US')).not.toContain(term.toLocaleLowerCase('en-US'));
    }
  });
});
