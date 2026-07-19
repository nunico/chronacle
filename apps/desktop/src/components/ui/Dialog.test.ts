import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import Dialog from './Dialog.svelte';

describe('Dialog', () => {
  it('names the modal dialog from its title and closes on Escape', async () => {
    const onclose = vi.fn();
    render(Dialog, { props: { title: 'Delete campaign', onclose } });

    const dialog = screen.getByRole('dialog', { name: 'Delete campaign' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');

    await fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(onclose).toHaveBeenCalledOnce();
  });

  it('focuses a body-only dialog so Escape can close it', async () => {
    const onclose = vi.fn();
    render(Dialog, { props: { title: 'About Chronacle', onclose } });

    const dialog = screen.getByRole('dialog', { name: 'About Chronacle' });
    expect(document.activeElement).toBe(dialog);

    await fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'Escape' });
    expect(onclose).toHaveBeenCalledOnce();
  });

  it('uses unique heading IDs for duplicate non-Latin titles', () => {
    render(Dialog, { props: { title: '同じ題名' } });
    render(Dialog, { props: { title: '同じ題名' } });

    const dialogs = screen.getAllByRole('dialog', { name: '同じ題名' });
    const titleIds = dialogs.map((dialog) => dialog.getAttribute('aria-labelledby'));

    expect(titleIds[0]).toBeTruthy();
    expect(titleIds[1]).toBeTruthy();
    expect(titleIds[0]).not.toBe(titleIds[1]);
    for (const titleId of titleIds) {
      if (titleId) {
        expect(document.getElementById(titleId)).toHaveTextContent('同じ題名');
      }
    }
  });
});
