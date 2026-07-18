import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import WikiText from './WikiText.svelte';

describe('WikiText', () => {
  it('renders plain text unchanged', () => {
    render(WikiText, {
      props: {
        text: 'Hello world, no wikilinks here.',
        entities: new Map(),
      },
    });
    expect(screen.getByText('Hello world, no wikilinks here.')).toBeTruthy();
  });

  it('renders entity badge for matched name', () => {
    const entities = new Map([['Torvin', { id: 'abc123', kind: 'npc' }]]);
    render(WikiText, {
      props: { text: '[[Torvin]] is here', entities },
    });
    const badge = screen.getByRole('button', { name: 'Torvin' });
    expect(badge).toBeTruthy();
    expect(badge.classList.contains('entity-badge')).toBe(true);
  });

  it('renders unmatched wikilink as literal text', () => {
    render(WikiText, {
      props: { text: '[[Unknown]] entity', entities: new Map() },
    });
    expect(screen.queryByRole('button')).toBeNull();
    // The span containing the text should include [[Unknown]]
    const container = screen.getByText(/\[\[Unknown\]\]/, { exact: false });
    expect(container).toBeTruthy();
  });

  it('calls onMissingLinkClick when an unmatched wikilink button is clicked', async () => {
    const onMissingLinkClick = vi.fn();
    render(WikiText, {
      props: {
        text: 'Go to [[Moon Gate]]',
        entities: new Map(),
        onMissingLinkClick,
      },
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Create article for Moon Gate' }));
    expect(onMissingLinkClick).toHaveBeenCalledWith('Moon Gate');
  });

  it('keeps unmatched wikilinks inert when no missing-link callback is provided', () => {
    render(WikiText, {
      props: { text: 'Go to [[Moon Gate]]', entities: new Map() },
    });

    expect(screen.queryByRole('button', { name: 'Create article for Moon Gate' })).toBeNull();
    expect(screen.getByText(/\[\[Moon Gate\]\]/)).toBeTruthy();
  });

  it('case-insensitive entity lookup', () => {
    const entities = new Map([['torvin', { id: 'abc123', kind: 'npc' }]]);
    render(WikiText, {
      props: { text: '[[Torvin]] appears', entities },
    });
    const badge = screen.getByRole('button', { name: 'Torvin' });
    expect(badge).toBeTruthy();
    expect(badge.classList.contains('entity-badge')).toBe(true);
  });

  it('multiple wikilinks in one text', () => {
    const entities = new Map([['Torvin', { id: 'abc123', kind: 'npc' }]]);
    render(WikiText, {
      props: { text: '[[Torvin]] meets [[Unknown]] at the tavern', entities },
    });
    // Torvin is matched — renders as badge
    expect(screen.getByRole('button', { name: 'Torvin' })).toBeTruthy();
    // Unknown is unmatched — renders as literal
    expect(screen.queryByRole('button', { name: 'Unknown' })).toBeNull();
    expect(screen.getByText(/\[\[Unknown\]\]/, { exact: false })).toBeTruthy();
  });

  it('onEntityClick called with correct id and kind when badge clicked', async () => {
    const onEntityClick = vi.fn();
    const entities = new Map([['Torvin', { id: 'abc123', kind: 'npc' }]]);
    render(WikiText, {
      props: { text: '[[Torvin]] is here', entities, onEntityClick },
    });
    const badge = screen.getByRole('button', { name: 'Torvin' });
    await fireEvent.click(badge);
    expect(onEntityClick).toHaveBeenCalledOnce();
    expect(onEntityClick).toHaveBeenCalledWith('abc123', 'npc');
  });
});
