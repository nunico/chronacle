import { beforeEach, describe, expect, it, vi } from 'vitest';

const windowApi = vi.hoisted(() => ({
  onCloseRequested: vi.fn(),
}));
const eventApi = vi.hoisted(() => ({ listen: vi.fn() }));
const commandApi = vi.hoisted(() => ({
  requestAppExit: vi.fn(),
  confirmAppExit: vi.fn(),
  cancelAppExit: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => windowApi,
}));
vi.mock('@tauri-apps/api/event', () => eventApi);
vi.mock('../commands', () => commandApi);

import { createTauriWindowClosePort } from './window-close';

describe('Tauri window close port', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    commandApi.requestAppExit.mockResolvedValue(17);
    commandApi.confirmAppExit.mockResolvedValue(undefined);
    commandApi.cancelAppExit.mockResolvedValue(undefined);
  });

  it('atomically registers window and application exit handlers', async () => {
    const unlistenWindow = vi.fn();
    const unlistenApplication = vi.fn();
    let nativeHandler: ((event: { preventDefault(): void }) => void) | undefined;
    let applicationHandler: ((event: { payload: { intent: number } }) => void) | undefined;
    windowApi.onCloseRequested.mockImplementation(
      async (handler: (event: { preventDefault(): void }) => void) => {
        nativeHandler = handler;
        return unlistenWindow;
      },
    );
    eventApi.listen.mockImplementation(
      async (_name: string, handler: (event: { payload: { intent: number } }) => void) => {
        applicationHandler = handler;
        return unlistenApplication;
      },
    );
    const handler = vi.fn();

    const port = createTauriWindowClosePort();
    const stop = await port.registerCloseRequests(handler);
    const event = { preventDefault: vi.fn() };
    nativeHandler?.(event);
    await vi.waitFor(() => expect(commandApi.requestAppExit).toHaveBeenCalledOnce());
    applicationHandler?.({ payload: { intent: 23 } });

    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenNthCalledWith(1, { intent: 17, source: 'window' });
    expect(handler).toHaveBeenNthCalledWith(2, { intent: 23, source: 'application' });
    stop();
    expect(unlistenWindow).toHaveBeenCalledOnce();
    expect(unlistenApplication).toHaveBeenCalledOnce();
  });

  it('tears down a partial registration when the other listener fails', async () => {
    const unlistenWindow = vi.fn();
    windowApi.onCloseRequested.mockResolvedValue(unlistenWindow);
    eventApi.listen.mockRejectedValue(new Error('event unavailable'));

    await expect(createTauriWindowClosePort().registerCloseRequests(vi.fn())).rejects.toThrow(
      'event unavailable',
    );
    expect(unlistenWindow).toHaveBeenCalledOnce();
  });

  it('routes confirmation and cancellation through typed application commands', async () => {
    const port = createTauriWindowClosePort();

    await port.confirmExit(17, 'discard');
    await port.cancelExit(17);

    expect(commandApi.confirmAppExit).toHaveBeenCalledWith(17, 'discard');
    expect(commandApi.cancelAppExit).toHaveBeenCalledWith(17);
  });
});
