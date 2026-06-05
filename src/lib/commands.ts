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
 * Get sources, optionally filtered by collection.
 * Pass null to get all sources across all collections.
 */
export async function getSources(collectionId: string | null): Promise<Source[]> {
  return invoke<Source[]>('get_sources', { collectionId });
}

/**
 * Delete a source, its blob data, and all associated chunks.
 */
export async function deleteSource(id: string): Promise<void> {
  return invoke('delete_source', { id });
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

// ── Campaign Types & Commands ──────────────────────────────────────────

export interface Campaign {
  id: string;
  name: string;
  system: string | null;
}

export async function getCampaigns(): Promise<Campaign[]> {
  return invoke<Campaign[]>('get_campaigns');
}

export async function getCampaign(id: string): Promise<Campaign> {
  return invoke<Campaign>('get_campaign', { id });
}

export async function updateCampaign(
  id: string,
  name: string,
  system: string,
): Promise<Campaign> {
  return invoke<Campaign>('update_campaign', { id, name, system });
}

export async function createCampaign(name: string, system: string): Promise<Campaign> {
  return invoke<Campaign>('create_campaign', {
    name,
    system: system || null,
  });
}

export async function deleteCampaign(id: string): Promise<void> {
  return invoke('delete_campaign', { id });
}

// MRU collection tracking (persisted in localStorage)
const MRU_KEY = 'chronacle_mru_collection_id';

export function getMruCollectionId(): string | null {
  return localStorage.getItem(MRU_KEY);
}

export function setMruCollectionId(id: string): void {
  localStorage.setItem(MRU_KEY, id);
}

// ── Embedding Model Commands ─────────────────────────────────────────

/**
 * Check whether the nomic-embed-text-v1.5 model is already cached locally.
 */
export async function checkEmbeddingModel(): Promise<boolean> {
  return invoke<boolean>('check_embedding_model');
}

export interface StaleModelCount {
  embed_model: string;
  source_count: number;
}

export interface EmbeddingModelMismatch {
  active_model: string;
  stale: StaleModelCount[];
}

/**
 * Report which indexed sources were embedded with a different model than the
 * active embedding provider. An empty `stale` array means there's no mismatch.
 */
export async function getEmbeddingModelMismatch(): Promise<EmbeddingModelMismatch> {
  return invoke<EmbeddingModelMismatch>('get_embedding_model_mismatch');
}

/**
 * Download the embedding model with streaming progress.
 * Progress is delivered via the `model-download-progress` event.
 */
export async function downloadEmbeddingModel(): Promise<void> {
  return invoke('download_embedding_model');
}

// ── Re-index all sources ─────────────────────────────────────────────

export interface ReindexProgress {
  source_id: string;
  current: number;
  total: number;
  progress: number;
  step: string;
}

/**
 * Re-run ingestion for every source currently in the database.
 *
 * Resolves with the number of sources re-indexed. While the command is
 * running, the `reindex-progress` Tauri event fires for each pipeline tick.
 * Listen with `app.listen<ReindexProgress>('reindex-progress', ...)`.
 */
export async function reindexAllSources(): Promise<number> {
  return invoke<number>('reindex_all_sources');
}

// ── Citation chunk lookup ────────────────────────────────────────────

export interface CitationChunk {
  text: string;
  page_start: number;
  page_end: number;
  section_heading: string;
}

/**
 * Look up the chunk that backs a citation, so the chat UI can show the
 * supporting passage when the user clicks a citation badge.
 *
 * `page` is the cited page number (the first integer from `p.45-49`).
 * Returns null if the source or matching chunk isn't found.
 */
export async function getChunkForCitation(
  sourceName: string,
  page: number | null,
): Promise<CitationChunk | null> {
  return await invoke<CitationChunk | null>('get_chunk_for_citation', {
    sourceName,
    page,
  });
}

// ── Entity Manager ───────────────────────────────────────────────────────────

export type EntityKind =
  | 'npc'
  | 'location'
  | 'faction'
  | 'creature'
  | 'item'
  | 'event'
  | 'player_character'
  | 'misc';

export interface GraphNode {
  id: string;
  kind: string;
  campaign_id: string | null;
  name: string;
  summary: string | null;
  notes: string | null;
  created_at: string | null;
  updated_at: string | null;
  // event fields
  date_start: string | null;
  date_end: string | null;
  is_ongoing: boolean | null;
  sequence_index: number | null; // Rust i64; safe as JS number for realistic ordering values
  era: string | null;
  duration_label: string | null;
  session_id: string | null; // event only — raw session record ID
  // player_character fields
  player_name: string | null;
  character_class: string | null;
  character_level: number | null; // Rust i64; safe as JS number for level 1-20
  status: 'active' | 'retired' | 'deceased' | 'missing' | 'on_hiatus' | null;
}

export interface EntityInput {
  name: string;
  summary?: string | null;
  notes?: string | null;
  // event
  dateStart?: string | null;
  dateEnd?: string | null;
  isOngoing?: boolean | null;
  sequenceIndex?: number | null;
  era?: string | null;
  durationLabel?: string | null;
  sessionId?: string | null; // event only — links event to a session
  // player_character
  playerName?: string | null;
  characterClass?: string | null;
  characterLevel?: number | null;
  status?: string | null;
}

export interface EntityError {
  code: 'NOT_FOUND' | 'CAMPAIGN_MISMATCH' | 'INVALID_KIND' | 'VALIDATION' | 'DATABASE';
  message: string;
  field?: string; // present on VALIDATION errors
}

export async function getEntities(campaignId: string, kind: EntityKind): Promise<GraphNode[]> {
  return invoke<GraphNode[]>('get_entities', { campaignId, kind });
}

export async function getEntity(id: string, kind: EntityKind): Promise<GraphNode> {
  return invoke<GraphNode>('get_entity', { id, kind });
}

export async function createEntity(
  campaignId: string,
  kind: EntityKind,
  input: EntityInput,
): Promise<GraphNode> {
  return invoke<GraphNode>('create_entity', { campaignId, kind, input });
}

export async function updateEntity(
  id: string,
  kind: EntityKind,
  input: EntityInput,
): Promise<GraphNode> {
  return invoke<GraphNode>('update_entity', { id, kind, input });
}

export async function deleteEntity(id: string, kind: EntityKind): Promise<void> {
  return invoke<void>('delete_entity', { id, kind });
}

export async function relateEntities(
  fromId: string,
  fromKind: EntityKind,
  toId: string,
  toKind: EntityKind,
  relType: string,
  notes?: string | null,
): Promise<void> {
  return invoke<void>('relate_entities', { fromId, fromKind, toId, toKind, relType, notes });
}

// ── Session Types & Commands ────────────────────────────────────────────

export interface Session {
  id: string;
  campaign_id: string | null;
  session_number: number;
  title: string;
  date_played: string;
  notes: string;
  created_at: string | null;
  updated_at: string | null;
}

export interface SessionInput {
  sessionNumber: number;
  title: string;
  datePlayed: string;
  notes: string;
}

export async function createSession(campaignId: string, input: SessionInput): Promise<Session> {
  return invoke<Session>('create_session', { campaignId, input });
}

export async function getSessions(campaignId: string): Promise<Session[]> {
  return invoke<Session[]>('get_sessions', { campaignId });
}

export async function getSession(id: string): Promise<Session> {
  return invoke<Session>('get_session', { id });
}

export async function updateSession(id: string, input: SessionInput): Promise<Session> {
  return invoke<Session>('update_session', { id, input });
}

export async function deleteSession(id: string): Promise<void> {
  return invoke<void>('delete_session', { id });
}

export async function getSessionEntities(sessionId: string): Promise<GraphNode[]> {
  return invoke<GraphNode[]>('get_session_entities', { sessionId });
}
