import { render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { landingCopy } from '$lib/i18n/landing-copy';
import ProductExample from '$lib/landing/ProductExample.svelte';
import LegalPage from '../../routes/legal/open-game-license/+page.svelte';

const legalPath = '/legal/open-game-license';

// Regression guard for the names enumerated in the official SRD 5.1 Product Identity notice.
// This does not attempt to classify every possible form of Product Identity.
const enumeratedProductIdentityNames = [
  'Dungeons & Dragons',
  `D${'&D'}`,
  "Player's Handbook",
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
  "tanar'ri",
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

  it.each(['en', 'de'] as const)('marks only the derived %s answer content', (locale) => {
    const copy = landingCopy[locale].productExample;
    const { container } = render(ProductExample, { copy });
    const markedContent = container.querySelector('[data-open-game-content]');

    expect(markedContent).not.toBeNull();
    const marked = within(markedContent as HTMLElement);
    expect(marked.queryByText(copy.question)).not.toBeInTheDocument();
    expect(marked.queryByText(copy.assistant)).not.toBeInTheDocument();
    expect(markedContent?.querySelector('svg')).toBeNull();
    expect(marked.getByText(copy.verdict)).toBeVisible();
    expect(marked.getByText(copy.answer)).toBeVisible();
    expect(marked.getByText(copy.citation)).toBeVisible();
    expect(marked.getByText(copy.excerpt)).toBeVisible();
  });

  it.each(['en', 'de'] as const)(
    "keeps the official notice's enumerated names out of the entire %s example section",
    (locale) => {
      const { container } = render(ProductExample, {
        copy: landingCopy[locale].productExample,
      });
      const exampleText = container.querySelector('section')?.textContent ?? '';

      for (const name of enumeratedProductIdentityNames) {
        expect(exampleText.toLocaleLowerCase('en-US')).not.toContain(
          name.toLocaleLowerCase('en-US'),
        );
      }
    },
  );

  it('publishes the supplemental copyright notice separately from the official PDF', () => {
    render(LegalPage);

    expect(screen.getByRole('heading', { name: 'Supplemental COPYRIGHT NOTICE' })).toBeVisible();
    expect(
      screen.getByText('Chronacle website example Copyright 2026 Nico Nußbaum.'),
    ).toBeVisible();
    expect(screen.getByText(/separate from the unmodified official PDF/i)).toBeVisible();
  });
});
