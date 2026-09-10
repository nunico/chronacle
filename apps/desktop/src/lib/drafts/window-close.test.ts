import { beforeEach, describe, expect, it, vi } from 'vitest';

const windowApi = vi.hoisted(() => ({
  onCloseRequested: vi.fn(),
}));
const eventApi = vi.hoisted(() => ({ listen: vi.fn() }));
const commandApi = vi.hoisted(() => ({
  requestAppExit: vi.fn(),
  pendingAppExit: vi.fn(),
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
    commandApi.pendingAppExit.mockResolvedValue(null);
    commandApi.confirmAppExit.mockResolvedValue(undefined);
    commandApi.cancelAppExit.mockResolvedValue(true);
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
    expect(handler).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ intent: 17, source: 'window' }),
    );
    expect(handler).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ intent: 23, source: 'application' }),
    );
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
    await expect(port.cancelExit(17)).resolves.toBe(true);

    expect(commandApi.confirmAppExit).toHaveBeenCalledWith(17, 'discard');
    expect(commandApi.cancelAppExit).toHaveBeenCalledWith(17);
  });

  it('replays an application exit emitted before registration completes exactly once', async () => {
    const pending = deferred<number | null>();
    commandApi.pendingAppExit.mockReturnValue(pending.promise);
    windowApi.onCloseRequested.mockResolvedValue(vi.fn());
    let applicationHandler: ((event: { payload: { intent: number } }) => void) | undefined;
    eventApi.listen.mockImplementation(
      async (_name: string, handler: (event: { payload: { intent: number } }) => void) => {
        applicationHandler = handler;
        return vi.fn();
      },
    );
    const handler = vi.fn();
    const registration = createTauriWindowClosePort().registerCloseRequests(handler);
    await vi.waitFor(() => expect(commandApi.pendingAppExit).toHaveBeenCalledOnce());

    applicationHandler?.({ payload: { intent: 31 } });
    pending.resolve(31);
    await registration;

    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith(
      expect.objectContaining({ intent: 31, source: 'application' }),
    );
  });

  it('reports request failure and ignores late request completion after teardown', async () => {
    let nativeHandler: ((event: { preventDefault(): void }) => void) | undefined;
    windowApi.onCloseRequested.mockImplementation(
      async (handler: (event: { preventDefault(): void }) => void) => {
        nativeHandler = handler;
        return vi.fn();
      },
    );
    eventApi.listen.mockResolvedValue(vi.fn());
    const handler = vi.fn();
    const port = createTauriWindowClosePort();
    const stop = await port.registerCloseRequests(handler);
    commandApi.requestAppExit.mockRejectedValueOnce(new Error('IPC unavailable'));

    nativeHandler?.({ preventDefault: vi.fn() });
    await vi.waitFor(() =>
      expect(handler).toHaveBeenCalledWith(
        expect.objectContaining({ source: 'window', intent: null }),
      ),
    );

    const late = deferred<number>();
    commandApi.requestAppExit.mockReturnValueOnce(late.promise);
    nativeHandler?.({ preventDefault: vi.fn() });
    stop();
    late.resolve(99);
    await Promise.resolve();

    expect(handler.mock.calls.some(([request]) => request.intent === 99)).toBe(false);
  });

  it('tears down immediately when aborted while pending-intent replay is unresolved', async () => {
    const replay = deferred<number | null>();
    const unlistenWindow = vi.fn();
    const unlistenApplication = vi.fn();
    commandApi.pendingAppExit.mockReturnValue(replay.promise);
    windowApi.onCloseRequested.mockResolvedValue(unlistenWindow);
    eventApi.listen.mockResolvedValue(unlistenApplication);
    const handler = vi.fn();
    const controller = new AbortController();

    const registration = createTauriWindowClosePort().registerCloseRequests(
      handler,
      controller.signal,
    );
    await vi.waitFor(() => expect(commandApi.pendingAppExit).toHaveBeenCalledOnce());
    controller.abort();

    expect(unlistenWindow).toHaveBeenCalledOnce();
    expect(unlistenApplication).toHaveBeenCalledOnce();
    replay.resolve(47);
    await expect(registration).rejects.toThrow('Close registration was cancelled.');
    expect(handler).not.toHaveBeenCalled();
  });

  it('captures the exact opener before waiting for the window intent', async () => {
    const intent = deferred<number>();
    commandApi.requestAppExit.mockReturnValue(intent.promise);
    let nativeHandler: ((event: { preventDefault(): void }) => void) | undefined;
    windowApi.onCloseRequested.mockImplementation(
      async (handler: (event: { preventDefault(): void }) => void) => {
        nativeHandler = handler;
        return vi.fn();
      },
    );
    eventApi.listen.mockResolvedValue(vi.fn());
    const handler = vi.fn();
    await createTauriWindowClosePort().registerCloseRequests(handler);
    const opener = document.createElement('textarea');
    const laterFocus = document.createElement('button');
    document.body.append(opener, laterFocus);
    opener.focus();

    nativeHandler?.({ preventDefault: vi.fn() });
    laterFocus.focus();
    intent.resolve(59);
    await vi.waitFor(() => expect(handler).toHaveBeenCalledOnce());

    expect(handler).toHaveBeenCalledWith({ intent: 59, source: 'window', opener });
    opener.remove();
    laterFocus.remove();
  });
});

interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
