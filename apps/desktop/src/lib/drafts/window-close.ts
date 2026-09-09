import { getCurrentWindow } from '@tauri-apps/api/window';

export interface WindowCloseEvent {
  preventDefault(): void;
}

export interface WindowClosePort {
  onCloseRequested(handler: (event: WindowCloseEvent) => void): Promise<() => void>;
  destroy(): Promise<void>;
}

export function createTauriWindowClosePort(): WindowClosePort {
  return {
    async onCloseRequested(handler) {
      return getCurrentWindow().onCloseRequested(handler);
    },
    async destroy() {
      await getCurrentWindow().destroy();
    },
  };
}
