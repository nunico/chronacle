/** Svelte action for dialog keyboard behavior: autofocus, focus trap,
 * Escape-to-close, and focus restoration to the opener on destroy.
 *
 * Usage: `<div role="dialog" use:modalBehavior={{ onClose }}>`
 */

export interface ModalBehaviorOptions {
  onClose: () => void;
}

const FOCUSABLE = 'a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])';
const activeModalNodes = new WeakSet<HTMLElement>();

function focusables(node: HTMLElement): HTMLElement[] {
  return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => !el.hasAttribute('disabled') && !el.getAttribute('aria-hidden'),
  );
}

export function modalBehavior(node: HTMLElement, options: ModalBehaviorOptions) {
  let opts = options;
  activeModalNodes.add(node);
  const previouslyFocused =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const initial =
    node.querySelector<HTMLElement>('[data-autofocus]:not([disabled])') ?? focusables(node)[0];
  (initial ?? node).focus();

  function handleKeydown(e: KeyboardEvent) {
    if (!(e.target instanceof HTMLElement) || !node.contains(e.target)) return;
    let owner: HTMLElement | null = e.target;
    while (owner && !activeModalNodes.has(owner)) owner = owner.parentElement;
    if (owner !== node) return;

    // A modal owns keyboard interaction while it is open. In particular, do not
    // let application-level window shortcuts run behind the dialog. Listening at
    // document lets Svelte's delegated component handlers run first.
    e.stopPropagation();
    if (e.key === 'Escape') {
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

  document.addEventListener('keydown', handleKeydown);

  return {
    update(newOptions: ModalBehaviorOptions) {
      opts = newOptions;
    },
    destroy() {
      document.removeEventListener('keydown', handleKeydown);
      activeModalNodes.delete(node);
      previouslyFocused?.focus();
    },
  };
}
