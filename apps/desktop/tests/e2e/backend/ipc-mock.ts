import type { Page } from '@playwright/test';

export type DraftWriteCommand =
  | 'create_entity'
  | 'create_session'
  | 'update_entity'
  | 'update_session'
  | 'update_rule_notes'
  | 'delete_session'
  | 'soft_delete_entity'
  | 'delete_campaign';

export interface DraftReliabilityControls {
  hold(command: DraftWriteCommand): void;
  commitPending(command: DraftWriteCommand, canonicalInput?: Record<string, unknown>): void;
  rejectPending(command: DraftWriteCommand, error: unknown): void;
  rejectNext(command: string, error: unknown): void;
  removeEntity(id: string): void;
  removeSession(id: string): void;
  removeRule(id: string): void;
  setEntityCanonical(id: string, changes: Record<string, unknown>): void;
  holdNextEntityList(campaignId: string, kind: string, omitId?: string): void;
  resolveNextEntityList(): void;
  pendingEntityLists(): number;
  holdNextSessionList(campaignId: string): void;
  resolveNextSessionList(): void;
  pendingSessionLists(): number;
  setSessionCanonical(id: string, changes: Record<string, unknown>): void;
  acknowledgeNextRuleAs(id: string): void;
  resolveNext(command: string, canonicalInput?: Record<string, unknown>): void;
  activeWrites(command: string): number;
  maxConcurrentWrites(command: string): number;
  persisted(command: string, id: string): unknown;
  pendingWrites(command: string): number;
  observations(command: string): Array<Record<string, unknown>>;
  hasEntity(id: string, kind: string): boolean;
  hasCampaign(id: string): boolean;
  requestApplicationExit(intent: number): void;
  seedMode(): DraftReliabilitySeedMode;
  submissions(): Array<Record<string, unknown>>;
}

interface DraftReliabilityOptions {
  noCampaign?: boolean;
}

export type DraftReliabilitySeedMode = 'campaigns' | 'no-campaign';

const DRAFT_RELIABILITY_SEED_PARAM = '__chronacleIpcSeed';

/**
 * Reset the page into a seeded draft/save scenario.
 *
 * The BDD fixture owns the one IPC init script. Selecting the seed through the
 * navigation URL lets that script construct deterministic state without
 * registering a competing mock script.
 */
export async function resetDraftReliabilityIpcMock(
  page: Page,
  options: DraftReliabilityOptions = {},
): Promise<void> {
  const seedMode: DraftReliabilitySeedMode = options.noCampaign ? 'no-campaign' : 'campaigns';
  await page.goto(`/?${DRAFT_RELIABILITY_SEED_PARAM}=${seedMode}`);
}

/**
 * Install the Tauri IPC mock into the page before app scripts run.
 *
 * We mock window.__TAURI_INTERNALS__.invoke directly instead of importing
 * mockIPC from @tauri-apps/api/mocks, because addInitScript runs in the
 * browser context where the module isn't available.
 *
 * Every invoke() is recorded into window.__ipcCalls so tests and BDD steps
 * can assert which commands were (not) sent.
 *
 * @param overrides - Optional map of command names to response values to override defaults
 * @param postSyncOverrides - Optional map of command names to response values that only take
 *   effect once `vault_sync_now` has been dispatched at least once. Use this to make a
 *   scenario's "after sync" reads (e.g. a subsequent `get_entities` or `list_vault_conflicts`
 *   call) causally depend on the sync having actually run, instead of baking the post-sync
 *   state into the very first page load. Commands not listed here fall back to `overrides`
 *   (or the built-in default) both before and after the sync.
 * @param postEmbeddingReconfigureOverrides - Optional command responses that take effect after
 *   `reconfigure_embedding_provider`. A supplied mismatch report is also emitted as the live
 *   mismatch event, matching the desktop command's behavior.
 */
export async function installIpcMock(
  page: Page,
  overrides?: Record<string, unknown>,
  postSyncOverrides?: Record<string, unknown>,
  postEmbeddingReconfigureOverrides?: Record<string, unknown>,
): Promise<void> {
  await page.addInitScript(
    ({ overridesArg, postSyncOverridesArg, postEmbeddingReconfigureOverridesArg, seedParam }) => {
      let _cbId = 0;
      let _synced = false;
      let _embeddingReconfigured = false;
      const callbacks = new Map<number, (event: unknown) => void>();
      const requestedSeed = new URLSearchParams(window.location.search).get(seedParam);
      const seedMode =
        requestedSeed === 'campaigns' || requestedSeed === 'no-campaign' ? requestedSeed : null;
      const reliability = seedMode
        ? (() => {
            interface Write {
              args: Record<string, unknown>;
              resolve: (value: unknown) => void;
              committed: boolean;
              committedValue?: unknown;
            }

            interface PendingEntityList {
              resolve: (value: unknown) => void;
              value: unknown;
            }

            interface PendingSessionList {
              resolve: (value: unknown) => void;
              value: unknown;
            }

            const campaigns =
              seedMode === 'no-campaign'
                ? []
                : [
                    { id: 'camp-a', name: 'Campaign A', system: 'D&D 5e' },
                    { id: 'camp-b', name: 'Campaign B', system: 'D&D 5e' },
                  ];
            const entities = [
              {
                id: 'mira',
                kind: 'npc',
                campaign_id: 'camp-a',
                name: 'Mira',
                aliases: [],
                summary: 'A careful archivist.',
                notes: 'Mira saved notes.',
                created_at: null,
                updated_at: null,
                date_start: null,
                date_end: null,
                is_ongoing: null,
                sequence_index: null,
                era: null,
                duration_label: null,
                session_id: null,
                player_name: null,
                character_class: null,
                character_level: null,
                status: null,
                codex_article: 'Aldric appears in the records as [[Aldric]].',
                codex_stale: false,
                codex_compiled_at: null,
              },
              {
                id: 'torvin',
                kind: 'npc',
                campaign_id: 'camp-a',
                name: 'Torvin',
                aliases: [],
                summary: 'A veteran guide.',
                notes: 'Torvin saved notes.',
                created_at: null,
                updated_at: null,
                date_start: null,
                date_end: null,
                is_ongoing: null,
                sequence_index: null,
                era: null,
                duration_label: null,
                session_id: null,
                player_name: null,
                character_class: null,
                character_level: null,
                status: null,
                codex_article: null,
                codex_stale: false,
                codex_compiled_at: null,
              },
              {
                id: 'mira',
                kind: 'location',
                campaign_id: 'camp-a',
                name: 'Mira',
                aliases: [],
                summary: "A place sharing the archivist's name.",
                notes: 'Location Mira saved notes.',
                created_at: null,
                updated_at: null,
                date_start: null,
                date_end: null,
                is_ongoing: null,
                sequence_index: null,
                era: null,
                duration_label: null,
                session_id: null,
                player_name: null,
                character_class: null,
                character_level: null,
                status: null,
                codex_article: null,
                codex_stale: false,
                codex_compiled_at: null,
              },
            ];
            const sessions = [
              {
                id: 'session-a',
                campaign_id: 'camp-a',
                session_number: 1,
                title: 'Ashes at Dawn',
                date_played: '2026-09-01',
                notes: 'The party reached the old road.',
                created_at: null,
                updated_at: null,
              },
              {
                id: 'session-b',
                campaign_id: 'camp-b',
                session_number: 1,
                title: 'Lanterns in Rain',
                date_played: '2026-09-02',
                notes: 'Campaign B followed the river road.',
                created_at: null,
                updated_at: null,
              },
            ];
            const collections = [
              { id: 'world-guide', name: 'World Guide', description: 'Campaign reference' },
              {
                id: 'adventurer-guide',
                name: 'Adventurer Guide',
                description: 'Player-facing reference',
              },
            ];
            const rules = [
              {
                id: 'initiative',
                collection_id: 'world-guide',
                name: 'Initiative',
                category: 'mechanic',
                body: 'Roll a d20 and add Dexterity to determine turn order.',
                notes: 'Initiative saved note.',
                page_refs: [{ source_name: 'Core Rulebook', page_start: 12, page_end: 12 }],
                stale: false,
              },
              {
                id: 'adventurer-initiative',
                collection_id: 'adventurer-guide',
                name: 'Initiative',
                category: 'mechanic',
                body: 'The adventurer guide uses the standard initiative procedure.',
                notes: 'Adventurer initiative saved note.',
                page_refs: [{ source_name: 'Adventurer Guide', page_start: 8, page_end: 8 }],
                stale: false,
              },
            ];
            const held = new Set<string>();
            const queuedErrors = new Map<string, unknown[]>();
            const pending = new Map<string, Write[]>();
            const active = new Map<string, number>();
            const maximum = new Map<string, number>();
            const observed = new Map<string, Array<Record<string, unknown>>>();
            const chatSubmissions: Array<Record<string, unknown>> = [];
            let heldEntityList: { campaignId: string; kind: string; omitId?: string } | null = null;
            const pendingEntityLists: PendingEntityList[] = [];
            let heldSessionListCampaign: string | null = null;
            const pendingSessionLists: PendingSessionList[] = [];

            const copy = <T>(value: T): T =>
              value === undefined ? value : (JSON.parse(JSON.stringify(value)) as T);

            function applyWrite(command: string, args: Record<string, unknown>): unknown {
              if (command === 'update_entity') {
                const entity = entities.find(
                  (candidate) => candidate.id === args.id && candidate.kind === args.kind,
                );
                if (!entity) throw { code: 'NOT_FOUND', message: 'Entity no longer available.' };
                const input = (args.input ?? {}) as Record<string, unknown>;
                Object.assign(entity, {
                  name: input.name ?? entity.name,
                  aliases: input.aliases ?? entity.aliases,
                  summary: input.summary ?? null,
                  notes: input.notes ?? null,
                  date_start: input.dateStart ?? null,
                  date_end: input.dateEnd ?? null,
                  is_ongoing: input.isOngoing ?? null,
                  sequence_index: input.sequenceIndex ?? null,
                  era: input.era ?? null,
                  duration_label: input.durationLabel ?? null,
                  session_id: input.sessionId ?? null,
                  player_name: input.playerName ?? null,
                  character_class: input.characterClass ?? null,
                  character_level: input.characterLevel ?? null,
                  status: input.status ?? null,
                });
                return copy(entity);
              }
              if (command === 'update_session') {
                const session = sessions.find((candidate) => candidate.id === args.id);
                if (!session) throw { code: 'NOT_FOUND', message: 'Session no longer available.' };
                const input = (args.input ?? {}) as Record<string, unknown>;
                Object.assign(session, {
                  session_number: input.sessionNumber ?? session.session_number,
                  title: input.title ?? session.title,
                  date_played: input.datePlayed ?? session.date_played,
                  notes: input.notes ?? session.notes,
                });
                return copy(session);
              }
              if (command === 'update_rule_notes') {
                const rule = rules.find((candidate) => candidate.id === args.id);
                if (!rule) throw { code: 'NOT_FOUND', message: 'Rule no longer available.' };
                rule.notes = (args.notes as string | null) ?? null;
                return copy(rule);
              }
              if (command === 'delete_session') {
                const index = sessions.findIndex((session) => session.id === args.id);
                if (index < 0) {
                  throw { code: 'NOT_FOUND', message: 'Session no longer available.' };
                }
                sessions.splice(index, 1);
                return null;
              }
              if (command === 'soft_delete_entity') {
                const index = entities.findIndex(
                  (entity) => entity.id === args.id && entity.kind === args.kind,
                );
                if (index < 0) {
                  throw { code: 'NOT_FOUND', message: 'Entity no longer available.' };
                }
                entities.splice(index, 1);
                return null;
              }
              if (command === 'delete_campaign') {
                const index = campaigns.findIndex((campaign) => campaign.id === args.id);
                if (index < 0) {
                  throw { code: 'NOT_FOUND', message: 'Campaign no longer available.' };
                }
                campaigns.splice(index, 1);
                return null;
              }
              if (command === 'create_entity') {
                const input = (args.input ?? {}) as Record<string, unknown>;
                const created = {
                  ...copy(entities[0]),
                  id: `created-${
                    entities.filter((entity) => entity.kind === args.kind).length + 1
                  }`,
                  kind: args.kind as string,
                  campaign_id: args.campaignId as string,
                  name: String(input.name ?? ''),
                  aliases: input.aliases ?? [],
                  summary: input.summary ?? null,
                  notes: input.notes ?? null,
                };
                entities.push(created);
                return copy(created);
              }
              if (command === 'create_session') {
                const input = (args.input ?? {}) as Record<string, unknown>;
                const created = {
                  id: `created-session-${sessions.length + 1}`,
                  campaign_id: args.campaignId as string,
                  session_number: input.sessionNumber as number,
                  title: String(input.title ?? ''),
                  date_played: String(input.datePlayed ?? ''),
                  notes: String(input.notes ?? ''),
                  created_at: null,
                  updated_at: null,
                };
                sessions.push(created);
                return copy(created);
              }
              return null;
            }

            function finish(
              command: string,
              write: Write,
              canonicalInput?: Record<string, unknown>,
            ): void {
              try {
                if (write.committed) {
                  active.set(command, Math.max(0, (active.get(command) ?? 1) - 1));
                  write.resolve(copy(write.committedValue));
                  return;
                }
                const args = canonicalInput
                  ? {
                      ...write.args,
                      input: {
                        ...((write.args.input ?? {}) as Record<string, unknown>),
                        ...canonicalInput,
                      },
                    }
                  : write.args;
                const value = applyWrite(command, args);
                active.set(command, Math.max(0, (active.get(command) ?? 1) - 1));
                write.resolve(value);
              } catch (error) {
                active.set(command, Math.max(0, (active.get(command) ?? 1) - 1));
                write.resolve(Promise.reject(error));
              }
            }

            function invokeWrite(command: string, args: Record<string, unknown>): Promise<unknown> {
              const attempts = observed.get(command) ?? [];
              attempts.push(copy(args));
              observed.set(command, attempts);
              const count = (active.get(command) ?? 0) + 1;
              active.set(command, count);
              maximum.set(command, Math.max(maximum.get(command) ?? 0, count));

              const errorQueue = queuedErrors.get(command);
              const nextError = errorQueue?.shift();
              if (nextError !== undefined) {
                active.set(command, count - 1);
                return Promise.reject(nextError);
              }
              if (!held.has(command)) {
                try {
                  const value = applyWrite(command, args);
                  active.set(command, count - 1);
                  return Promise.resolve(value);
                } catch (error) {
                  active.set(command, count - 1);
                  return Promise.reject(error);
                }
              }
              return new Promise((resolve) => {
                const writes = pending.get(command) ?? [];
                writes.push({ args: copy(args), resolve, committed: false });
                pending.set(command, writes);
              });
            }

            function invokeEntityList(args: Record<string, unknown>): Promise<unknown> {
              const holdMatches =
                heldEntityList?.campaignId === args.campaignId && heldEntityList.kind === args.kind;
              const value = copy(
                entities.filter(
                  (entity) =>
                    entity.campaign_id === args.campaignId &&
                    entity.kind === args.kind &&
                    (!holdMatches || entity.id !== heldEntityList?.omitId),
                ),
              );
              if (!holdMatches) return Promise.resolve(value);
              heldEntityList = null;
              return new Promise((resolve) => {
                pendingEntityLists.push({ resolve, value });
              });
            }

            function invokeSessionList(args: Record<string, unknown>): Promise<unknown> {
              const value = copy(
                sessions.filter((session) => session.campaign_id === args.campaignId),
              );
              if (heldSessionListCampaign !== args.campaignId) return Promise.resolve(value);
              heldSessionListCampaign = null;
              return new Promise((resolve) => {
                pendingSessionLists.push({ resolve, value });
              });
            }

            const controls = {
              hold(command: string) {
                held.add(command);
              },
              commitPending(command: string, canonicalInput?: Record<string, unknown>) {
                const writes = pending.get(command) ?? [];
                const next = writes[0];
                if (!next) throw new Error(`No pending ${command} write to commit.`);
                if (next.committed) throw new Error(`The pending ${command} write is committed.`);
                const args = canonicalInput
                  ? {
                      ...next.args,
                      input: {
                        ...((next.args.input ?? {}) as Record<string, unknown>),
                        ...canonicalInput,
                      },
                    }
                  : next.args;
                next.committedValue = applyWrite(command, args);
                next.committed = true;
              },
              rejectNext(command: string, error: unknown) {
                const errors = queuedErrors.get(command) ?? [];
                errors.push(copy(error));
                queuedErrors.set(command, errors);
              },
              rejectPending(command: string, error: unknown) {
                const writes = pending.get(command) ?? [];
                const next = writes.shift();
                if (!next) throw new Error(`No pending ${command} write to reject.`);
                active.set(command, Math.max(0, (active.get(command) ?? 1) - 1));
                next.resolve(Promise.reject(copy(error)));
              },
              removeEntity(id: string) {
                const index = entities.findIndex((entity) => entity.id === id);
                if (index >= 0) entities.splice(index, 1);
              },
              removeSession(id: string) {
                const index = sessions.findIndex((session) => session.id === id);
                if (index >= 0) sessions.splice(index, 1);
              },
              removeRule(id: string) {
                const index = rules.findIndex((rule) => rule.id === id);
                if (index >= 0) rules.splice(index, 1);
              },
              setEntityCanonical(id: string, changes: Record<string, unknown>) {
                const entity = entities.find((candidate) => candidate.id === id);
                if (!entity) throw new Error(`No entity ${id} to update.`);
                Object.assign(entity, copy(changes));
              },
              holdNextEntityList(campaignId: string, kind: string, omitId?: string) {
                if (heldEntityList) throw new Error('An entity list hold is already armed.');
                heldEntityList = { campaignId, kind, omitId };
              },
              resolveNextEntityList() {
                const next = pendingEntityLists.shift();
                if (!next) throw new Error('No pending entity list to resolve.');
                next.resolve(copy(next.value));
              },
              pendingEntityLists() {
                return pendingEntityLists.length;
              },
              holdNextSessionList(campaignId: string) {
                if (heldSessionListCampaign)
                  throw new Error('A session list hold is already armed.');
                heldSessionListCampaign = campaignId;
              },
              resolveNextSessionList() {
                const next = pendingSessionLists.shift();
                if (!next) throw new Error('No pending session list to resolve.');
                next.resolve(copy(next.value));
              },
              pendingSessionLists() {
                return pendingSessionLists.length;
              },
              setSessionCanonical(id: string, changes: Record<string, unknown>) {
                const session = sessions.find((candidate) => candidate.id === id);
                if (!session) throw new Error(`No session ${id} to update.`);
                Object.assign(session, copy(changes));
              },
              acknowledgeNextRuleAs(id: string) {
                const writes = pending.get('update_rule_notes') ?? [];
                const next = writes.shift();
                if (!next) throw new Error('No pending rule-note write to acknowledge.');
                const rule = rules.find((candidate) => candidate.id === id);
                if (!rule) throw new Error(`No rule ${id} to acknowledge.`);
                active.set(
                  'update_rule_notes',
                  Math.max(0, (active.get('update_rule_notes') ?? 1) - 1),
                );
                next.resolve(copy(rule));
              },
              resolveNext(command: string, canonicalInput?: Record<string, unknown>) {
                const writes = pending.get(command) ?? [];
                const next = writes.shift();
                if (!next) throw new Error(`No pending ${command} write to resolve.`);
                finish(command, next, canonicalInput);
              },
              activeWrites(command: string) {
                return active.get(command) ?? 0;
              },
              maxConcurrentWrites(command: string) {
                return maximum.get(command) ?? 0;
              },
              persisted(command: string, id: string) {
                if (command === 'update_entity' || command === 'create_entity') {
                  return copy(entities.find((entity) => entity.id === id && entity.kind === 'npc'));
                }
                if (command === 'update_session') {
                  return copy(sessions.find((session) => session.id === id));
                }
                if (command === 'update_rule_notes') {
                  return copy(rules.find((rule) => rule.id === id));
                }
                return undefined;
              },
              pendingWrites(command: string) {
                return pending.get(command)?.length ?? 0;
              },
              observations(command: string) {
                return copy(observed.get(command) ?? []);
              },
              hasEntity(id: string, kind: string) {
                return entities.some((entity) => entity.id === id && entity.kind === kind);
              },
              hasCampaign(id: string) {
                return campaigns.some((campaign) => campaign.id === id);
              },
              requestApplicationExit(intent: number) {
                for (const call of window.__ipcCalls) {
                  if (
                    call.cmd !== 'plugin:event|listen' ||
                    call.args?.event !== 'app-exit-requested'
                  ) {
                    continue;
                  }
                  const callback = callbacks.get(call.args.handler as number);
                  callback?.({ event: 'app-exit-requested', payload: { intent } });
                }
              },
              seedMode() {
                return seedMode;
              },
              submissions() {
                return copy(chatSubmissions);
              },
            };

            return {
              campaigns,
              entities,
              sessions,
              collections,
              rules,
              chatSubmissions,
              controls,
              invokeWrite,
              invokeEntityList,
              invokeSessionList,
              copy,
            };
          })()
        : null;
      // @ts-expect-error -- injected by Tauri at runtime
      // eslint-disable-next-line @typescript-eslint/no-empty-function
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      // @ts-expect-error -- test-only call log
      window.__ipcCalls = [] as Array<{ cmd: string; args?: Record<string, unknown> }>;
      if (reliability) {
        // @ts-expect-error -- test-only deterministic draft/save controls
        window.__draftReliability = reliability.controls;
        // Match the native runtime boundary for reliability scenarios so the
        // Shell registers its real close port against this deterministic IPC.
        // @ts-expect-error -- Tauri exposes this runtime marker globally.
        window.isTauri = true;
      }
      // @ts-expect-error -- injected by Tauri at runtime
      window.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: 'main' } },
        transformCallback: (cb: unknown, _once?: boolean) => {
          const id = ++_cbId;
          callbacks.set(id, cb as (event: unknown) => void);
          return id;
        },
        invoke: (cmd: string, args?: Record<string, unknown>) => {
          // @ts-expect-error -- test-only call log
          window.__ipcCalls.push({ cmd, args });
          // Decide before flipping the flag: the vault_sync_now call itself is
          // always answered from the pre-sync overrides (its report), never
          // from postSyncOverrides.
          const usePostSync = _synced && postSyncOverridesArg && cmd in postSyncOverridesArg;
          const usePostEmbeddingReconfigure =
            _embeddingReconfigured &&
            postEmbeddingReconfigureOverridesArg &&
            cmd in postEmbeddingReconfigureOverridesArg;
          if (cmd === 'vault_sync_now') {
            _synced = true;
          }
          if (cmd === 'reconfigure_embedding_provider') {
            _embeddingReconfigured = true;
            const report = postEmbeddingReconfigureOverridesArg?.get_embedding_model_mismatch;
            if (report && typeof report === 'object') {
              for (const call of window.__ipcCalls) {
                if (
                  call.cmd !== 'plugin:event|listen' ||
                  call.args?.event !== 'embedding-model-mismatch'
                ) {
                  continue;
                }
                const callback = callbacks.get(call.args.handler as number);
                callback?.({ event: 'embedding-model-mismatch', payload: report });
              }
            }
          }
          if (usePostEmbeddingReconfigure) {
            return Promise.resolve(
              postEmbeddingReconfigureOverridesArg[
                cmd as keyof typeof postEmbeddingReconfigureOverridesArg
              ],
            );
          }
          if (usePostSync) {
            return Promise.resolve(postSyncOverridesArg[cmd as keyof typeof postSyncOverridesArg]);
          }
          if (reliability) {
            switch (cmd) {
              case 'pending_app_exit':
                return Promise.resolve(null);
              case 'request_app_exit':
                return Promise.resolve(1);
              case 'cancel_app_exit':
                return Promise.resolve(true);
              case 'confirm_app_exit':
                return Promise.resolve(null);
              case 'get_campaigns':
                return Promise.resolve(reliability.copy(reliability.campaigns));
              case 'get_entity_counts': {
                const counts: Record<string, number> = {};
                for (const entity of reliability.entities) {
                  if (entity.campaign_id === args?.campaignId) {
                    counts[entity.kind] = (counts[entity.kind] ?? 0) + 1;
                  }
                }
                return Promise.resolve(counts);
              }
              case 'get_sessions':
                return reliability.invokeSessionList(args ?? {});
              case 'get_entities':
                return reliability.invokeEntityList(args ?? {});
              case 'get_entity':
                return Promise.resolve(
                  reliability.copy(
                    reliability.entities.find((entity) => entity.id === args?.id) ?? null,
                  ),
                );
              case 'get_entity_relations':
              case 'get_session_entities':
              case 'get_sources':
                return Promise.resolve([]);
              case 'get_collections':
                return Promise.resolve(reliability.copy(reliability.collections));
              case 'get_campaign_collections':
                return Promise.resolve(
                  args?.campaignId ? reliability.copy(reliability.collections) : [],
                );
              case 'get_codex_status':
                return Promise.resolve({
                  stale_entities: 0,
                  total_entities: reliability.entities.length,
                  rules_stale: 0,
                  rule_entries: reliability.rules.length,
                });
              case 'get_rule_entries':
                return Promise.resolve(
                  reliability.copy(
                    reliability.rules.filter((rule) => rule.collection_id === args?.collectionId),
                  ),
                );
              case 'create_campaign': {
                const created = {
                  id: `created-campaign-${reliability.campaigns.length + 1}`,
                  name: String(args?.name ?? ''),
                  system: (args?.system as string | null) ?? null,
                };
                reliability.campaigns.push(created);
                return Promise.resolve(reliability.copy(created));
              }
              case 'create_session':
              case 'update_entity':
              case 'update_session':
              case 'update_rule_notes':
              case 'create_entity':
              case 'delete_session':
              case 'soft_delete_entity':
              case 'delete_campaign':
                return reliability.invokeWrite(cmd, args ?? {});
              case 'chat_send':
                reliability.chatSubmissions.push(reliability.copy(args ?? {}));
                return Promise.resolve(null);
            }
          }
          // Check overrides first
          if (overridesArg && cmd in overridesArg) {
            return Promise.resolve(overridesArg[cmd as keyof typeof overridesArg]);
          }
          switch (cmd) {
            case 'plugin:event|listen':
              return Promise.resolve(0);
            case 'plugin:event|unlisten':
              return Promise.resolve(null);
            case 'plugin:os|locale':
              return Promise.resolve('en-US');
            case 'get_embedding_provider_status':
              return Promise.resolve({
                backend: 'openai',
                model: 'text-embedding-3-small',
                dimension: 1536,
                api_key_configured: true,
                local_available: false,
                local_cached: false,
              });
            case 'get_embedding_model_mismatch':
              return Promise.resolve({ active_model: 'nomic-embed-text-v1.5', stale: [] });
            case 'get_campaigns':
              return Promise.resolve([{ id: 'camp1', name: 'Test Campaign', system: 'D&D 5e' }]);
            case 'get_entity_counts':
              return Promise.resolve({});
            case 'get_sessions':
              return Promise.resolve([]);
            case 'get_entities':
              return Promise.resolve([]);
            case 'get_entity_relations':
              return Promise.resolve([]);
            case 'get_collections':
              return Promise.resolve([]);
            case 'get_sources':
              return Promise.resolve([]);
            case 'get_settings':
              return Promise.resolve({
                llm_provider: 'openai',
                llm_model: 'gpt-4o-mini',
                llm_api_key: 'sk-test',
                llm_base_url: '',
                active_campaign_id: '',
              });
            case 'update_setting':
              return Promise.resolve(null);
            case 'get_llm_provider_status':
              return Promise.resolve({
                provider_type: 'openai',
                model: 'gpt-4o-mini',
                api_key_configured: true,
              });
            case 'get_chat_history':
              return Promise.resolve([]);
            case 'chat_send':
              return Promise.resolve(null);
            case 'get_custom_providers':
              return Promise.resolve([]);
            case 'get_provider_models':
              return Promise.resolve([]);
            case 'delete_campaign':
              return Promise.resolve(null);
            case 'get_campaign_collections':
              return Promise.resolve([]);
            case 'get_codex_status':
              return Promise.resolve({
                stale_entities: 12,
                total_entities: 40,
                rules_stale: 0,
                rule_entries: 0,
              });
            case 'compile_collection':
              return Promise.resolve({ articles_compiled: 12, remaining_stale: 0 });
            case 'get_rule_entries':
              return Promise.resolve([
                {
                  id: 'rule1',
                  name: 'Initiative',
                  category: 'mechanic',
                  body: 'Roll a d20 and add your Dexterity modifier to determine turn order.',
                  notes: null,
                  page_refs: [{ source_name: 'Core Rulebook', page_start: 12, page_end: 13 }],
                  stale: false,
                },
              ]);
            case 'update_rule_notes':
              if (args?.id !== 'rule1') {
                return Promise.reject({
                  code: 'NOT_FOUND',
                  message: `Rule entry ${String(args?.id ?? '')} not found`,
                });
              }
              return Promise.resolve({
                id: 'rule1',
                name: 'Initiative',
                category: 'mechanic',
                body: 'Roll a d20 and add your Dexterity modifier to determine turn order.',
                notes: typeof args?.notes === 'string' && args.notes.length > 0 ? args.notes : null,
                page_refs: [{ source_name: 'Core Rulebook', page_start: 12, page_end: 13 }],
                stale: false,
              });
            case 'redo_rule_entry':
              return Promise.resolve(null);
            case 'save_chat_to_codex':
              return Promise.resolve(0);
            case 'get_proposals':
              return Promise.resolve([]);
            case 'get_maintenance_counts':
              return Promise.resolve({ pending_proposals: 0, unresolved_findings: 0 });
            case 'accept_proposal':
              return Promise.resolve(null);
            case 'reject_proposal':
              return Promise.resolve(null);
            case 'get_vault_path':
              return Promise.resolve(null);
            case 'set_vault_path':
              return Promise.resolve(null);
            case 'vault_sync_now':
              return Promise.resolve({
                exported: 0,
                unchanged: 0,
                adopted: 0,
                applied: 0,
                conflicts: 0,
                resolved: 0,
                soft_deleted: 0,
                swept: 0,
                invalid: 0,
                failed: 0,
              });
            case 'list_vault_conflicts':
              return Promise.resolve([]);
            case 'soft_delete_entity':
              return Promise.resolve(null);
            default:
              console.warn(`Unhandled IPC mock: ${cmd}`);
              return Promise.resolve(null);
          }
        },
      };
    },
    {
      overridesArg: overrides,
      postSyncOverridesArg: postSyncOverrides,
      postEmbeddingReconfigureOverridesArg: postEmbeddingReconfigureOverrides,
      seedParam: DRAFT_RELIABILITY_SEED_PARAM,
    },
  );
}
