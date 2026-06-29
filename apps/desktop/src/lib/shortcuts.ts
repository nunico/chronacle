// Keyboard-shortcut model — Vim-style g-chords for GM-at-the-table navigation.
//
// Pure, framework-free so it can be unit-tested without a DOM. The Shell owns
// the leader-key state machine and DOM wiring; this module owns the *mapping*
// (which key goes where) and the typing-suppression rule.

import type { NoteCategoryId } from '../shell/note-categories';

/// A resolved navigation destination from a `g`-chord.
export type NavTarget = 'oracle' | 'settings' | 'timeline' | { category: NoteCategoryId };

/// Second key of a `g`-chord → destination. `g o` → Oracle, `g n` → NPCs, …
const NAV_CHORDS: Record<string, NavTarget> = {
  o: 'oracle',
  t: 'timeline',
  p: { category: 'player_characters' },
  n: { category: 'npcs' },
  l: { category: 'locations' },
  f: { category: 'factions' },
  c: { category: 'creatures' },
  i: { category: 'items' },
  e: { category: 'events' },
  s: { category: 'sessions' },
  m: { category: 'misc' },
  ',': 'settings',
};

/// Resolve the second key of a `g`-chord to a destination, or null if unmapped.
/// Case-insensitive so Shift/Caps don't break navigation.
export function resolveNavChord(key: string): NavTarget | null {
  return NAV_CHORDS[key.toLowerCase()] ?? null;
}

/// True when the event target is a place the user is typing, so single-key and
/// chord shortcuts must be ignored (modifier combos are handled separately).
/// Covers inputs, textareas, selects, and any contenteditable host.
export function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || typeof el.tagName !== 'string') return false;
  const tag = el.tagName.toLowerCase();
  if (tag === 'input' || tag === 'textarea' || tag === 'select') return true;
  // `isContentEditable` is the real-browser signal; fall back to the attribute
  // so the check also holds under jsdom, which doesn't implement the property.
  if (el.isContentEditable === true) return true;
  const attr = el.getAttribute?.('contenteditable');
  return attr === '' || attr === 'true';
}

/// Rows for the `?` help overlay — single source of truth for documentation.
export const SHORTCUT_HELP: ReadonlyArray<{ keys: string; label: string }> = [
  { keys: 'g o', label: 'Oracle (chat)' },
  { keys: 'g t', label: 'Timeline' },
  { keys: 'g p', label: 'Player Characters' },
  { keys: 'g n', label: 'NPCs' },
  { keys: 'g l', label: 'Locations' },
  { keys: 'g f', label: 'Factions' },
  { keys: 'g c', label: 'Creatures' },
  { keys: 'g i', label: 'Items' },
  { keys: 'g e', label: 'Events' },
  { keys: 'g s', label: 'Sessions' },
  { keys: 'g m', label: 'Misc' },
  { keys: 'g ,', label: 'Settings' },
  { keys: 'c', label: 'New entity (in a notebook)' },
  { keys: '/', label: 'Focus the chat box' },
  { keys: '?', label: 'Show / hide this help' },
  { keys: 'Esc', label: 'Close overlay' },
];
