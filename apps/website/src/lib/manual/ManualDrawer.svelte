<script lang="ts">
  import type { Locale } from '$lib/i18n/types';
  import type { Attachment } from 'svelte/attachments';
  import Menu from 'lucide-svelte/icons/menu';
  import X from 'lucide-svelte/icons/x';
  import Icon from '$lib/components/Icon.svelte';
  import ManualSidebar from './ManualSidebar.svelte';

  let { locale, currentSlug }: { locale: Locale; currentSlug: string } = $props();

  let dialog: HTMLDialogElement | undefined;
  let closeButton: HTMLButtonElement | undefined;
  let opener: HTMLButtonElement | undefined;
  const label = $derived(locale === 'de' ? 'Handbuchnavigation' : 'Manual navigation');
  const openLabel = $derived(
    locale === 'de' ? 'Handbuchnavigation öffnen' : 'Open manual navigation',
  );
  const closeLabel = $derived(
    locale === 'de' ? 'Handbuchnavigation schließen' : 'Close manual navigation',
  );

  const attachDialog: Attachment<HTMLDialogElement> = (node) => {
    dialog = node;
    return () => {
      dialog = undefined;
    };
  };

  const attachCloseButton: Attachment<HTMLButtonElement> = (node) => {
    closeButton = node;
    return () => {
      closeButton = undefined;
    };
  };

  function focusableElements(): HTMLElement[] {
    return dialog
      ? Array.from(
          dialog.querySelectorAll<HTMLElement>(
            'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
          ),
        )
      : [];
  }

  function openDrawer(event: MouseEvent): void {
    opener = event.currentTarget as HTMLButtonElement;
    if (!dialog) {
      return;
    }
    if (typeof dialog.showModal === 'function') {
      dialog.showModal();
    } else {
      dialog.setAttribute('open', '');
    }
    queueMicrotask(() => closeButton?.focus());
  }

  function closeDrawer(): void {
    if (!dialog) {
      return;
    }
    if (typeof dialog.close === 'function') {
      dialog.close();
    } else {
      dialog.removeAttribute('open');
    }
    queueMicrotask(() => opener?.focus());
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeDrawer();
      return;
    }
    if (event.key !== 'Tab') {
      return;
    }

    const focusable = focusableElements();
    const first = focusable[0];
    const last = focusable.at(-1);
    if (!first || !last) {
      event.preventDefault();
      return;
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function handleCancel(event: Event): void {
    event.preventDefault();
    closeDrawer();
  }
</script>

<button class="drawer-trigger" type="button" aria-label={openLabel} onclick={openDrawer}>
  <Icon icon={Menu} size={16} />
  <span>{locale === 'de' ? 'Inhalt' : 'Contents'}</span>
</button>

<dialog
  {@attach attachDialog}
  class="manual-drawer"
  aria-label={label}
  oncancel={handleCancel}
  onkeydown={handleKeydown}
>
  <div class="manual-drawer__header">
    <strong>{label}</strong>
    <button {@attach attachCloseButton} type="button" aria-label={closeLabel} onclick={closeDrawer}>
      <Icon icon={X} size={18} />
    </button>
  </div>
  <div class="manual-drawer__body">
    <ManualSidebar
      {locale}
      {currentSlug}
      navigationLabel={locale === 'de' ? 'Navigation im Menü' : 'Navigation in menu'}
    />
  </div>
</dialog>

<style>
  .drawer-trigger {
    display: none;
    align-items: center;
    gap: var(--s-2);
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg-panel);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 0.8125rem;
    cursor: pointer;
  }

  .manual-drawer {
    width: min(22rem, calc(100vw - 2rem));
    max-width: none;
    height: calc(100dvh - 2rem);
    max-height: none;
    margin: 1rem 1rem 1rem auto;
    padding: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-xl);
    background: rgb(10 12 26 / 96%);
    color: var(--fg-1);
    box-shadow: var(--shadow-3), var(--glow-violet);
  }

  .manual-drawer::backdrop {
    background: var(--bg-scrim);
    backdrop-filter: blur(5px);
  }

  .manual-drawer__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--s-4) var(--s-5);
    border-bottom: 1px solid var(--line);
    font-family: var(--font-sans);
  }

  .manual-drawer__header button {
    display: grid;
    width: 2.25rem;
    height: 2.25rem;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--fg-2);
    font-size: 1.5rem;
    cursor: pointer;
  }

  .manual-drawer__body {
    height: calc(100% - 4.5rem);
    padding: var(--s-5);
    overflow-y: auto;
  }

  @media (max-width: 62rem) {
    .drawer-trigger {
      display: inline-flex;
    }
  }
</style>
