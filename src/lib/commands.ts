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
 * Retrieve chat history from the message table.
 *
 * @param campaignId  Optional campaign filter; null returns global messages.
 */
export async function getChatHistory(
  campaignId: string | null,
): Promise<Array<{ role: string; content: string }>> {
  return invoke<Array<{ role: string; content: string }>>('get_chat_history', {
    campaignId,
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

/**
 * Current LLM provider status returned from the backend.
 */
export interface LlmProviderStatus {
  provider_type: string;
  model: string;
  api_key_configured: boolean;
}

/**
 * Get the current LLM provider configuration status.
 */
export async function getLlmProviderStatus(): Promise<LlmProviderStatus> {
  return invoke<LlmProviderStatus>('get_llm_provider_status');
}

/**
 * Re-read settings from the database and reconstruct the LLM provider at
 * runtime. Returns the active provider type name on success.
 */
export async function reconfigureLlmProvider(): Promise<string> {
  return invoke<string>('reconfigure_llm_provider');
}

// ── Custom Provider Types ──────────────────────────────────────────────

export interface CustomProvider {
  id: string;
  name: string;
  provider_type: string;
  base_url: string;
  api_key: string;
}

export interface CustomProviderModel {
  id: string;
  provider_id: string;
  model_id: string;
  display_name: string;
}

export async function getCustomProviders(): Promise<CustomProvider[]> {
  return invoke<CustomProvider[]>('get_custom_providers');
}

export async function createCustomProvider(
  name: string,
  providerType: string,
  baseUrl: string,
  apiKey: string,
): Promise<CustomProvider> {
  return invoke<CustomProvider>('create_custom_provider', {
    name,
    providerType,
    baseUrl,
    apiKey,
  });
}

export async function updateCustomProvider(
  id: string,
  name: string,
  providerType: string,
  baseUrl: string,
  apiKey: string,
): Promise<CustomProvider> {
  return invoke<CustomProvider>('update_custom_provider', {
    id,
    name,
    providerType,
    baseUrl,
    apiKey,
  });
}

export async function deleteCustomProvider(id: string): Promise<void> {
  return invoke('delete_custom_provider', { id });
}

export async function getProviderModels(providerId: string): Promise<CustomProviderModel[]> {
  return invoke<CustomProviderModel[]>('get_provider_models', { providerId });
}

export async function addProviderModel(
  providerId: string,
  modelId: string,
  displayName: string,
): Promise<CustomProviderModel> {
  return invoke<CustomProviderModel>('add_provider_model', {
    providerId,
    modelId,
    displayName,
  });
}

export async function removeProviderModel(id: string): Promise<void> {
  return invoke('remove_provider_model', { id });
}
