import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { cancelAppExit, confirmAppExit, requestAppExit, type AppExitDecision } from '../commands';

export interface WindowCloseEvent {
  preventDefault(): void;
}

export interface CloseRequest {
  intent: number;
  source: 'window' | 'application';
}

export interface WindowClosePort {
  registerCloseRequests(handler: (request: CloseRequest) => void): Promise<() => void>;
  confirmExit(intent: number, decision: AppExitDecision): Promise<void>;
  cancelExit(intent: number): Promise<void>;
}

export function createTauriWindowClosePort(): WindowClosePort {
  return {
    async registerCloseRequests(handler) {
      let stopWindow: (() => void) | undefined;
      let stopApplication: (() => void) | undefined;
      try {
        stopWindow = await getCurrentWindow().onCloseRequested((event) => {
          event.preventDefault();
          void requestAppExit().then((intent) => handler({ intent, source: 'window' }));
        });
        stopApplication = await listen<{ intent: number }>('app-exit-requested', (event) => {
          handler({ intent: event.payload.intent, source: 'application' });
        });
      } catch (error) {
        stopApplication?.();
        stopWindow?.();
        throw error;
      }
      return () => {
        stopApplication?.();
        stopWindow?.();
      };
    },
    async confirmExit(intent, decision) {
      await confirmAppExit(intent, decision);
    },
    async cancelExit(intent) {
      await cancelAppExit(intent);
    },
  };
}
