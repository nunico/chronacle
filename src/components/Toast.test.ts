import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import Toast from './Toast.svelte';
import { showToast, clearToasts } from '../lib/toast.svelte';

describe('Toast', () => {
  beforeEach(() => {
    clearToasts();
  });

  it('renders nothing when there are no toasts', () => {
    const { container } = render(Toast);
    expect(container.querySelector('.toast')).toBeNull();
  });

  it('renders an error toast with role="alert" and a dismiss button', async () => {
    render(Toast);
    showToast('Upload failed', 'error');
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Upload failed');
    const dismiss = screen.getByRole('button', { name: /dismiss/i });
    await fireEvent.click(dismiss);
    await waitFor(() => {
      expect(screen.queryByRole('alert')).toBeNull();
    });
  });

  it('renders a success toast with role="status"', async () => {
    render(Toast);
    showToast('Saved', 'success');
    const status = await screen.findByRole('status');
    expect(status.textContent).toContain('Saved');
  });
});
