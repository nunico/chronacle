import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { i18n } from '../lib/locale.svelte';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import CloseDraftsDialog from './CloseDraftsDialog.svelte';

interface Deferred<T> {
  readonly promise: Promise<T>;
  resolve(value: T): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function dirtyCoordinator(): DraftCoordinator {
  const coordinator = new DraftCoordinator();
  coordinator.open('oracle:camp-a', null, '');
  coordinator.revise('oracle:camp-a', 'Where is the silver key?');
  return coordinator;
}

function renderDialog(
  coordinator: DraftCoordinator,
  overrides: { oncancel?: () => void; ondestroy?: () => Promise<void> | void } = {},
) {
  const oncancel = overrides.oncancel ?? vi.fn();
  const ondestroy = overrides.ondestroy ?? vi.fn().mockResolvedValue(undefined);
  const rendered = render(CloseDraftsDialog, {
    props: { draftCoordinator: coordinator, oncancel, ondestroy },
  });
  return { ...rendered, oncancel, ondestroy };
}

describe('CloseDraftsDialog', () => {
  beforeEach(() => i18n.setLocale('en'));
  afterEach(() => i18n.setLocale('en'));

  it('is accessible, initially focuses Cancel, traps focus, and restores the opener on Escape', async () => {
    const opener = document.createElement('textarea');
    document.body.append(opener);
    opener.focus();
    const { oncancel, unmount } = renderDialog(dirtyCoordinator());

    const dialog = screen.getByRole('dialog', { name: 'Unsaved changes' });
    const cancel = screen.getByRole('button', { name: 'Cancel' });
    const discard = screen.getByRole('button', { name: 'Discard and close' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(document.activeElement).toBe(cancel);

    await fireEvent.keyDown(cancel, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(discard);
    await fireEvent.keyDown(discard, { key: 'Tab' });
    expect(document.activeElement).toBe(cancel);

    await fireEvent.keyDown(cancel, { key: 'Escape' });
    expect(oncancel).toHaveBeenCalledOnce();
    unmount();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it('discards every retained draft through coordinator policy before destroying exactly once', async () => {
    const coordinator = dirtyCoordinator();
    coordinator.open('entity:camp-a:npc:mira', 'entity:npc:mira', { name: 'Mira' });
    coordinator.revise('entity:camp-a:npc:mira', { name: 'Mira the Bold' });
    const discardAll = vi.spyOn(coordinator, 'discardAll');
    const ondestroy = vi.fn().mockResolvedValue(undefined);
    renderDialog(coordinator, { ondestroy });

    const close = screen.getByRole('button', { name: 'Discard and close' });
    await fireEvent.click(close);
    await fireEvent.click(close);

    await waitFor(() => expect(ondestroy).toHaveBeenCalledOnce());
    expect(discardAll).toHaveBeenCalledOnce();
    expect(coordinator.atRiskCount()).toBe(0);
  });

  it('blocks destructive close while a write is active and never starts another writer', async () => {
    const coordinator = new DraftCoordinator();
    const save = deferred<string>();
    const writer = vi.fn(() => save.promise);
    const ondestroy = vi.fn();
    coordinator.open('session:camp-a:s1', 'session:s1', 'Saved title');
    coordinator.revise('session:camp-a:s1', 'Earlier revision');
    const activeSave = coordinator.requestSave('session:camp-a:s1', writer);
    coordinator.revise('session:camp-a:s1', 'Newer revision');
    renderDialog(coordinator, { ondestroy });

    expect(screen.getByRole('button', { name: 'Discard and close' })).toBeDisabled();
    expect(screen.getByRole('status')).toHaveTextContent(
      'Wait for saving to finish before discarding and closing.',
    );
    expect(writer).toHaveBeenCalledOnce();
    expect(ondestroy).not.toHaveBeenCalled();

    save.resolve('Earlier revision');
    await activeSave;
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Discard and close' })).toBeEnabled(),
    );
    expect(coordinator.get<string>('session:camp-a:s1')?.value).toBe('Newer revision');
    expect(writer).toHaveBeenCalledOnce();
  });

  it('keeps the prevented request explicit and offers Close when an active save makes all work clean', async () => {
    const coordinator = new DraftCoordinator();
    const save = deferred<string>();
    coordinator.open('session:camp-a:s1', 'session:s1', 'Saved title');
    coordinator.revise('session:camp-a:s1', 'Canonical title');
    const activeSave = coordinator.requestSave('session:camp-a:s1', () => save.promise);
    const ondestroy = vi.fn().mockResolvedValue(undefined);
    renderDialog(coordinator, { ondestroy });
    const cancel = screen.getByRole('button', { name: 'Cancel' });
    expect(document.activeElement).toBe(cancel);

    save.resolve('Canonical title');
    await activeSave;

    expect(await screen.findByText('Saving finished. It is safe to close.')).toBeInTheDocument();
    const close = screen.getByRole('button', { name: /^Close$/ });
    expect(close).toBeEnabled();
    expect(document.activeElement).toBe(cancel);
    expect(ondestroy).not.toHaveBeenCalled();

    await fireEvent.click(close);
    await waitFor(() => expect(ondestroy).toHaveBeenCalledOnce());
  });

  it.each([
    [
      'en',
      'Discard and close',
      'Wait for saving to finish before discarding and closing.',
      'Saving finished. It is safe to close.',
      'Close',
    ],
    [
      'de',
      'Verwerfen und schließen',
      'Warte, bis das Speichern abgeschlossen ist, bevor du Änderungen verwirfst und schließt.',
      'Speichern abgeschlossen. Du kannst jetzt sicher schließen.',
      'Schließen',
    ],
    [
      'fr',
      'Abandonner et fermer',
      'Attendez la fin de l’enregistrement avant d’abandonner et de fermer.',
      'L’enregistrement est terminé. Vous pouvez fermer en toute sécurité.',
      'Fermer',
    ],
    [
      'es',
      'Descartar y cerrar',
      'Espera a que termine el guardado antes de descartar y cerrar.',
      'El guardado ha finalizado. Ya puedes cerrar de forma segura.',
      'Cerrar',
    ],
  ] as const)(
    'renders the active-save and safe-close contract in %s',
    async (locale, discardAndClose, waitForSaving, safeToClose, close) => {
      i18n.setLocale(locale);
      const coordinator = new DraftCoordinator();
      const save = deferred<string>();
      coordinator.open('session:camp-a:s1', 'session:s1', 'Saved title');
      coordinator.revise('session:camp-a:s1', 'Canonical title');
      const activeSave = coordinator.requestSave('session:camp-a:s1', () => save.promise);
      renderDialog(coordinator);

      expect(screen.getByRole('button', { name: discardAndClose })).toBeDisabled();
      expect(screen.getByRole('status')).toHaveTextContent(waitForSaving);

      save.resolve('Canonical title');
      await activeSave;

      expect(await screen.findByText(safeToClose)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: new RegExp(`^${close}$`) })).toBeEnabled();
    },
  );
});
