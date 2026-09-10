import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import {
  cancelAppExit,
  confirmAppExit,
  pendingAppExit,
  requestAppExit,
  type AppExitDecision,
} from '../commands';

export interface CloseRequest {
  intent: number | null;
  source: 'window' | 'application';
}

export interface WindowClosePort {
  registerCloseRequests(handler: (request: CloseRequest) => void): Promise<() => void>;
  requestExitIntent(): Promise<number>;
  confirmExit(intent: number, decision: AppExitDecision): Promise<void>;
  cancelExit(intent: number): Promise<boolean>;
}

export function createTauriWindowClosePort(): WindowClosePort {
  return {
    async registerCloseRequests(handler) {
      let active = true;
      const deliveredIntents = new Set<number>();
      let stopWindow: (() => void) | undefined;
      let stopApplication: (() => void) | undefined;
      const deliver = (request: CloseRequest): void => {
        if (!active) return;
        if (request.intent !== null) {
          if (deliveredIntents.has(request.intent)) return;
          deliveredIntents.add(request.intent);
        }
        handler(request);
      };
      try {
        stopWindow = await getCurrentWindow().onCloseRequested((event) => {
          event.preventDefault();
          void requestAppExit()
            .then((intent) => deliver({ intent, source: 'window' }))
            .catch(() => deliver({ intent: null, source: 'window' }));
        });
        stopApplication = await listen<{ intent: number }>('app-exit-requested', (event) => {
          deliver({ intent: event.payload.intent, source: 'application' });
        });
        const pendingIntent = await pendingAppExit();
        if (pendingIntent !== null) deliver({ intent: pendingIntent, source: 'application' });
      } catch (error) {
        active = false;
        stopApplication?.();
        stopWindow?.();
        throw error;
      }
      return () => {
        active = false;
        stopApplication?.();
        stopWindow?.();
      };
    },
    async requestExitIntent() {
      return requestAppExit();
    },
    async confirmExit(intent, decision) {
      await confirmAppExit(intent, decision);
    },
    async cancelExit(intent) {
      return cancelAppExit(intent);
    },
  };
}
