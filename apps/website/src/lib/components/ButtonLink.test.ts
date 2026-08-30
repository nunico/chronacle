import { render, screen } from '@testing-library/svelte';
import { createRawSnippet, type ComponentProps } from 'svelte';
import { describe, expect, it } from 'vitest';
import ButtonLink from './ButtonLink.svelte';

const children = createRawSnippet(() => ({
  render: () => '<span>Action</span>',
}));

describe('ButtonLink', () => {
  it('requires internal href values to be known pathnames', () => {
    // @ts-expect-error Internal links must use a generated SvelteKit pathname.
    const invalidInternal: ComponentProps<typeof ButtonLink> = {
      href: 'not-an-internal-path',
      children,
    };

    expect(invalidInternal.href).toBe('not-an-internal-path');
  });

  it('keeps an internal link rel value untouched', () => {
    render(ButtonLink, {
      href: '/en/manual',
      rel: ' nofollow  ugc ',
      children,
    });

    expect(screen.getByRole('link', { name: 'Action' })).toHaveAttribute('rel', ' nofollow  ugc ');
  });

  it('merges and deduplicates mandatory external rel tokens', () => {
    render(ButtonLink, {
      href: 'https://example.com/download',
      rel: '  nofollow  ugc external nofollow ',
      external: true,
      children,
    });

    expect(screen.getByRole('link', { name: 'Action' })).toHaveAttribute(
      'rel',
      'nofollow ugc external noopener noreferrer',
    );
  });
});
