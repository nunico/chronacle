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

// ── Source Types & Commands ────────────────────────────────────────────

export interface Source {
  id: string;
  filename: string;
  display_name: string;
  source_type: string;
  page_count: number;
  index_status: string;
  embed_model: string;
  collection_id: string | null;
}

/**
 * Upload a source PDF and index it into the given collection.
 * collectionId is required — every source must belong to a collection.
 */
export async function uploadSource(
  filePath: string,
  displayName: string | undefined,
  sourceType: string | undefined,
  collectionId: string,
): Promise<Record<string, unknown>> {
  return invoke('upload_source', {
    filePath,
    displayName: displayName ?? null,
    sourceType: sourceType ?? null,
    collectionId,
  });
}

/**
 * Get sources, optionally filtered by collection.
 * Pass null to get all sources across all collections.
 */
export async function getSources(collectionId: string | null): Promise<Source[]> {
  return invoke<Source[]>('get_sources', { collectionId });
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

// ── Collection Types & Commands ────────────────────────────────────────

export interface Collection {
  id: string;
  name: string;
  description: string | null;
}

export async function getCollections(): Promise<Collection[]> {
  return invoke<Collection[]>('get_collections');
}

export async function createCollection(
  name: string,
  description?: string,
): Promise<Collection> {
  return invoke<Collection>('create_collection', {
    name,
    description: description ?? null,
  });
}

export async function updateCollection(
  id: string,
  name: string,
  description?: string,
): Promise<Collection> {
  return invoke<Collection>('update_collection', {
    id,
    name,
    description: description ?? null,
  });
}

export async function deleteCollection(id: string): Promise<void> {
  return invoke('delete_collection', { id });
}

export async function addCampaignCollection(
  campaignId: string,
  collectionId: string,
): Promise<void> {
  return invoke('add_campaign_collection', { campaignId, collectionId });
}

export async function removeCampaignCollection(
  campaignId: string,
  collectionId: string,
): Promise<void> {
  return invoke('remove_campaign_collection', { campaignId, collectionId });
}

export async function getCampaignCollections(campaignId: string): Promise<Collection[]> {
  return invoke<Collection[]>('get_campaign_collections', { campaignId });
}

// MRU collection tracking (persisted in localStorage)
const MRU_KEY = 'chronacle_mru_collection_id';

export function getMruCollectionId(): string | null {
  return localStorage.getItem(MRU_KEY);
}

export function setMruCollectionId(id: string): void {
  localStorage.setItem(MRU_KEY, id);
}
