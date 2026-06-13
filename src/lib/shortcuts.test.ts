import { describe, it, expect } from 'vitest';
import { resolveNavChord, isEditableTarget, SHORTCUT_HELP } from './shortcuts';

describe('resolveNavChord', () => {
  it('maps g-chord keys to destinations', () => {
    expect(resolveNavChord('o')).toBe('oracle');
    expect(resolveNavChord(',')).toBe('settings');
    expect(resolveNavChord('n')).toEqual({ category: 'npcs' });
    expect(resolveNavChord('s')).toEqual({ category: 'sessions' });
  });

  it('is case-insensitive', () => {
    expect(resolveNavChord('N')).toEqual({ category: 'npcs' });
    expect(resolveNavChord('O')).toBe('oracle');
  });

  it('returns null for unmapped keys', () => {
    expect(resolveNavChord('z')).toBeNull();
    expect(resolveNavChord('1')).toBeNull();
  });
});

describe('isEditableTarget', () => {
  it('flags inputs, textareas, and selects', () => {
    expect(isEditableTarget(document.createElement('input'))).toBe(true);
    expect(isEditableTarget(document.createElement('textarea'))).toBe(true);
    expect(isEditableTarget(document.createElement('select'))).toBe(true);
  });

  it('flags contenteditable hosts', () => {
    const div = document.createElement('div');
    div.setAttribute('contenteditable', 'true');
    expect(isEditableTarget(div)).toBe(true);
  });

  it('does not flag plain elements or null', () => {
    expect(isEditableTarget(document.createElement('div'))).toBe(false);
    expect(isEditableTarget(document.createElement('button'))).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
  });
});

describe('SHORTCUT_HELP', () => {
  it('documents every nav chord plus the action keys', () => {
    const keys = SHORTCUT_HELP.map((r) => r.keys);
    for (const k of ['g o', 'g n', 'g s', 'g ,', 'c', '/', '?', 'Esc']) {
      expect(keys).toContain(k);
    }
  });
});
