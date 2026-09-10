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
  opener: HTMLElement | null;
}

export interface WindowClosePort {
  registerCloseRequests(
    handler: (request: CloseRequest) => void,
    signal?: AbortSignal,
  ): Promise<() => void>;
  requestExitIntent(): Promise<number>;
  confirmExit(intent: number, decision: AppExitDecision): Promise<void>;
  cancelExit(intent: number): Promise<boolean>;
}

export function createTauriWindowClosePort(): WindowClosePort {
  return {
    async registerCloseRequests(handler, signal) {
      let active = !signal?.aborted;
      const deliveredIntents = new Set<number>();
      let stopWindow: (() => void) | undefined;
      let stopApplication: (() => void) | undefined;
      const teardown = (): void => {
        active = false;
        stopApplication?.();
        stopApplication = undefined;
        stopWindow?.();
        stopWindow = undefined;
      };
      const ensureActive = (): void => {
        if (!active) throw new Error('Close registration was cancelled.');
      };
      signal?.addEventListener('abort', teardown, { once: true });
      const deliver = (request: CloseRequest): void => {
        if (!active) return;
        if (request.intent !== null) {
          if (deliveredIntents.has(request.intent)) return;
          deliveredIntents.add(request.intent);
        }
        handler(request);
      };
      try {
        ensureActive();
        const registeredWindowStop = await getCurrentWindow().onCloseRequested((event) => {
          event.preventDefault();
          const opener =
            document.activeElement instanceof HTMLElement ? document.activeElement : null;
          void requestAppExit()
            .then((intent) => deliver({ intent, source: 'window', opener }))
            .catch(() => deliver({ intent: null, source: 'window', opener }));
        });
        if (!active) {
          registeredWindowStop();
          ensureActive();
        }
        stopWindow = registeredWindowStop;
        const registeredApplicationStop = await listen<{ intent: number }>(
          'app-exit-requested',
          (event) => {
            const opener =
              document.activeElement instanceof HTMLElement ? document.activeElement : null;
            deliver({ intent: event.payload.intent, source: 'application', opener });
          },
        );
        if (!active) {
          registeredApplicationStop();
          ensureActive();
        }
        stopApplication = registeredApplicationStop;
        const pendingIntent = await pendingAppExit();
        ensureActive();
        if (pendingIntent !== null) {
          const opener =
            document.activeElement instanceof HTMLElement ? document.activeElement : null;
          deliver({ intent: pendingIntent, source: 'application', opener });
        }
      } catch (error) {
        teardown();
        signal?.removeEventListener('abort', teardown);
        throw error;
      }
      return () => {
        teardown();
        signal?.removeEventListener('abort', teardown);
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
