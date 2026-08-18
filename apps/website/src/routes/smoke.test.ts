import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import appTemplate from '../app.html?raw';
import Page from './+page.svelte';

describe('website root', () => {
  it('renders the Chronacle heading', () => {
    render(Page);

    expect(screen.getByRole('heading', { level: 1, name: /chronacle/i })).toBeInTheDocument();
  });

  it('uses the approved Chronacle favicon instead of the Svelte scaffold logo', () => {
    expect(appTemplate).toContain('<link rel="icon" type="image/png" href="/brand/favicon.png" />');
    expect(appTemplate).not.toContain('favicon.svg');
    expect(appTemplate).not.toContain('svelte-logo');
  });
});
