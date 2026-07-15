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
export async function chatSend(message: string, campaignId: string | null): Promise<void> {
  return invoke('chat_send', {
    request: { message, campaignId },
  });
}

/**
 * Cancel the in-flight chat response, if any. The backend emits a final
 * `chat-token` event with `done: true` so the UI resolves its loading state.
 */
export async function chatCancel(): Promise<void> {
  return invoke('chat_cancel');
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

/**
 * Current embedding-provider status returned from the backend.
 */
export interface EmbeddingProviderStatus {
  backend: string; // "local" | "openai"
  model: string;
  dimension: number;
  api_key_configured: boolean;
  local_available: boolean; // ONNX Runtime bundled for this platform
  local_cached: boolean; // local nomic model downloaded
}

/**
 * Get the current embedding-provider configuration status.
 */
export async function getEmbeddingProviderStatus(): Promise<EmbeddingProviderStatus> {
  return invoke<EmbeddingProviderStatus>('get_embedding_provider_status');
}

/**
 * Re-read settings and reconstruct the embedding provider at runtime. Returns
 * the active model identity (the value stored in `embed_model`).
 */
export async function reconfigureEmbeddingProvider(): Promise<string> {
  return invoke<string>('reconfigure_embedding_provider');
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

export async function createCollection(name: string, description?: string): Promise<Collection> {
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

export async function updateCampaign(id: string, name: string, system: string): Promise<Campaign> {
  return invoke<Campaign>('update_campaign', { id, name, system });
}

export async function createCampaign(name: string, system: string): Promise<Campaign> {
  return invoke<Campaign>('create_campaign', {
    name,
    system: system || null,
  });
}

/** What happens to a campaign's owned collection when the campaign is deleted. */
export type OnOwnedCollection = 'delete' | 'convert_to_regular';

export async function deleteCampaign(
  id: string,
  onOwnedCollection: OnOwnedCollection,
): Promise<void> {
  return invoke('delete_campaign', { id, onOwnedCollection });
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
  /** Other names this entity is known by ("alternate names" to the GM). */
  aliases: string[];
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
  // codex fields
  codex_article: string | null;
  codex_stale: boolean | null;
  codex_compiled_at: string | null;
}

export interface GraphNodeRef {
  id: string;
  kind: string;
  name: string;
}

export interface GraphEdge {
  from_id: string;
  from_kind: string;
  to_id: string;
  to_kind: string;
  rel_type: string;
  notes: string | null;
}

export interface EntityGraph {
  nodes: GraphNodeRef[];
  edges: GraphEdge[];
}

export interface EntityInput {
  name: string;
  /**
   * Alternate names for this entity. ALWAYS send the complete array — the
   * backend treats an omitted/`undefined` value as "preserve the existing
   * list unchanged", so a partial edit that forgets to include this field
   * silently no-ops instead of applying.
   */
  aliases?: string[];
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

/** Campaign events in canonical timeline order (sequence_index, nulls last). */
export async function getEventsTimeline(campaignId: string): Promise<GraphNode[]> {
  return invoke<GraphNode[]>('get_events_timeline', { campaignId });
}

/** Ego graph (one hop) around an entity. Re-call on a neighbor to expand. */
export async function getEntityGraph(
  id: string,
  kind: EntityKind,
  depth = 1,
): Promise<EntityGraph> {
  return invoke<EntityGraph>('get_entity_graph', { id, kind, depth });
}

export async function getEntity(id: string, kind: EntityKind): Promise<GraphNode> {
  return invoke<GraphNode>('get_entity', { id, kind });
}

/** Per-kind entity counts for a campaign, keyed by kind (`npc`, `location`, …). */
export async function getEntityCounts(campaignId: string): Promise<Record<EntityKind, number>> {
  return invoke<Record<EntityKind, number>>('get_entity_counts', { campaignId });
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
  return invoke<never>('delete_entity', { id, kind });
}

/** Which side of a per-field conflict to keep when merging two entities. */
export type FieldChoice = 'keepSurvivor' | 'keepLoser' | 'keepBoth';

/** The GM's per-field decisions for a merge — see {@link mergeEntities}. */
export interface MergeChoices {
  summary: FieldChoice;
  notes: FieldChoice;
}

/**
 * Fold `loserId` into `survivorId`: every relationship is re-pointed onto the
 * survivor, the loser's name is kept as one of the survivor's alternate
 * names, the per-field choices are applied, and the loser is soft-deleted.
 */
export async function mergeEntities(
  survivorId: string,
  loserId: string,
  choices: MergeChoices,
): Promise<void> {
  return invoke<never>('merge_entities', { survivorId, loserId, choices });
}

/**
 * Soft-delete: the record disappears from Chronacle and (on the next vault
 * reconcile) from the vault. Hand-edited vault files outside the mirrored
 * key are left alone. Prefer this over `deleteEntity` for user-facing delete.
 */
export async function softDeleteEntity(id: string, kind: EntityKind): Promise<void> {
  return invoke<never>('soft_delete_entity', { id, kind });
}

export async function relateEntities(
  fromId: string,
  fromKind: EntityKind,
  toId: string,
  toKind: EntityKind,
  relType: string,
  notes?: string | null,
): Promise<void> {
  return invoke<never>('relate_entities', { fromId, fromKind, toId, toKind, relType, notes });
}

/** A related entity as returned by the flat relationships list. */
export interface RelatedEntity {
  id: string;
  kind: string;
  name: string;
  rel_type: string;
  /** `"outbound"` when center→other, `"inbound"` when other→center. */
  direction: 'outbound' | 'inbound';
}

/**
 * Fetch all entities related to the given entity as a flat list.
 * Includes both inbound and outbound edges. Sorted by name.
 */
export async function getEntityRelations(id: string, kind: EntityKind): Promise<RelatedEntity[]> {
  return invoke<RelatedEntity[]>('get_entity_relations', { id, kind });
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
  return invoke<never>('delete_session', { id });
}

export async function getSessionEntities(sessionId: string): Promise<GraphNode[]> {
  return invoke<GraphNode[]>('get_session_entities', { sessionId });
}

// ── Entity Extraction ────────────────────────────────────────────────────────

export interface ExtractionSummary {
  entities_created: number;
  relations_created: number;
}

export type ExtractionPhase =
  | 'resolving'
  | 'searching'
  | 'extracting'
  | 'relating'
  | 'embedding'
  | 'enriching'
  | 'done'
  | 'empty';

export interface ExtractionProgress {
  phase: ExtractionPhase;
  detail: string;
  entities_found: number;
  relations_found: number;
}

/**
 * Seed-anchored extraction of a single named entity. Progress arrives via the
 * `extract-progress` Tauri event.
 */
export async function extractEntityByName(
  campaignId: string,
  name: string,
): Promise<ExtractionSummary> {
  return invoke<ExtractionSummary>('extract_entity_by_name', { campaignId, name });
}

/** Full sweep across all collections linked to the campaign. Cancellable. */
export async function extractAllFromCampaign(campaignId: string): Promise<ExtractionSummary> {
  return invoke<ExtractionSummary>('extract_all_from_campaign', { campaignId });
}

/** Abort the in-flight extraction, if any. */
export async function cancelExtraction(): Promise<void> {
  return invoke('cancel_extraction');
}

/**
 * Re-run wikilink resolution over every existing entity in the database.
 * Forward references that never resolved at creation time become edges now that
 * all entities exist. Returns the number of entities whose notes were processed.
 */
export async function resyncWikilinks(): Promise<number> {
  return invoke<number>('resync_wikilinks');
}

// ── Codex ─────────────────────────────────────────────────────────────────────

export type CodexPhase = 'resolving' | 'compiling' | 'embedding' | 'done' | 'empty';

export interface CompileProgress {
  phase: CodexPhase;
  detail: string;
  compiled: number;
  total: number;
}

export interface CodexStatus {
  stale_entities: number;
  total_entities: number;
  rules_stale: number;
  rule_entries: number;
}

export interface CompileSummary {
  articles_compiled: number;
  remaining_stale: number;
  entries_created: number;
  entries_updated: number;
}

/** Compile codex articles for every stale entity in a collection. Progress arrives via the `codex-progress` Tauri event. */
export async function compileCollection(collectionId: string): Promise<CompileSummary> {
  return invoke<CompileSummary>('compile_collection', { collectionId });
}

/** Compile a single entity's codex article. Returns false if no context was found (article left unchanged). */
export async function compileEntity(kind: string, id: string): Promise<boolean> {
  return invoke<boolean>('compile_entity', { kind, id });
}

/** Abort the in-flight codex compile, if any. */
export async function cancelCompile(): Promise<void> {
  return invoke('cancel_compile');
}

/** Codex staleness/coverage snapshot for a collection. */
export async function getCodexStatus(collectionId: string): Promise<CodexStatus> {
  return invoke<CodexStatus>('get_codex_status', { collectionId });
}

// ── Rules Codex Types & Commands ───────────────────────────────────────────

export interface RulePageRef {
  source_name: string;
  page_start: number;
  page_end: number;
}

export interface RuleEntry {
  id: string;
  name: string;
  category: string;
  body: string;
  notes: string | null;
  page_refs: RulePageRef[];
  stale: boolean;
}

/** Retrieve all compiled rule entries for a collection. */
export async function getRuleEntries(collectionId: string): Promise<RuleEntry[]> {
  return invoke<RuleEntry[]>('get_rule_entries', { collectionId });
}

/** Update a rule entry's freeform GM notes. */
export async function updateRuleNotes(id: string, notes: string | null): Promise<void> {
  return invoke('update_rule_notes', { id, notes });
}

/** Regenerate a single rule entry honoring a new GM objection. */
export async function redoRuleEntry(id: string, objection: string): Promise<void> {
  return invoke('redo_rule_entry', { id, objection });
}

// ── Maintenance ──────────────────────────────────────────────────────────

export interface ProposalPayload {
  proposed_text: string;
  rationale: string;
  name: string | null;
  entity_kind: string | null;
  category: string | null;
}

/** Frontend-facing proposal DTO, enriched with the target's display name and
 * the current text of the field the proposal would change (for diff preview). */
export interface CodexProposal {
  id: string;
  kind: string;
  target: string | null;
  target_name: string | null;
  current_text: string | null;
  payload: ProposalPayload;
  origin_kind: string;
  status: string;
  created_at: string;
}

/** Pending work counts for the Maintenance badge. */
export interface MaintenanceCounts {
  pending_proposals: number;
  unresolved_findings: number;
}

/** Distill an assistant answer into pending codex proposals; returns the count created. */
export async function saveChatToCodex(campaignId: string, content: string): Promise<number> {
  return invoke<number>('save_chat_to_codex', { campaignId, content });
}

/** List codex proposals, optionally filtered by status ('pending', 'accepted', 'rejected'). */
export async function getProposals(status?: string): Promise<CodexProposal[]> {
  return invoke<CodexProposal[]>('get_proposals', { status: status ?? null });
}

/** Accept a proposal: applies the change, appends provenance, re-embeds. */
export async function acceptProposal(id: string): Promise<void> {
  return invoke('accept_proposal', { id });
}

/** Reject a proposal without applying it. */
export async function rejectProposal(id: string): Promise<void> {
  return invoke('reject_proposal', { id });
}

/** Pending proposals + unresolved lint findings, for the Maintenance badge. */
export async function getMaintenanceCounts(): Promise<MaintenanceCounts> {
  return invoke<MaintenanceCounts>('get_maintenance_counts');
}

/** Result of a manual "Check campaign" lint pass. */
export interface LintSummary {
  new_findings: number;
  unresolved_total: number;
}

/** One unresolved lint finding; `payload` is kind-shaped (see the detector that produced it). */
export interface LintFinding {
  id: string;
  kind: string;
  payload: Record<string, unknown>;
  created_at: string;
}

/** Run the manual lint pass over a campaign's full scope ("Check campaign"). */
export async function runLint(campaignId: string): Promise<LintSummary> {
  return invoke<LintSummary>('run_lint', { campaignId });
}

/** List unresolved lint findings for the Maintenance inbox. */
export async function getLintFindings(): Promise<LintFinding[]> {
  return invoke<LintFinding[]>('get_lint_findings');
}

/** Mark one lint finding resolved. */
export async function resolveLintFinding(id: string): Promise<void> {
  return invoke('resolve_lint_finding', { id });
}

/** Delete one `relates_to` edge by its full record id (Maintenance resolve action). */
export async function deleteRelation(edgeId: string): Promise<void> {
  return invoke('delete_relation', { edgeId });
}

/**
 * One-click "did you mean X?" confirmation: persists `alias` as a permanent
 * alternate name for `entityId`. Throws an {@link EntityError} (code
 * `VALIDATION`) if the name collides with another entity's name or
 * alternate name in the same scope.
 */
export async function confirmAliasSuggestion(entityId: string, alias: string): Promise<void> {
  return invoke('confirm_alias_suggestion', { entityId, alias });
}

/**
 * Undo an alternate name auto-created by fuzzy wikilink resolution: removes
 * the alternate name, then resolves the `auto_alias` finding that recorded
 * it so it drops out of the Maintenance inbox.
 */
export async function undoAutoAlias(
  entityId: string,
  alias: string,
  findingId: string,
): Promise<void> {
  return invoke('undo_auto_alias', { entityId, alias, findingId });
}

// ── Vault Sync ──────────────────────────────────────────────────────────

/** Wire shape of the reconcile report — snake_case matches the Rust struct. */
export interface ReconcileReport {
  exported: number;
  unchanged: number;
  adopted: number;
  applied: number;
  conflicts: number;
  resolved: number;
  soft_deleted: number;
  swept: number;
  invalid: number;
  failed: number;
}

/** The configured vault root, or null when vault sync is off. */
export function getVaultPath(): Promise<string | null> {
  return invoke('get_vault_path');
}

/** Set or clear the vault root. Setting a path runs a full reconcile. */
export function setVaultPath(vaultPath: string | null): Promise<void> {
  return invoke('set_vault_path', { vaultPath });
}

/** Run a full reconcile now. */
export function vaultSyncNow(): Promise<ReconcileReport> {
  return invoke('vault_sync_now');
}

/** One record frozen in conflict — wire shape is camelCase (`VaultConflictDto`). */
export interface VaultConflict {
  id: string;
  kind: string;
  name: string;
  key: string;
  sidecarKey: string;
}

/**
 * Every record currently frozen in conflict. Empty (never an error) when no
 * vault is configured — always safe to call unconditionally.
 */
export function listVaultConflicts(): Promise<VaultConflict[]> {
  return invoke('list_vault_conflicts');
}
