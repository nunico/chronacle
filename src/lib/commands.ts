import { invoke } from '@tauri-apps/api/core';

/**
 * Retrieve all stored settings as a key-value map.
 */
export async function getSettings(): Promise<Record<string, string>> {
  return invoke<Record<string, string>>('get_settings');
}

/**
 * Upsert a single setting.
 */
export async function updateSetting(key: string, value: string): Promise<void> {
  return invoke('update_setting', { key, value });
}

/**
 * Upload a source PDF file for indexing.
 *
 * @param filePath  Absolute path to the PDF on the local filesystem.
 * @param displayName  Optional human-readable label.
 * @param sourceType  One of "rules", "lore", "supplement".
 * @returns The created source record.
 */
export async function uploadSource(
  filePath: string,
  displayName?: string,
  sourceType?: string,
): Promise<Record<string, unknown>> {
  return invoke('upload_source', {
    filePath,
    displayName: displayName ?? null,
    sourceType: sourceType ?? null,
  });
}

/**
 * Send a chat message to the AI agent (streaming response is delivered
 * via the `chat-token` event).
 */
export async function chatSend(
  message: string,
  campaignId: string | null,
): Promise<void> {
  return invoke('chat_send', {
    request: { message, campaignId },
  });
}
