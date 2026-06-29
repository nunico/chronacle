import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { EmbeddingModelMismatch } from './commands';

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

/**
 * Subscribe to embedding-model mismatch reports emitted at startup when the
 * active embedding provider's model ID differs from the model used to index
 * existing sources (ADR-003).
 */
export function onEmbeddingModelMismatch(
  callback: (payload: EmbeddingModelMismatch) => void,
): Promise<UnlistenFn> {
  return listen<EmbeddingModelMismatch>('embedding-model-mismatch', (event) => {
    callback(event.payload);
  });
}
