import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/**
 * Subscribe to streaming chat tokens emitted by the backend.
 *
 * The event carries `{ token: string, done: boolean }` for each chunk.
 * Returns an unlisten function to cancel the subscription.
 */
export function onChatToken(
  callback: (payload: { token: string; done: boolean }) => void,
): Promise<UnlistenFn> {
  return listen<{ token: string; done: boolean }>('chat-token', (event) => {
    callback(event.payload);
  });
}
