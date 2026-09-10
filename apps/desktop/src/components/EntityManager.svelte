<script module lang="ts">
  let fallbackDraftId = 0;

  function allocateNewDraftId(): string {
    fallbackDraftId += 1;
    return globalThis.crypto?.randomUUID?.() ?? `draft-${fallbackDraftId}`;
  }
</script>

<script lang="ts">
  import { SvelteMap } from 'svelte/reactivity';
  import { onDestroy, tick, untrack } from 'svelte';
  import {
    getEntities,
    getEntity,
    createEntity,
    updateEntity,
    softDeleteEntity,
    getSessions,
    compileEntity,
    type EntityKind,
    type GraphNode,
    type EntityInput,
    type EntityError,
    type Session,
  } from '../lib/commands';
  import EntityForm, { type EntityDraftValue } from './EntityForm.svelte';
  import SaveStatus from './SaveStatus.svelte';
  import WikiText from './WikiText.svelte';
  import { buildWikiLinkEntityMap } from '../lib/wikilinks';
  import { i18n } from '../lib/locale.svelte';
  import type { MessageKey } from '../lib/i18n/messages';
  import Button from './ui/Button.svelte';
  import Dialog from './ui/Dialog.svelte';
  import {
    DraftCoordinator,
    type CreatePromotionResult,
  } from '../lib/drafts/draft-coordinator.svelte';
  import {
    entityScope,
    newEntityScope,
    statusOf,
    type DraftRecord,
    type DraftSnapshot,
    type DraftStatus,
  } from '../lib/drafts/draft-state';

  interface PendingCreate {
    kind: EntityKind;
    name: string;
    sourceFindingId?: string;
  }

  interface ScopedEntityError {
    scope: string;
    error: EntityError;
  }

  interface EntityDeletionIntent {
    readonly campaignId: string;
    readonly kind: EntityKind;
    readonly entityId: string;
    readonly scope: string;
    readonly name: string;
    readonly rowIndex: number;
  }

  interface Props {
    campaignId: string;
    kind: EntityKind;
    /// Bumped by the `c` shortcut to start creating a new entity.
    createNonce?: number;
    /// Set to an entity id to open its edit form once entities are loaded (deep-link).
    openId?: string | null;
    /// Called immediately after the deep-link edit form is opened so the caller
    /// can clear openId and prevent the effect from re-firing on entity list mutations.
    onOpenIdConsumed?: () => void;
    /// Called when the user clicks the "Graph" button on an entity row.
    onViewGraph?: (node: GraphNode) => void;
    /// Called when the user clicks a related entity in the Relationships section.
    onOpenEntity?: (id: string, kind: string) => void;
    pendingCreate?: PendingCreate | null;
    onPendingCreateConsumed?: () => void;
    onPendingCreateSaved?: (sourceFindingId: string) => void;
    onMissingLinkClick?: (name: string) => void;
    draftCoordinator?: DraftCoordinator;
  }

  let {
    campaignId,
    kind,
    createNonce = 0,
    openId = null,
    onOpenIdConsumed,
    onViewGraph,
    onOpenEntity,
    pendingCreate = null,
    onPendingCreateConsumed,
    onPendingCreateSaved,
    onMissingLinkClick,
    draftCoordinator = new DraftCoordinator(),
  }: Props = $props();

  const KIND_LABEL: Record<EntityKind, MessageKey> = {
    npc: 'entityUi.kindNpc',
    location: 'entityUi.kindLocation',
    faction: 'entityUi.kindFaction',
    creature: 'entityUi.kindCreature',
    item: 'entityUi.kindItem',
    event: 'entityUi.kindEvent',
    player_character: 'entityUi.kindPlayerCharacter',
    misc: 'entityUi.kindMisc',
  };
  let entities = $state<GraphNode[]>([]);
  let loading = $state(false);
  let formNode = $state<GraphNode | null>(null); // null = create, non-null = edit
  let showForm = $state(false);
  let formError = $state<ScopedEntityError | null>(null);
  let toast = $state<string | null>(null);
  let deleteConfirm = $state<EntityDeletionIntent | null>(null);
  let deletingScope = $state<string | null>(null);
  let deleteProgress = $state<HTMLButtonElement>();
  // SvelteMap is inherently reactive — no $state wrapper needed
  let entityMap = new SvelteMap<string, { id: string; kind: string }>();
  let sessions = $state<Session[]>([]);
  let recompiling = $state(false);
  let pendingInitialName = $state<string | null>(null);
  let pendingSourceFindingId = $state<string | null>(null);
  let blockedPendingCreate = $state<PendingCreate | null>(null);
  let consumedPendingCreate = $state<PendingCreate | null>(null);
  let loadedScope = $state<string | null>(null);
  let activeDraftScope = $state<string | null>(null);
  let activeRecordId = $state<string | null>(null);
  let discardConfirm = $state(false);
  let activeTargetUnavailable = $state(false);
  let managerElement = $state<HTMLDivElement>();
  let formPanel = $state<HTMLDivElement>();
  let consumedOpenKey = $state<string | null>(null);
  let mounted = true;
  let activeExistingProjectionPrefix: string | null = null;
  let activeNewProjectionPrefix: string | null = null;
  const projectionLeases = new SvelteMap<string, () => void>();

  function leaseProjection(scope: string): void {
    if (!projectionLeases.has(scope)) {
      projectionLeases.set(scope, draftCoordinator.acquireLease(scope));
    }
  }

  function releaseProjection(scope: string): void {
    const releaseLease = projectionLeases.get(scope);
    if (!releaseLease) return;
    draftCoordinator.release(scope);
    releaseLease();
    projectionLeases.delete(scope);
  }

  function releaseProjectionPrefix(prefix: string): void {
    for (const scope of [...projectionLeases.keys()]) {
      if (scope.startsWith(prefix)) releaseProjection(scope);
    }
  }

  onDestroy(() => {
    mounted = false;
    for (const scope of [...projectionLeases.keys()]) releaseProjection(scope);
  });
  let resolvedActiveDraftScope = $derived(
    activeDraftScope ? draftCoordinator.resolveScope(activeDraftScope) : null,
  );
  let currentDraft = $derived(
    resolvedActiveDraftScope
      ? draftCoordinator.get<EntityDraftValue>(resolvedActiveDraftScope)
      : undefined,
  );
  let presentedFormNode = $derived(
    formNode && currentDraft ? nodeWithDraftValue(formNode, currentDraft.value) : formNode,
  );
  let activeCreatePromotionIssue = $derived(
    activeDraftScope ? draftCoordinator.getCreatePromotionIssue(activeDraftScope) : undefined,
  );
  let newDraftPrefix = $derived(`entity-new:${campaignId}:${kind}:`);
  let existingDraftPrefix = $derived(`entity:${campaignId}:${kind}:`);
  let retainedNewDrafts = $derived(draftCoordinator.listByPrefix<EntityDraftValue>(newDraftPrefix));
  let retainedExistingDrafts = $derived(
    draftCoordinator.listByPrefix<EntityDraftValue>(existingDraftPrefix),
  );
  let creating = $derived(
    activeDraftScope?.startsWith(newDraftPrefix) === true &&
      resolvedActiveDraftScope === activeDraftScope &&
      (currentDraft?.inFlight !== null || activeCreatePromotionIssue !== undefined),
  );
  let activeDeletionPending = $derived(
    deletingScope !== null && deletingScope === resolvedActiveDraftScope,
  );
  let anyModalOpen = $derived(
    deleteConfirm !== null || blockedPendingCreate !== null || discardConfirm,
  );

  interface PresentationRow {
    key: string;
    name: string;
    node: GraphNode | null;
    draft: DraftRecord<EntityDraftValue> | null;
    recordId: string | null;
    unavailable: boolean;
  }

  let presentationRows = $derived.by(() => {
    const rows: PresentationRow[] = entities.map((node) => {
      const draft =
        retainedExistingDrafts.find(
          (candidate) => candidate.scope === entityScope(campaignId, kind, node.id),
        ) ?? null;
      const presentedNode = draft ? nodeWithDraftValue(node, draft.value) : node;
      return {
        key: `entity:${node.id}`,
        name: presentedNode.name.trim() || draft?.baseline.name.trim() || node.name,
        node: presentedNode,
        draft,
        recordId: node.id,
        unavailable: false,
      };
    });
    const backendIds = new Set(entities.map(({ id }) => id));

    for (const draft of retainedExistingDrafts) {
      const recordId = draft.scope.slice(existingDraftPrefix.length);
      if (backendIds.has(recordId)) continue;
      const provisional =
        activeDraftScope?.startsWith(newDraftPrefix) === true &&
        resolvedActiveDraftScope === draft.scope;
      if (!provisional && statusOf(draft) === 'saved') continue;
      rows.push({
        key: `${provisional ? 'provisional' : 'unavailable'}:${draft.scope}`,
        name: draft.value.name.trim() || draft.baseline.name.trim(),
        node: null,
        draft,
        recordId,
        unavailable: !provisional,
      });
    }

    for (const draft of retainedNewDrafts) {
      const hasPromotionIssue = draftCoordinator.getCreatePromotionIssue(draft.scope) !== undefined;
      if ((!hasPromotionIssue && statusOf(draft) === 'saved') || draft.scope === activeDraftScope) {
        continue;
      }
      rows.push({
        key: `new:${draft.scope}`,
        name:
          draft.value.name.trim() ||
          i18n.t('entityUi.newEntity', { kind: i18n.t(KIND_LABEL[kind]) }),
        node: null,
        draft,
        recordId: null,
        unavailable: false,
      });
    }

    return rows;
  });

  function emptyDraft(): EntityDraftValue {
    return {
      name: '',
      aliases: [],
      summary: '',
      notes: '',
      dateStart: '',
      dateEnd: '',
      isOngoing: false,
      sequenceIndex: '',
      era: '',
      durationLabel: '',
      sessionId: '',
      playerName: '',
      characterClass: '',
      characterLevel: '',
      status: '',
    };
  }

  function draftFromNode(node: GraphNode): EntityDraftValue {
    return {
      name: node.name,
      aliases: [...node.aliases],
      summary: node.summary ?? '',
      notes: node.notes ?? '',
      dateStart: node.date_start ?? '',
      dateEnd: node.date_end ?? '',
      isOngoing: node.is_ongoing ?? false,
      sequenceIndex: node.sequence_index?.toString() ?? '',
      era: node.era ?? '',
      durationLabel: node.duration_label ?? '',
      sessionId: node.session_id ?? '',
      playerName: node.player_name ?? '',
      characterClass: node.character_class ?? '',
      characterLevel: node.character_level?.toString() ?? '',
      status: node.status ?? '',
    };
  }

  function inputFromDraft(value: DraftSnapshot<EntityDraftValue>): EntityInput {
    return {
      name: value.name.trim(),
      aliases: [...value.aliases],
      summary: value.summary || null,
      notes: value.notes || null,
      dateStart: value.dateStart || null,
      dateEnd: value.dateEnd || null,
      isOngoing: value.isOngoing || null,
      sequenceIndex: value.sequenceIndex ? parseInt(value.sequenceIndex, 10) : null,
      era: value.era || null,
      durationLabel: value.durationLabel || null,
      sessionId: value.sessionId || null,
      playerName: value.playerName || null,
      characterClass: value.characterClass || null,
      characterLevel: value.characterLevel ? parseInt(value.characterLevel, 10) : null,
      status: value.status || null,
    };
  }

  function nodeWithDraftValue(node: GraphNode, value: DraftSnapshot<EntityDraftValue>): GraphNode {
    return {
      ...node,
      name: value.name,
      aliases: [...value.aliases],
      summary: value.summary || null,
      notes: value.notes || null,
      date_start: value.dateStart || null,
      date_end: value.dateEnd || null,
      is_ongoing: value.isOngoing || null,
      sequence_index: value.sequenceIndex ? parseInt(value.sequenceIndex, 10) : null,
      era: value.era || null,
      duration_label: value.durationLabel || null,
      session_id: value.sessionId || null,
      player_name: value.playerName || null,
      character_class: value.characterClass || null,
      character_level: value.characterLevel ? parseInt(value.characterLevel, 10) : null,
      status: (value.status || null) as GraphNode['status'],
    };
  }

  async function loadEntities(requestCampaignId = campaignId, requestKind = kind) {
    const requestScope = `${requestCampaignId}:${requestKind}`;
    loading = true;
    try {
      const loaded = await getEntities(requestCampaignId, requestKind);
      if (!mounted || `${campaignId}:${kind}` !== requestScope) return;
      for (const node of loaded) {
        draftCoordinator.open(
          entityScope(requestCampaignId, requestKind, node.id),
          `entity:${requestKind}:${node.id}`,
          draftFromNode(node),
          'authoritative-list',
        );
        leaseProjection(entityScope(requestCampaignId, requestKind, node.id));
      }
      const loadedScopes = new Set(
        loaded.map((node) => entityScope(requestCampaignId, requestKind, node.id)),
      );
      for (const draft of draftCoordinator.listByPrefix<EntityDraftValue>(
        `entity:${requestCampaignId}:${requestKind}:`,
      )) {
        if (!loadedScopes.has(draft.scope) && statusOf(draft) === 'saved') {
          releaseProjection(draft.scope);
        }
      }
      entities = loaded;
    } catch (e) {
      if (!mounted || `${campaignId}:${kind}` !== requestScope) return;
      showToastMsg((e as EntityError).message ?? i18n.t('entityUi.failedLoadEntities'));
    } finally {
      if (mounted && `${campaignId}:${kind}` === requestScope) loading = false;
    }
  }

  async function buildEntityMap() {
    const allKinds: EntityKind[] = [
      'npc',
      'location',
      'faction',
      'creature',
      'item',
      'event',
      'player_character',
      'misc',
    ];
    try {
      const results = await Promise.all(
        allKinds.map((k) => getEntities(campaignId, k).catch(() => [])),
      );
      entityMap.clear();
      buildWikiLinkEntityMap(results.flat()).forEach((target, key) => {
        entityMap.set(key, target);
      });
    } catch {
      // ignore — entity map is best-effort
    }
  }

  async function loadSessions() {
    try {
      sessions = await getSessions(campaignId);
    } catch {
      sessions = [];
    }
  }

  function oneRetainedNewDraft(
    requestCampaignId = campaignId,
    requestKind = kind,
  ): DraftRecord<EntityDraftValue> | null {
    const matches = draftCoordinator.listByPrefix<EntityDraftValue>(
      `entity-new:${requestCampaignId}:${requestKind}:`,
    );
    if (matches.length > 1) {
      throw new Error(`Multiple retained new drafts exist for ${requestCampaignId}:${requestKind}`);
    }
    return matches[0] ?? null;
  }

  function showNewDraft(draft: DraftRecord<EntityDraftValue>): void {
    formNode = null;
    pendingInitialName = null;
    pendingSourceFindingId = null;
    activeDraftScope = draft.scope;
    activeRecordId = null;
    activeTargetUnavailable = false;
    showForm = true;
    buildEntityMap();
    if (kind === 'event') loadSessions();
  }

  function openCreate(): void {
    const retained = oneRetainedNewDraft();
    if (retained) {
      showNewDraft(retained);
      return;
    }
    const scope = newEntityScope(campaignId, kind, allocateNewDraftId());
    const draft = draftCoordinator.open(scope, scope, emptyDraft());
    leaseProjection(scope);
    showNewDraft(draft);
  }

  function startPendingCreate(request: PendingCreate): void {
    const retained = oneRetainedNewDraft(campaignId, request.kind);
    const scope = retained?.scope ?? newEntityScope(campaignId, request.kind, allocateNewDraftId());
    const draft = retained ?? draftCoordinator.open(scope, scope, emptyDraft());
    leaseProjection(scope);
    if (draft.value.name === '' && request.name !== '') {
      draftCoordinator.revise(scope, { ...emptyDraft(), name: request.name });
    }
    formNode = null;
    pendingInitialName = request.name;
    pendingSourceFindingId = request.sourceFindingId ?? null;
    activeDraftScope = scope;
    activeRecordId = null;
    activeTargetUnavailable = false;
    showForm = true;
    buildEntityMap();
    if (request.kind === 'event') loadSessions();
  }

  function openPendingCreate(request: PendingCreate): void {
    const retained = oneRetainedNewDraft(campaignId, request.kind);
    if (retained && statusOf(retained) !== 'saved') {
      blockedPendingCreate = request;
      return;
    }
    startPendingCreate(request);
  }

  // The `c` shortcut bumps createNonce; open the create form.
  $effect(() => {
    if (createNonce > 0) untrack(openCreate);
  });

  // Deep-link: when asked to open a specific entity, open its edit form once
  // it's present in the loaded list. Immediately invokes onOpenIdConsumed so
  // the caller can clear openId — preventing the effect from re-firing when the
  // entities array is mutated by a subsequent save or delete.
  $effect(() => {
    if (!openId) {
      consumedOpenKey = null;
      return;
    }
    const openKey = `${campaignId}:${kind}:${openId}`;
    if (consumedOpenKey === openKey) return;
    const node = entities.find((n) => n.id === openId);
    if (node) {
      consumedOpenKey = openKey;
      untrack(() => openEdit(node));
      onOpenIdConsumed?.();
    }
  });

  function openEdit(node: GraphNode) {
    const scope = entityScope(campaignId, kind, node.id);
    const draft =
      draftCoordinator.get<EntityDraftValue>(scope) ??
      draftCoordinator.open(scope, `entity:${kind}:${node.id}`, draftFromNode(node));
    leaseProjection(scope);
    formNode = nodeWithDraftValue(node, draft.value);
    pendingInitialName = null;
    pendingSourceFindingId = null;
    activeDraftScope = scope;
    activeRecordId = node.id;
    activeTargetUnavailable = false;
    showForm = true;
    buildEntityMap();
    if (kind === 'event') loadSessions();
  }

  function openPresentationRow(row: PresentationRow): void {
    if (row.node) {
      openEdit(row.node);
      return;
    }
    if (!row.draft) return;
    if (row.recordId === null) {
      showNewDraft(row.draft);
      return;
    }

    formNode = null;
    pendingInitialName = null;
    pendingSourceFindingId = null;
    activeDraftScope = row.draft.scope;
    activeRecordId = row.recordId;
    activeTargetUnavailable = row.unavailable;
    showForm = true;
    buildEntityMap();
    if (kind === 'event') loadSessions();
  }

  function rowStatus(draft: DraftRecord<EntityDraftValue> | null): DraftStatus | null {
    if (!draft || draft.scope === resolvedActiveDraftScope) return null;
    const status = statusOf(draft);
    return status === 'saved' ? null : status;
  }

  function statusLabel(status: DraftStatus): string {
    if (status === 'pending') return i18n.t('drafts.unsavedChanges');
    if (status === 'saving') return i18n.t('drafts.saving');
    if (status === 'failed') return i18n.t('drafts.couldNotSave');
    return i18n.t('drafts.saved');
  }

  type SuccessfulCreatePromotion = Extract<
    CreatePromotionResult<EntityDraftValue>,
    { outcome: 'promoted' }
  >;

  function finishCreatePromotion(
    sourceScope: string,
    destinationScope: string,
    promotion: SuccessfulCreatePromotion,
    canonical: GraphNode | null = null,
  ): void {
    if (activeDraftScope !== sourceScope) return;
    const recordId = destinationScope.slice(existingDraftPrefix.length);
    const base = canonical ?? entities.find((entity) => entity.id === recordId) ?? null;
    activeDraftScope = destinationScope;
    activeRecordId = recordId;
    formNode = base ? nodeWithDraftValue(base, promotion.draft.value) : null;
    activeTargetUnavailable = false;
    pendingInitialName = null;
    pendingSourceFindingId = null;
  }

  function retryCreatePromotion(): void {
    if (!activeDraftScope) return;
    const sourceScope = activeDraftScope;
    const issue = draftCoordinator.getCreatePromotionIssue(sourceScope);
    if (!issue) return;
    const result = draftCoordinator.retryCreatePromotion<EntityDraftValue>(sourceScope);
    if (result.outcome === 'promoted') {
      finishCreatePromotion(sourceScope, issue.destinationScope, result);
      void focusStableEditorAction();
    }
  }

  function resolveCreatePromotion(choice: 'keep-destination' | 'keep-source'): void {
    if (!activeDraftScope) return;
    const sourceScope = activeDraftScope;
    const issue = draftCoordinator.getCreatePromotionIssue(sourceScope);
    if (!issue) return;
    const result = draftCoordinator.resolveCreatePromotion<EntityDraftValue>(sourceScope, choice);
    if (result.outcome === 'promoted') {
      finishCreatePromotion(sourceScope, issue.destinationScope, result);
      void focusStableEditorAction();
    }
  }

  async function focusStableEditorAction(): Promise<void> {
    await tick();
    const save = formPanel?.querySelector<HTMLButtonElement>('[data-testid="entity-form-submit"]');
    if (save && !save.disabled) {
      save.focus();
      return;
    }
    formPanel
      ?.querySelector<HTMLElement>('input:not([disabled]), textarea:not([disabled])')
      ?.focus();
  }

  async function handleSave(_input: EntityInput) {
    if (!activeDraftScope) return;
    const saveCampaignId = campaignId;
    const saveKind = kind;
    const saveScope = draftCoordinator.resolveScope(activeDraftScope);
    if (deletingScope === saveScope) return;
    const saveDraft = draftCoordinator.get<EntityDraftValue>(saveScope);
    if (!saveDraft) return;
    const recordPrefix = `entity:${saveCampaignId}:${saveKind}:`;
    const recordId = saveScope.startsWith(recordPrefix)
      ? saveScope.slice(recordPrefix.length)
      : activeRecordId;
    if (recordId === null && saveDraft.inFlight) return;
    const sourceFindingId = pendingSourceFindingId;
    let acknowledgedNode: GraphNode | null = null;
    let adapterError: EntityError | null = null;
    if (formError?.scope === saveScope) formError = null;

    await draftCoordinator.requestSave<EntityDraftValue>(saveScope, async (value) => {
      try {
        if (recordId) {
          const updated = await updateEntity(recordId, saveKind, inputFromDraft(value));
          if (updated.id !== recordId) {
            throw new Error(`Entity save acknowledged the wrong target: ${updated.id}`);
          }
          acknowledgedNode = updated;
        } else {
          acknowledgedNode = await createEntity(saveCampaignId, saveKind, inputFromDraft(value));
        }
        return draftFromNode(acknowledgedNode);
      } catch (error) {
        adapterError = error as EntityError;
        throw error;
      }
    });

    const savedDraft = draftCoordinator.get<EntityDraftValue>(saveScope);
    if (savedDraft?.error) {
      const error: EntityError = adapterError ?? {
        code: 'DATABASE',
        message: savedDraft.error,
      };
      if (error.code === 'VALIDATION') formError = { scope: saveScope, error };
      if (error.code === 'NOT_FOUND') {
        if (resolvedActiveDraftScope === saveScope) activeTargetUnavailable = true;
        await loadEntities();
      }
      return;
    }

    if (acknowledgedNode) {
      const canonical = acknowledgedNode as GraphNode;
      const stillInContext = mounted && campaignId === saveCampaignId && kind === saveKind;
      if (recordId) {
        if (stillInContext) {
          entities = [canonical, ...entities.filter((entity) => entity.id !== canonical.id)];
          if (resolvedActiveDraftScope === saveScope) {
            formNode = canonical;
            activeTargetUnavailable = false;
          }
        }
      } else {
        if (sourceFindingId) {
          onPendingCreateSaved?.(sourceFindingId);
        }
        const mappedScope = entityScope(saveCampaignId, saveKind, canonical.id);
        const promotion = draftCoordinator.tryRemapAfterCreate<EntityDraftValue>(
          saveScope,
          mappedScope,
          `entity:${saveKind}:${canonical.id}`,
        );
        if (stillInContext) {
          if (promotion.outcome === 'promoted') {
            const base = entities.find((entity) => entity.id === canonical.id) ?? canonical;
            const promotedNode = nodeWithDraftValue(base, promotion.draft.value);
            entities = [promotedNode, ...entities.filter((entity) => entity.id !== canonical.id)];
            if (activeDraftScope === saveScope) {
              finishCreatePromotion(saveScope, mappedScope, promotion, promotedNode);
            } else if (resolvedActiveDraftScope === mappedScope) {
              activeRecordId = canonical.id;
              formNode = promotedNode;
              activeTargetUnavailable = false;
            }
          } else if (!entities.some((entity) => entity.id === canonical.id)) {
            entities = [canonical, ...entities];
          }
        }
      }
    }
  }

  async function retryActiveSave(event: MouseEvent): Promise<void> {
    if (!currentDraft || activeDeletionPending) return;
    const shouldRestoreFocus = event.currentTarget === document.activeElement;
    if (shouldRestoreFocus) void focusStableEditorAction();
    await handleSave(inputFromDraft(currentDraft.value));
    if (!shouldRestoreFocus) return;
    await tick();
    const retry = formPanel?.querySelector<HTMLButtonElement>(
      '.draft-status [role="alert"] button',
    );
    if (retry) retry.focus();
    else await focusStableEditorAction();
  }

  function reviseActiveDraft(value: EntityDraftValue): void {
    if (!activeDraftScope) return;
    const scope = draftCoordinator.resolveScope(activeDraftScope);
    if (deletingScope === scope) return;
    const previous = draftCoordinator.get<EntityDraftValue>(scope);
    const revised = draftCoordinator.revise(scope, value);
    if (revised !== previous && formError?.scope === scope) formError = null;
  }

  function closeForm(): void {
    showForm = false;
    formNode = null;
    activeDraftScope = null;
    activeRecordId = null;
    activeTargetUnavailable = false;
    pendingInitialName = null;
    pendingSourceFindingId = null;
  }

  function requestCancel(): void {
    if (activeDeletionPending) return;
    if (!activeDraftScope || !currentDraft || statusOf(currentDraft) === 'saved') {
      closeForm();
      return;
    }
    discardConfirm = true;
  }

  function discardActiveDraft(): void {
    if (!activeDraftScope) return;
    const scope = activeDraftScope;
    if (draftCoordinator.discard(scope) !== 'discarded') return;
    discardConfirm = false;
    closeForm();
  }

  function requestDelete(node: GraphNode): void {
    const requestCampaignId = campaignId;
    const requestKind = kind;
    const scope = entityScope(requestCampaignId, requestKind, node.id);
    if (draftCoordinator.get<EntityDraftValue>(scope)?.inFlight) return;
    deleteConfirm = Object.freeze({
      campaignId: requestCampaignId,
      kind: requestKind,
      entityId: node.id,
      scope,
      name: node.name,
      rowIndex: presentationRows.findIndex((row) => row.recordId === node.id),
    });
  }

  function suppressNavigationWhileModal(event: KeyboardEvent): void {
    if (anyModalOpen) event.stopPropagation();
  }

  async function focusAfterDelete(intent: EntityDeletionIntent): Promise<void> {
    await tick();
    if (!managerElement?.isConnected) return;
    const rows = Array.from(managerElement.querySelectorAll<HTMLButtonElement>('.entity-name'));
    const sameContext = campaignId === intent.campaignId && kind === intent.kind;
    const row = sameContext
      ? rows[Math.min(Math.max(intent.rowIndex, 0), rows.length - 1)]
      : rows[0];
    const fallback = managerElement.querySelector<HTMLButtonElement>('[data-testid="entity-new"]');
    (row ?? fallback)?.focus();
  }

  async function confirmDelete(intent: EntityDeletionIntent) {
    if (deletingScope !== null) return;
    if (draftCoordinator.get<EntityDraftValue>(intent.scope)?.inFlight) return;
    deletingScope = intent.scope;
    await tick();
    deleteProgress?.focus();
    let deleted = false;
    try {
      await softDeleteEntity(intent.entityId, intent.kind);
      const cleanup = draftCoordinator.removeAfterDelete(intent.scope);
      if (cleanup !== 'removed') {
        showToastMsg(i18n.t('entityUi.failedDeleteEntity'));
        return;
      }
      if (campaignId === intent.campaignId && kind === intent.kind) {
        entities = entities.filter((entity) => entity.id !== intent.entityId);
        if (resolvedActiveDraftScope === intent.scope) closeForm();
      }
      deleted = true;
    } catch (e) {
      showToastMsg((e as EntityError).message ?? i18n.t('entityUi.failedDeleteEntity'));
    } finally {
      if (deleteConfirm?.scope === intent.scope) deleteConfirm = null;
      deletingScope = null;
    }
    if (deleted) await focusAfterDelete(intent);
  }

  async function handleRecompile() {
    if (!formNode || recompiling || activeDeletionPending) return;
    recompiling = true;
    try {
      const ok = await compileEntity(kind, formNode.id);
      if (!ok) {
        showToastMsg(i18n.t('entityUi.noSourceContext'));
        return;
      }
      const refreshed = await getEntity(formNode.id, kind);
      formNode = refreshed;
      entities = entities.map((e) => (e.id === refreshed.id ? refreshed : e));
    } catch (e) {
      showToastMsg((e as EntityError).message ?? i18n.t('entityUi.failedRecompileArticle'));
    } finally {
      recompiling = false;
    }
  }

  function showToastMsg(msg: string) {
    toast = msg;
    setTimeout(() => {
      toast = null;
    }, 4000);
  }

  // Reset form and reload when kind or campaign changes
  $effect(() => {
    const scope = `${campaignId}:${kind}`;
    if (scope === loadedScope) return;
    untrack(() => {
      if (activeExistingProjectionPrefix) {
        releaseProjectionPrefix(activeExistingProjectionPrefix);
      }
      if (activeNewProjectionPrefix) releaseProjectionPrefix(activeNewProjectionPrefix);
      for (const draft of draftCoordinator.listByPrefix<EntityDraftValue>(
        `entity:${campaignId}:${kind}:`,
      )) {
        leaseProjection(draft.scope);
      }
      for (const draft of draftCoordinator.listByPrefix<EntityDraftValue>(
        `entity-new:${campaignId}:${kind}:`,
      )) {
        leaseProjection(draft.scope);
      }
    });
    activeExistingProjectionPrefix = `entity:${campaignId}:${kind}:`;
    activeNewProjectionPrefix = `entity-new:${campaignId}:${kind}:`;
    loadedScope = scope;
    showForm = false;
    formNode = null;
    activeDraftScope = null;
    activeRecordId = null;
    activeTargetUnavailable = false;
    pendingInitialName = null;
    pendingSourceFindingId = null;
    blockedPendingCreate = null;
    discardConfirm = false;
    if (campaignId) void loadEntities(campaignId, kind);
    if (
      !openId &&
      (!pendingCreate || pendingCreate.kind !== kind) &&
      untrack(() => {
        const retained = oneRetainedNewDraft(campaignId, kind);
        return (
          retained !== null &&
          (statusOf(retained) !== 'saved' ||
            draftCoordinator.getCreatePromotionIssue(retained.scope) !== undefined)
        );
      })
    ) {
      untrack(openCreate);
    }
  });

  $effect(() => {
    if (!pendingCreate || pendingCreate.kind !== kind || pendingCreate === consumedPendingCreate) {
      return;
    }
    consumedPendingCreate = pendingCreate;
    untrack(() => openPendingCreate(pendingCreate as PendingCreate));
    onPendingCreateConsumed?.();
  });

  $effect(() => {
    const sourceScope = activeDraftScope;
    const resolvedScope = resolvedActiveDraftScope;
    const recordPrefix = existingDraftPrefix;
    if (!sourceScope || !resolvedScope || resolvedScope === sourceScope) return;
    if (!resolvedScope.startsWith(recordPrefix)) return;
    const recordId = resolvedScope.slice(recordPrefix.length);
    if (!recordId) return;

    untrack(() => {
      if (activeDraftScope !== sourceScope) return;
      activeRecordId = recordId;
      activeTargetUnavailable = false;
      pendingInitialName = null;
      pendingSourceFindingId = null;
    });
  });
</script>

<svelte:document onkeydown={suppressNavigationWhileModal} />

<div class="entity-manager" bind:this={managerElement}>
  <div class="content">
    <!-- List panel -->
    <div class="list-panel">
      <div class="list-header">
        <Button testId="entity-new" onclick={openCreate}
          >{i18n.t('entityUi.newEntity', { kind: i18n.t(KIND_LABEL[kind]) })}</Button
        >
      </div>

      {#if loading}
        <p class="muted">{i18n.t('entityUi.loading')}</p>
      {:else if presentationRows.length === 0}
        <p class="muted">
          {i18n.t('entityUi.noEntities', { kind: i18n.t(KIND_LABEL[kind]).toLowerCase() })}
        </p>
      {:else}
        <ul class="entity-list">
          {#each presentationRows as row (row.key)}
            {@const status = rowStatus(row.draft)}
            {@const statusId = `entity-draft-status-${row.key}`}
            {@const showUnavailable =
              row.unavailable && row.draft?.scope !== resolvedActiveDraftScope}
            {@const unavailableId = `entity-draft-unavailable-${row.key}`}
            <li
              class="entity-row"
              class:selected={resolvedActiveDraftScope === row.draft?.scope ||
                activeRecordId === row.recordId}
            >
              <button
                type="button"
                class="entity-name"
                aria-describedby={[
                  status ? statusId : undefined,
                  showUnavailable ? unavailableId : undefined,
                ]
                  .filter(Boolean)
                  .join(' ') || undefined}
                onclick={() => openPresentationRow(row)}>{row.name}</button
              >
              {#if status}
                <span id={statusId} class={['row-status', status]}>{statusLabel(status)}</span>
              {/if}
              {#if showUnavailable}
                <span id={unavailableId} class="row-unavailable">
                  {i18n.t('drafts.targetUnavailable')}
                </span>
              {/if}
              {#if row.node && onViewGraph}
                <Button
                  variant="ghost"
                  class="btn-icon entity-graph-btn"
                  title={i18n.t('entityUi.viewRelationships')}
                  onclick={() => onViewGraph(row.node as GraphNode)}
                  >{i18n.t('entityUi.graph')}</Button
                >
              {/if}
              {#if row.node}
                {@const deleteScope = entityScope(campaignId, kind, row.node.id)}
                {@const deleteBlocked = Boolean(
                  draftCoordinator.get<EntityDraftValue>(deleteScope)?.inFlight,
                )}
                {@const deleteHelpId = `entity-delete-help-${campaignId}-${kind}-${row.node.id}`}
                {#if deleteBlocked}
                  <span id={deleteHelpId} class="visually-hidden">
                    {i18n.t('drafts.waitForSavingBeforeDiscard')}
                  </span>
                {/if}
                <Button
                  variant="ghost"
                  iconOnly
                  class="btn-icon delete"
                  ariaLabel={i18n.t('entityUi.deleteEntity', { name: row.name })}
                  ariaDescribedby={deleteBlocked ? deleteHelpId : undefined}
                  title={deleteBlocked ? i18n.t('drafts.waitForSavingBeforeDiscard') : undefined}
                  disabled={deleteBlocked || deletingScope === deleteScope}
                  onclick={() => requestDelete(row.node as GraphNode)}>×</Button
                >
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <!-- Form panel -->
    {#if showForm}
      <div class="form-panel" bind:this={formPanel}>
        {#if presentedFormNode?.notes}
          <div class="notes-preview">
            <WikiText text={presentedFormNode.notes} entities={entityMap} {onMissingLinkClick} />
          </div>
        {/if}
        {#if presentedFormNode}
          <div class="codex-section">
            <div class="codex-header">
              <h3>{i18n.t('entityUi.codexArticle')}</h3>
              {#if presentedFormNode.codex_stale !== false}
                <span class="chip-stale">{i18n.t('entityUi.stale')}</span>
              {/if}
              <Button
                variant="ghost"
                class="btn-recompile"
                disabled={recompiling || activeDeletionPending}
                loading={recompiling}
                loadingText={i18n.t('status.processing')}
                onclick={handleRecompile}
              >
                {i18n.t('entityUi.recompileArticle')}
              </Button>
            </div>
            <div class="codex-article">
              {#if presentedFormNode.codex_article}
                <WikiText
                  text={presentedFormNode.codex_article}
                  entities={entityMap}
                  onEntityClick={onOpenEntity}
                  {onMissingLinkClick}
                />
              {:else}
                <p class="muted">{i18n.t('entityUi.noArticle')}</p>
              {/if}
            </div>
          </div>
        {/if}
        <fieldset class="entity-controls" disabled={activeDeletionPending}>
          <EntityForm
            {kind}
            node={presentedFormNode}
            error={formError?.scope === resolvedActiveDraftScope ? formError.error : null}
            editingScope={resolvedActiveDraftScope ?? ''}
            initialName={pendingInitialName ?? undefined}
            draftValue={currentDraft?.value}
            onvaluechange={reviseActiveDraft}
            existing={activeRecordId !== null}
            submitDisabled={creating || activeDeletionPending}
            sessions={kind === 'event' ? sessions : []}
            {entityMap}
            onsave={handleSave}
            oncancel={requestCancel}
            {onOpenEntity}
          />
          {#if currentDraft}
            <div class="draft-status">
              <SaveStatus
                status={statusOf(currentDraft)}
                error={activeTargetUnavailable
                  ? i18n.t('drafts.targetUnavailable')
                  : currentDraft.error}
                onRetry={retryActiveSave}
              />
            </div>
          {/if}
        </fieldset>
        {#if activeCreatePromotionIssue}
          <div class="promotion-alert" role="alert">
            <div>
              <strong>{i18n.t('drafts.createdNeedsAttention')}</strong>
              <p>
                {i18n.t(
                  activeCreatePromotionIssue.reason === 'destination-conflict' ||
                    activeCreatePromotionIssue.reason === 'destination-newer-acknowledgment'
                    ? 'drafts.createdEntityConflict'
                    : 'drafts.finishCreatedEntity',
                )}
              </p>
            </div>
            {#if activeCreatePromotionIssue.reason === 'destination-conflict' || activeCreatePromotionIssue.reason === 'destination-newer-acknowledgment'}
              <div class="promotion-actions">
                <Button variant="ghost" onclick={() => resolveCreatePromotion('keep-destination')}>
                  {i18n.t('drafts.keepSavedRecord')}
                </Button>
                <Button variant="secondary" onclick={() => resolveCreatePromotion('keep-source')}>
                  {i18n.t('drafts.keepMyDraft')}
                </Button>
              </div>
            {:else}
              <Button variant="ghost" onclick={retryCreatePromotion}>
                {i18n.t('drafts.retry')}
              </Button>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Delete confirmation -->
  {#if deleteConfirm}
    {#snippet deleteBody()}
      <p>{i18n.t('entityUi.removeEntity', { name: deleteConfirm?.name ?? '' })}</p>
      {#if deletingScope === deleteConfirm?.scope}
        <button
          type="button"
          class="delete-progress"
          aria-disabled="true"
          bind:this={deleteProgress}>{i18n.t('status.processing')}</button
        >
      {/if}
    {/snippet}
    {#snippet deleteActions()}
      <Button
        variant="danger"
        disabled={deletingScope !== null}
        loading={deletingScope === deleteConfirm?.scope}
        loadingText={i18n.t('common.delete')}
        onclick={() => confirmDelete(deleteConfirm as EntityDeletionIntent)}
        >{i18n.t('common.delete')}</Button
      >
      <Button
        variant="ghost"
        disabled={deletingScope !== null}
        onclick={() => {
          deleteConfirm = null;
        }}>{i18n.t('common.cancel')}</Button
      >
    {/snippet}
    <Dialog
      title={i18n.t('common.delete')}
      body={deleteBody}
      actions={deleteActions}
      onclose={() => {
        if (deletingScope === null) deleteConfirm = null;
      }}
    />
  {/if}

  {#if blockedPendingCreate}
    {@const retainedBlockedDraft = oneRetainedNewDraft(campaignId, blockedPendingCreate.kind)}
    {#snippet pendingCreateBody()}
      <p>{i18n.t('entityUi.creatingReplacesForm', { name: blockedPendingCreate?.name ?? '' })}</p>
      {#if retainedBlockedDraft && !draftCoordinator.canDiscard(retainedBlockedDraft.scope)}
        <p id="entity-new-discard-blocked" aria-live="polite">
          {i18n.t('drafts.waitForSavingBeforeDiscard')}
        </p>
      {/if}
    {/snippet}
    {#snippet pendingCreateActions()}
      <Button
        variant="danger"
        disabled={retainedBlockedDraft
          ? !draftCoordinator.canDiscard(retainedBlockedDraft.scope)
          : false}
        title={retainedBlockedDraft && !draftCoordinator.canDiscard(retainedBlockedDraft.scope)
          ? i18n.t('drafts.waitForSavingBeforeDiscard')
          : undefined}
        ariaDescribedby={retainedBlockedDraft &&
        !draftCoordinator.canDiscard(retainedBlockedDraft.scope)
          ? 'entity-new-discard-blocked'
          : undefined}
        onclick={() => {
          const request = blockedPendingCreate;
          const retained = request ? oneRetainedNewDraft(campaignId, request.kind) : null;
          if (retained && draftCoordinator.discard(retained.scope) !== 'discarded') {
            return;
          }
          blockedPendingCreate = null;
          closeForm();
          if (request) startPendingCreate(request);
        }}>{i18n.t('entityUi.discardAndCreate')}</Button
      >
      <Button
        variant="ghost"
        initialFocus
        onclick={() => {
          const request = blockedPendingCreate;
          const retained = request ? oneRetainedNewDraft(campaignId, request.kind) : null;
          blockedPendingCreate = null;
          if (retained) showNewDraft(retained);
        }}>{i18n.t('entityUi.keepEditing')}</Button
      >
    {/snippet}
    <Dialog
      title={i18n.t('entityUi.discardUnsaved')}
      body={pendingCreateBody}
      actions={pendingCreateActions}
      onclose={() => {
        blockedPendingCreate = null;
      }}
    />
  {/if}

  {#if discardConfirm && currentDraft}
    {#snippet discardBody()}
      <p>{i18n.t('drafts.retainedThisSession')}</p>
      {#if !draftCoordinator.canDiscard(currentDraft.scope)}
        <p id="entity-discard-blocked" aria-live="polite">
          {i18n.t('drafts.waitForSavingBeforeDiscard')}
        </p>
      {/if}
    {/snippet}
    {#snippet discardActions()}
      <button
        type="button"
        class="dialog-button danger"
        disabled={!draftCoordinator.canDiscard(currentDraft.scope)}
        aria-describedby={!draftCoordinator.canDiscard(currentDraft.scope)
          ? 'entity-discard-blocked'
          : undefined}
        onclick={discardActiveDraft}>{i18n.t('drafts.discardChanges')}</button
      >
      <button
        type="button"
        class="dialog-button ghost"
        data-autofocus
        onclick={() => (discardConfirm = false)}>{i18n.t('common.cancel')}</button
      >
    {/snippet}
    <Dialog
      title={i18n.t('drafts.unsavedChanges')}
      body={discardBody}
      actions={discardActions}
      onclose={() => (discardConfirm = false)}
    />
  {/if}

  <!-- Toast -->
  {#if toast}
    <div class="toast" role="alert">{toast}</div>
  {/if}
</div>

<style>
  .entity-manager {
    display: flex;
    flex-direction: column;
    gap: 0;
    min-height: 60vh;
  }
  .content {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .list-panel {
    flex: 0 0 260px;
    border-right: 1px solid var(--line);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .list-header {
    padding: 10px;
    border-bottom: 1px solid var(--line);
  }
  .entity-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .entity-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
    border-bottom: 1px solid var(--line);
  }
  .entity-row.selected {
    background: var(--bg-panel-2);
  }
  :global(.entity-name) {
    flex: 1;
    background: none;
    border: none;
    color: var(--fg-1);
    text-align: left;
    padding: 10px 4px;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .row-status,
  .row-unavailable {
    flex: 0 1 auto;
    color: var(--fg-3);
    font: 600 0.7rem/1.25 var(--font-sans);
  }
  .row-status.pending,
  .row-status.saving {
    color: var(--warning);
  }
  .row-status.failed,
  .row-unavailable {
    color: var(--danger);
  }
  .row-unavailable {
    max-width: 9rem;
  }
  :global(.btn-icon) {
    background: none;
    border: none;
    color: var(--fg-4);
    cursor: pointer;
    font-size: 1rem;
  }
  :global(.btn-icon.delete:hover) {
    color: var(--danger);
  }
  .form-panel {
    flex: 1;
    padding: 16px;
    overflow-y: auto;
  }
  .entity-controls {
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
  .draft-status {
    margin-top: var(--s-3);
  }
  .promotion-alert {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    margin-top: var(--s-3);
    padding: var(--s-3);
    border: 1px solid color-mix(in srgb, var(--danger) 55%, transparent);
    border-radius: var(--r-sm);
    background: var(--danger-bg);
    color: var(--danger);
  }
  .promotion-alert p {
    margin: var(--s-1) 0 0;
  }
  .promotion-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .dialog-button {
    min-height: 36px;
    padding: var(--s-2) var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--fg-2);
    font: 600 0.875rem/1.2 var(--font-sans);
    cursor: pointer;
  }
  .dialog-button.danger {
    border-color: color-mix(in srgb, var(--danger) 55%, transparent);
    background: var(--danger-bg);
    color: var(--danger);
  }
  .dialog-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }
  .delete-progress {
    margin-top: var(--s-3);
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--fg-2);
    font: inherit;
    cursor: progress;
  }
  .delete-progress:focus-visible {
    outline: 2px solid var(--arcane-300);
    outline-offset: 3px;
  }
  .notes-preview {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 12px;
    margin-bottom: 12px;
    font-size: 0.9rem;
    color: var(--fg-2);
    line-height: 1.5;
  }
  .muted {
    color: var(--fg-3);
    font-size: 0.85rem;
    padding: 16px;
  }
  .codex-section {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 12px;
    margin-bottom: 12px;
  }
  .codex-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }
  .codex-header h3 {
    margin: 0;
    font-size: 0.9rem;
    color: var(--fg-1);
    flex: 1;
  }
  .chip-stale {
    background: var(--danger);
    color: var(--bg-abyss);
    border-radius: 10px;
    padding: 2px 8px;
    font-size: 0.75rem;
    font-weight: 600;
  }
  :global(.btn-recompile) {
    font-size: 0.8rem;
    padding: 4px 10px;
  }
  .codex-article {
    white-space: pre-wrap;
    font-size: 0.9rem;
    color: var(--fg-2);
    line-height: 1.5;
  }
  .toast {
    position: fixed;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-panel-2);
    color: var(--fg-1);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 10px 20px;
    z-index: 200;
    font-size: 0.9rem;
  }
</style>
