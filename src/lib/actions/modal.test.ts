import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { modalBehavior } from './modal';

function buildDialog(): HTMLDivElement {
  const dialog = document.createElement('div');
  dialog.innerHTML = `
    <select id="first"><option>a</option></select>
    <input id="middle" />
    <button id="last">Confirm</button>
  `;
  document.body.appendChild(dialog);
  return dialog;
}

describe('modalBehavior', () => {
  let dialog: HTMLDivElement;
  let opener: HTMLButtonElement;

  beforeEach(() => {
    opener = document.createElement('button');
    opener.id = 'opener';
    document.body.appendChild(opener);
    opener.focus();
    dialog = buildDialog();
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('focuses the first focusable element on mount', () => {
    const action = modalBehavior(dialog, { onClose: vi.fn() });
    expect(document.activeElement?.id).toBe('first');
    action.destroy();
  });

  it('prefers an element marked data-autofocus', () => {
    dialog.querySelector('#middle')?.setAttribute('data-autofocus', '');
    const action = modalBehavior(dialog, { onClose: vi.fn() });
    expect(document.activeElement?.id).toBe('middle');
    action.destroy();
  });

  it('calls onClose when Escape is pressed inside the dialog', () => {
    const onClose = vi.fn();
    const action = modalBehavior(dialog, { onClose });
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(onClose).toHaveBeenCalledOnce();
    action.destroy();
  });

  it('wraps Tab from the last focusable to the first', () => {
    const action = modalBehavior(dialog, { onClose: vi.fn() });
    (dialog.querySelector('#last') as HTMLElement).focus();
    dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    expect(document.activeElement?.id).toBe('first');
    action.destroy();
  });

  it('wraps Shift+Tab from the first focusable to the last', () => {
    const action = modalBehavior(dialog, { onClose: vi.fn() });
    (dialog.querySelector('#first') as HTMLElement).focus();
    dialog.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Tab', shiftKey: true, bubbles: true }),
    );
    expect(document.activeElement?.id).toBe('last');
    action.destroy();
  });

  it('restores focus to the previously focused element on destroy', () => {
    const action = modalBehavior(dialog, { onClose: vi.fn() });
    expect(document.activeElement?.id).toBe('first');
    action.destroy();
    expect(document.activeElement?.id).toBe('opener');
  });

  it('skips disabled elements when choosing the initial focus target', () => {
    (dialog.querySelector('#first') as HTMLSelectElement).disabled = true;
    const action = modalBehavior(dialog, { onClose: vi.fn() });
    expect(document.activeElement?.id).toBe('middle');
    action.destroy();
  });
});
