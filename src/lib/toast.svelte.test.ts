import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { toasts, showToast, dismissToast, clearToasts } from './toast.svelte';

describe('toast store', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    clearToasts();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('adds a toast with the given message and variant', () => {
    showToast('Saved', 'success');
    expect(toasts.length).toBe(1);
    expect(toasts[0].message).toBe('Saved');
    expect(toasts[0].variant).toBe('success');
  });

  it('auto-dismisses success toasts after the timeout', () => {
    showToast('Saved', 'success');
    expect(toasts.length).toBe(1);
    vi.advanceTimersByTime(4000);
    expect(toasts.length).toBe(0);
  });

  it('keeps error toasts until manually dismissed', () => {
    const id = showToast('Upload failed', 'error');
    vi.advanceTimersByTime(60_000);
    expect(toasts.length).toBe(1);
    dismissToast(id);
    expect(toasts.length).toBe(0);
  });

  it('dismissing one toast leaves the others intact', () => {
    const a = showToast('one', 'error');
    showToast('two', 'error');
    dismissToast(a);
    expect(toasts.length).toBe(1);
    expect(toasts[0].message).toBe('two');
  });
});
