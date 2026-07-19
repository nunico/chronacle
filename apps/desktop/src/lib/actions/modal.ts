/** Svelte action for dialog keyboard behavior: autofocus, focus trap,
 * Escape-to-close, and focus restoration to the opener on destroy.
 *
 * Usage: `<div role="dialog" use:modalBehavior={{ onClose }}>`
 */

export interface ModalBehaviorOptions {
  onClose: () => void;
}

const FOCUSABLE =
  'a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])';

function focusables(node: HTMLElement): HTMLElement[] {
  return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => !el.hasAttribute('disabled') && !el.getAttribute('aria-hidden'),
  );
}

export function modalBehavior(node: HTMLElement, options: ModalBehaviorOptions) {
  let opts = options;
  const previouslyFocused =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const initial =
    node.querySelector<HTMLElement>('[data-autofocus]:not([disabled])') ?? focusables(node)[0];
  (initial ?? node).focus();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      opts.onClose();
      return;
    }
    if (e.key !== 'Tab') return;
    const els = focusables(node);
    if (els.length === 0) return;
    const first = els[0];
    const last = els[els.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  }

  node.addEventListener('keydown', handleKeydown);

  return {
    update(newOptions: ModalBehaviorOptions) {
      opts = newOptions;
    },
    destroy() {
      node.removeEventListener('keydown', handleKeydown);
      previouslyFocused?.focus();
    },
  };
}
