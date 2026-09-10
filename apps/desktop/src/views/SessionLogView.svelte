<script module lang="ts">
  let fallbackDraftId = 0;

  function allocateNewDraftId(): string {
    fallbackDraftId += 1;
    return globalThis.crypto?.randomUUID?.() ?? `draft-${fallbackDraftId}`;
  }

  interface ActiveSessionLoad {
    request: number;
    campaignId: string;
    acknowledgedCreates: Set<string>;
  }

  export class SessionLoadFence {
    private active: ActiveSessionLoad | undefined;

    get trackedCount() {
      return this.active?.acknowledgedCreates.size ?? 0;
    }

    begin(request: number, campaignId: string) {
      this.active = { request, campaignId, acknowledgedCreates: new Set() };
    }

    acknowledge(campaignId: string, scope: string) {
      if (this.active?.campaignId === campaignId) this.active.acknowledgedCreates.add(scope);
    }

    forget(scope: string) {
      this.active?.acknowledgedCreates.delete(scope);
    }

    settle(request: number, campaignId: string) {
      if (this.active?.request !== request || this.active.campaignId !== campaignId) {
        return new Set<string>();
      }
      const acknowledgedCreates = this.active.acknowledgedCreates;
      this.active = undefined;
      return acknowledgedCreates;
    }
  }
</script>

<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import {
    getSessions,
    createSession,
    getEntities,
    type Session,
    type EntityKind,
  } from '../lib/commands';
  import SessionList from '../components/SessionList.svelte';
  import {
    sessionDraftValue,
    sessionWithDraft,
    type SessionDraftValue,
  } from '../components/SessionRow.svelte';
  import Button from '../components/ui/Button.svelte';
  import { i18n } from '../lib/locale.svelte';
  import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
  import {
    newSessionScope,
    sessionScope,
    statusOf,
    type DraftRecord,
  } from '../lib/drafts/draft-state';

  interface Props {
    campaignId: string;
    draftCoordinator?: DraftCoordinator;
  }

  let { campaignId, draftCoordinator = new DraftCoordinator() }: Props = $props();

  const ALL_KINDS: EntityKind[] = [
    'npc',
    'location',
    'faction',
    'creature',
    'item',
    'event',
    'player_character',
    'misc',
  ];

  let backendSessions = $state<Session[]>([]);
  let loading = $state(true);
  let entityMap = new SvelteMap<string, { id: string; kind: string }>();
  let mounted = true;
  let sessionRequest = 0;
  let entityRequest = 0;
  let activeProjectionPrefix: string | null = null;
  let sessionLogElement = $state<HTMLDivElement>();
  let retryingCreateScope = $state<string | null>(null);
  const projectionLeases = new SvelteMap<string, () => void>();
  const sessionLoadFence = new SessionLoadFence();

  const EMPTY_SESSION_CREATE: SessionDraftValue = Object.freeze({
    sessionNumber: 0,
    title: '',
    datePlayed: '',
    notes: '',
  });

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

  let sessionPrefix = $derived(`session:${campaignId}:`);
  let newSessionPrefix = $derived(`session-new:${campaignId}:`);
  let retainedCreate = $derived.by(
    () =>
      draftCoordinator.listByPrefix<SessionDraftValue>(newSessionPrefix).find((draft) => {
        return (
          statusOf(draft) !== 'saved' ||
          draftCoordinator.getCreatePromotionIssue(draft.scope) !== undefined
        );
      }) ?? null,
  );
  let createPromotionIssue = $derived(
    retainedCreate ? draftCoordinator.getCreatePromotionIssue(retainedCreate.scope) : undefined,
  );
  let showCreateRecovery = $derived(
    createPromotionIssue === undefined &&
      (retainedCreate?.error != null || retainedCreate?.scope === retryingCreateScope),
  );
  let unavailableSessionIds = $derived(
    new Set(
      draftCoordinator
        .listByPrefix<SessionDraftValue>(sessionPrefix)
        .filter(
          (draft) =>
            statusOf(draft) !== 'saved' &&
            !backendSessions.some(
              (session) => sessionScope(campaignId, session.id) === draft.scope,
            ),
        )
        .map((draft) => draft.scope.slice(sessionPrefix.length)),
    ),
  );
  let sessions = $derived.by(() => {
    const retained = draftCoordinator.listByPrefix<SessionDraftValue>(sessionPrefix);
    const byScope = new Map(retained.map((draft) => [draft.scope, draft]));
    const presented = backendSessions.map((session) => {
      const draft = byScope.get(sessionScope(campaignId, session.id));
      return draft ? sessionWithDraft(session, draft.value) : session;
    });
    const backendIds = new Set(backendSessions.map(({ id }) => id));

    for (const draft of retained) {
      const id = draft.scope.slice(sessionPrefix.length);
      if (backendIds.has(id) || statusOf(draft) === 'saved') continue;
      presented.push({
        id,
        campaign_id: campaignId,
        session_number: draft.value.sessionNumber,
        title: draft.value.title,
        date_played: draft.value.datePlayed,
        notes: draft.value.notes,
        created_at: null,
        updated_at: null,
      });
    }

    return presented;
  });

  $effect(() => {
    const requestedCampaign = campaignId;
    const request = ++sessionRequest;
    const prefix = `session:${requestedCampaign}:`;
    if (activeProjectionPrefix && activeProjectionPrefix !== prefix) {
      const previousPrefix = activeProjectionPrefix;
      untrack(() => releaseProjectionPrefix(previousPrefix));
    }
    activeProjectionPrefix = prefix;
    sessionLoadFence.begin(request, requestedCampaign);
    const acknowledgmentsAtStart = untrack(() => {
      const retained = draftCoordinator.listByPrefix<SessionDraftValue>(prefix);
      for (const draft of retained) leaseProjection(draft.scope);
      return new Map(retained.map((draft) => [draft.scope, draft.lastAcknowledgedAttemptId]));
    });
    loading = true;

    getSessions(requestedCampaign).then(
      (loaded) => {
        if (!mounted || request !== sessionRequest || campaignId !== requestedCampaign) return;
        const reconciled = [...loaded];
        const loadedIds = new Set(loaded.map(({ id }) => id));
        for (const scope of sessionLoadFence.settle(request, requestedCampaign)) {
          const createdId = scope.slice(prefix.length);
          const session = backendSessions.find(
            ({ id, campaign_id }) => id === createdId && campaign_id === requestedCampaign,
          );
          if (session && !loadedIds.has(session.id)) {
            reconciled.push(session);
          }
        }
        for (const session of reconciled) {
          const scope = sessionScope(requestedCampaign, session.id);
          const currentAcknowledgment =
            draftCoordinator.get<SessionDraftValue>(scope)?.lastAcknowledgedAttemptId ?? 0;
          const requestAcknowledgment = acknowledgmentsAtStart.get(scope) ?? 0;
          if (currentAcknowledgment > requestAcknowledgment) continue;
          draftCoordinator.open(
            scope,
            `session:${session.id}`,
            sessionDraftValue(session),
            'authoritative-list',
          );
          leaseProjection(scope);
        }
        const reconciledScopes = new Set(
          reconciled.map((session) => sessionScope(requestedCampaign, session.id)),
        );
        for (const draft of draftCoordinator.listByPrefix<SessionDraftValue>(prefix)) {
          if (!reconciledScopes.has(draft.scope) && statusOf(draft) === 'saved') {
            releaseProjection(draft.scope);
          }
        }
        backendSessions = reconciled;
        loading = false;
      },
      (error: unknown) => {
        if (!mounted || request !== sessionRequest || campaignId !== requestedCampaign) return;
        console.error('Failed to load sessions:', error);
        const acknowledgedCreates = sessionLoadFence.settle(request, requestedCampaign);
        backendSessions = backendSessions.filter((session) => {
          if (session.campaign_id !== requestedCampaign) return false;
          return acknowledgedCreates.has(sessionScope(requestedCampaign, session.id));
        });
        loading = false;
      },
    );
  });

  $effect(() => {
    const requestedCampaign = campaignId;
    const request = ++entityRequest;
    Promise.all(ALL_KINDS.map((kind) => getEntities(requestedCampaign, kind))).then(
      (results) => {
        if (!mounted || request !== entityRequest || campaignId !== requestedCampaign) return;
        entityMap.clear();
        for (const list of results) {
          for (const node of list) {
            entityMap.set(node.name, { id: node.id, kind: node.kind });
          }
        }
      },
      (error: unknown) => {
        if (!mounted || request !== entityRequest || campaignId !== requestedCampaign) return;
        console.error('Failed to load entities for WikiText:', error);
      },
    );
  });

  async function saveNewSession(
    createDraft: DraftRecord<SessionDraftValue>,
    requestedCampaign: string,
    focusOnSuccess = false,
  ): Promise<void> {
    const saveScope = createDraft.scope;
    let acknowledgedSession: Session | null = null;

    await draftCoordinator.requestSave<SessionDraftValue>(saveScope, async (value) => {
      const created = await createSession(requestedCampaign, value);
      if (created.campaign_id !== requestedCampaign) {
        throw new Error(`Session creation acknowledged the wrong campaign: ${created.campaign_id}`);
      }
      acknowledgedSession = created;
      return sessionDraftValue(created);
    });

    const settled = draftCoordinator.get<SessionDraftValue>(saveScope);
    if (settled?.error || !acknowledgedSession) return;

    const created = acknowledgedSession as Session;
    const scope = sessionScope(requestedCampaign, created.id);
    const promotion = draftCoordinator.tryRemapAfterCreate<SessionDraftValue>(
      saveScope,
      scope,
      `session:${created.id}`,
    );
    retryingCreateScope = null;
    if (promotion.outcome !== 'promoted') return;

    if (!mounted || campaignId !== requestedCampaign) return;
    leaseProjection(scope);
    sessionLoadFence.acknowledge(requestedCampaign, scope);
    backendSessions = [
      ...backendSessions.filter(
        (session) => session.campaign_id === requestedCampaign && session.id !== created.id,
      ),
      created,
    ];
    if (focusOnSuccess) await focusSessionHeader(created.id);
  }

  async function handleNewSession(): Promise<void> {
    const requestedCampaign = campaignId;
    if (
      draftCoordinator.listByPrefix<SessionDraftValue>(`session-new:${requestedCampaign}:`).length >
      0
    ) {
      return;
    }
    const nextNumber =
      sessions.length === 0 ? 1 : Math.max(...sessions.map((s) => s.session_number)) + 1;
    const today = new Date().toISOString().slice(0, 10);
    const input: SessionDraftValue = {
      sessionNumber: nextNumber,
      title: i18n.t('sessions.defaultTitle', { number: nextNumber }),
      datePlayed: today,
      notes: '',
    };
    const scope = newSessionScope(requestedCampaign, allocateNewDraftId());
    draftCoordinator.open(scope, scope, EMPTY_SESSION_CREATE);
    const draft = draftCoordinator.revise(scope, input);
    await saveNewSession(draft, requestedCampaign);
  }

  async function focusSessionHeader(sessionId: string): Promise<void> {
    await tick();
    const header = Array.from(
      sessionLogElement?.querySelectorAll<HTMLButtonElement>('.session-header') ?? [],
    ).find((candidate) => candidate.dataset.sessionId === sessionId);
    header?.focus();
  }

  async function retryNewSession(event: MouseEvent): Promise<void> {
    const attempt = retainedCreate;
    if (!attempt || attempt.inFlight !== null || !attempt.scope.startsWith(newSessionPrefix))
      return;
    const shouldRestoreFocus = event.currentTarget === document.activeElement;
    const promotionIssue = draftCoordinator.getCreatePromotionIssue(attempt.scope);
    if (promotionIssue) {
      const result = draftCoordinator.retryCreatePromotion<SessionDraftValue>(attempt.scope);
      if (result.outcome === 'promoted') {
        if (shouldRestoreFocus) {
          const createdId = promotionIssue.destinationScope.slice(sessionPrefix.length);
          await focusSessionHeader(createdId);
        }
        return;
      }
      if (shouldRestoreFocus) {
        await tick();
        sessionLogElement?.querySelector<HTMLButtonElement>('.promotion-alert button')?.focus();
      }
      return;
    }
    retryingCreateScope = attempt.scope;
    await saveNewSession(attempt, campaignId, shouldRestoreFocus);
  }

  async function discardNewSession(): Promise<void> {
    const attempt = retainedCreate;
    if (!attempt || attempt.inFlight !== null) return;
    if (draftCoordinator.discard(attempt.scope) !== 'discarded') return;
    retryingCreateScope = null;
    await tick();
    sessionLogElement?.querySelector<HTMLButtonElement>('.new-session-button')?.focus();
  }

  function handleUpdate(updated: Session) {
    backendSessions = backendSessions.map((session) =>
      session.id === updated.id ? updated : session,
    );
  }

  async function handleDelete(id: string): Promise<void> {
    const deletedIndex = sessions.findIndex((session) => session.id === id);
    const nextFocusId = sessions[deletedIndex + 1]?.id ?? sessions[deletedIndex - 1]?.id ?? null;
    sessionLoadFence.forget(sessionScope(campaignId, id));
    backendSessions = backendSessions.filter((session) => session.id !== id);
    if (nextFocusId) {
      await focusSessionHeader(nextFocusId);
    } else {
      await tick();
      sessionLogElement?.querySelector<HTMLButtonElement>('.new-session-button')?.focus();
    }
  }

  async function handleDiscardUnavailable(id: string) {
    releaseProjection(sessionScope(campaignId, id));
    await tick();
    sessionLogElement?.querySelector<HTMLButtonElement>('.new-session-button')?.focus();
  }
</script>

<div class="session-log" bind:this={sessionLogElement}>
  <div class="session-log-head">
    <div>
      <h1>{i18n.t('sessions.title')}</h1>
      <p class="sub">{i18n.t('sessions.subtitle')}</p>
    </div>
    <Button class="new-session-button" onclick={handleNewSession} disabled={retainedCreate !== null}
      >+ {i18n.t('sessions.newSession')}</Button
    >
  </div>
  {#if retainedCreate && showCreateRecovery}
    <div class="create-failure" role="alert" aria-busy={retainedCreate.inFlight !== null}>
      <div class="create-failure-message">
        <strong>{i18n.t('sessions.createFailed')}</strong>
        {#if retainedCreate.error}<span>{retainedCreate.error}</span>{/if}
      </div>
      <div class="create-failure-actions">
        <Button
          variant="secondary"
          onclick={retryNewSession}
          disabled={retainedCreate.inFlight !== null}>{i18n.t('drafts.retry')}</Button
        >
        <Button
          variant="ghost"
          onclick={discardNewSession}
          disabled={retainedCreate.inFlight !== null}>{i18n.t('drafts.discardChanges')}</Button
        >
      </div>
    </div>
  {/if}
  {#if retainedCreate && createPromotionIssue}
    <div class="create-failure promotion-alert" role="alert">
      <div class="create-failure-message">
        <strong>{i18n.t('drafts.createdNeedsAttention')}</strong>
        <span>{i18n.t('drafts.finishCreatedSession')}</span>
      </div>
      <div class="create-failure-actions">
        <Button variant="secondary" onclick={retryNewSession}>{i18n.t('drafts.retry')}</Button>
      </div>
    </div>
  {/if}
  {#if loading}
    <p class="muted">{i18n.t('sessions.loading')}</p>
  {:else if sessions.length === 0}
    <div class="empty">{i18n.t('sessions.empty')}</div>
  {:else}
    <SessionList
      {campaignId}
      {sessions}
      {entityMap}
      onUpdate={handleUpdate}
      onDelete={handleDelete}
      onDiscardUnavailable={handleDiscardUnavailable}
      {unavailableSessionIds}
      {draftCoordinator}
    />
  {/if}
</div>

<style>
  .session-log {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 24px 28px;
    gap: 20px;
    overflow-y: auto;
    font-family: var(--font-sans);
  }

  .session-log-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
  }

  .session-log-head h1 {
    margin: 0 0 2px;
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 600;
    color: var(--fg-1);
  }

  .sub {
    margin: 0;
    font-size: 13px;
    color: var(--fg-3);
  }

  .muted {
    color: var(--fg-3);
    font-size: 14px;
    margin: 0;
  }

  .create-failure {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--s-3);
    padding: var(--s-3);
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    border-radius: var(--r-md);
    background: var(--danger-bg);
    color: var(--danger);
    font: 500 0.8125rem/1.4 var(--font-sans);
  }

  .create-failure-message {
    display: flex;
    min-width: min(100%, 18rem);
    flex: 1;
    flex-direction: column;
    gap: var(--s-1);
  }

  .create-failure-message span {
    color: var(--fg-2);
    overflow-wrap: anywhere;
  }

  .create-failure-actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }

  .empty {
    color: var(--fg-3);
    font-size: 14px;
    padding: 32px 0;
    text-align: center;
  }
</style>
