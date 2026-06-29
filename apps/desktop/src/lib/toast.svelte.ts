/** App-wide toast store. Success/info toasts auto-dismiss; error toasts
 * persist until the user dismisses them. Rendered once by `Toast.svelte`
 * mounted in the Shell.
 */

export type ToastVariant = 'info' | 'success' | 'error';

export interface ToastItem {
  id: number;
  message: string;
  variant: ToastVariant;
}

const AUTO_DISMISS_MS = 4000;

let nextId = 1;

export const toasts = $state<ToastItem[]>([]);

export function showToast(message: string, variant: ToastVariant = 'info'): number {
  const id = nextId++;
  toasts.push({ id, message, variant });
  if (variant !== 'error') {
    setTimeout(() => dismissToast(id), AUTO_DISMISS_MS);
  }
  return id;
}

export function dismissToast(id: number): void {
  const idx = toasts.findIndex((t) => t.id === id);
  if (idx !== -1) toasts.splice(idx, 1);
}

export function clearToasts(): void {
  toasts.length = 0;
}
