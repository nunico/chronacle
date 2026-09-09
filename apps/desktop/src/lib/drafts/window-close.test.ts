import { beforeEach, describe, expect, it, vi } from 'vitest';

const windowApi = vi.hoisted(() => ({
  onCloseRequested: vi.fn(),
  destroy: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => windowApi,
}));

import { createTauriWindowClosePort } from './window-close';

describe('Tauri window close port', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    windowApi.destroy.mockResolvedValue(undefined);
  });

  it('registers the close handler and exposes its unlisten function', async () => {
    const unlisten = vi.fn();
    let nativeHandler: ((event: { preventDefault(): void }) => void) | undefined;
    windowApi.onCloseRequested.mockImplementation(
      async (handler: (event: { preventDefault(): void }) => void) => {
        nativeHandler = handler;
        return unlisten;
      },
    );
    const handler = vi.fn();

    const port = createTauriWindowClosePort();
    const stop = await port.onCloseRequested(handler);
    const event = { preventDefault: vi.fn() };
    nativeHandler?.(event);

    expect(handler).toHaveBeenCalledWith(event);
    stop();
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it('uses forced native destruction after an explicit close decision', async () => {
    const port = createTauriWindowClosePort();

    await port.destroy();

    expect(windowApi.destroy).toHaveBeenCalledOnce();
  });
});
