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
});
