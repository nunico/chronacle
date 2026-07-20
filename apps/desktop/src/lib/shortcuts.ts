// Keyboard-shortcut model — Vim-style g-chords for GM-at-the-table navigation.
//
// Pure, framework-free so it can be unit-tested without a DOM. The Shell owns
// the leader-key state machine and DOM wiring; this module owns the *mapping*
// (which key goes where) and the typing-suppression rule.

import type { NoteCategoryId } from '../shell/note-categories';
import type { MessageKey } from './i18n/messages';

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
export const SHORTCUT_HELP: ReadonlyArray<{ keys: string; labelKey: MessageKey }> = [
  { keys: 'g o', labelKey: 'shortcuts.oracle' },
  { keys: 'g t', labelKey: 'shortcuts.timeline' },
  { keys: 'g p', labelKey: 'shortcuts.playerCharacters' },
  { keys: 'g n', labelKey: 'shortcuts.npcs' },
  { keys: 'g l', labelKey: 'shortcuts.locations' },
  { keys: 'g f', labelKey: 'shortcuts.factions' },
  { keys: 'g c', labelKey: 'shortcuts.creatures' },
  { keys: 'g i', labelKey: 'shortcuts.items' },
  { keys: 'g e', labelKey: 'shortcuts.events' },
  { keys: 'g s', labelKey: 'shortcuts.sessions' },
  { keys: 'g m', labelKey: 'shortcuts.misc' },
  { keys: 'g ,', labelKey: 'shortcuts.settings' },
  { keys: 'c', labelKey: 'shortcuts.newEntity' },
  { keys: '/', labelKey: 'shortcuts.focusChat' },
  { keys: '?', labelKey: 'shortcuts.toggleHelp' },
  { keys: 'Esc', labelKey: 'shortcuts.closeOverlay' },
];
