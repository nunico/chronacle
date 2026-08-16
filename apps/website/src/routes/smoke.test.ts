import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Page from './+page.svelte';

describe('website root', () => {
  it('renders the Chronacle heading', () => {
    render(Page);

    expect(screen.getByRole('heading', { level: 1, name: /chronacle/i })).toBeInTheDocument();
  });
});
