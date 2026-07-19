import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import UploadProgress from './UploadProgress.svelte';
import { setUiLocalePreference } from './lib/locale.svelte';

describe('UploadProgress', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    setUiLocalePreference('en');
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders nothing when idle', () => {
    const { container } = render(UploadProgress, {
      props: { phase: 'idle', filename: '', status: '', progress: 0, onDismiss: vi.fn() },
    });
    expect(container.querySelector('.upload-status')).toBeNull();
  });

  it('shows filename, status, and progress bar while active', () => {
    render(UploadProgress, {
      props: {
        phase: 'active',
        filename: 'rules.pdf',
        status: 'Indexing PDF…',
        progress: 42,
        onDismiss: vi.fn(),
      },
    });
    expect(screen.getByText('rules.pdf')).toBeTruthy();
    expect(screen.getByText('Indexing PDF…')).toBeTruthy();
    expect(screen.getByText('42%')).toBeTruthy();
    expect(screen.getByRole('progressbar', { name: 'Upload progress' })).toHaveAttribute(
      'aria-valuenow',
      '42',
    );
  });

  it('auto-dismisses a few seconds after completion', async () => {
    const onDismiss = vi.fn();
    render(UploadProgress, {
      props: { phase: 'done', filename: 'rules.pdf', status: 'Ready!', progress: 100, onDismiss },
    });
    expect(screen.getByText('Ready!')).toBeTruthy();
    expect(onDismiss).not.toHaveBeenCalled();
    vi.advanceTimersByTime(4000);
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('keeps an error visible until dismissed via the close button', async () => {
    const onDismiss = vi.fn();
    render(UploadProgress, {
      props: {
        phase: 'error',
        filename: 'rules.pdf',
        status: 'Error: corrupt PDF',
        progress: 10,
        onDismiss,
      },
    });
    vi.advanceTimersByTime(60_000);
    expect(onDismiss).not.toHaveBeenCalled();
    expect(screen.getByText(/corrupt PDF/)).toBeTruthy();
    const dismiss = screen.getByRole('button', { name: /dismiss/i });
    await fireEvent.click(dismiss);
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
