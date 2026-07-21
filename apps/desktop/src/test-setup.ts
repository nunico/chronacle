import '@testing-library/jest-dom/vitest';

interface TauriInternals {
  invoke: (command: string, payload?: Record<string, unknown>) => Promise<unknown>;
  transformCallback: (_callback: unknown, _once?: boolean) => number;
}

interface EventPluginInternals {
  unregisterListener(eventId: number): void;
}

const tauriWindow = window as typeof window & {
  __TAURI_INTERNALS__?: TauriInternals;
  __TAURI_EVENT_PLUGIN_INTERNALS__?: EventPluginInternals;
};

let callbackId = 0;

tauriWindow.__TAURI_INTERNALS__ ??= {
  async invoke(command) {
    switch (command) {
      case 'plugin:os|locale':
        return navigator.language;
      case 'plugin:event|listen':
        return ++callbackId;
      case 'plugin:event|unlisten':
        return undefined;
      default:
        return undefined;
    }
  },
  transformCallback() {
    return ++callbackId;
  },
};

tauriWindow.__TAURI_EVENT_PLUGIN_INTERNALS__ ??= {
  unregisterListener(_eventId) {
    return undefined;
  },
};
