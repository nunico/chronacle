<script lang="ts">
  import { onDestroy, untrack } from 'svelte';
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
  import { sessionScope, statusOf } from '../lib/drafts/draft-state';

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
  let createAcknowledgmentGeneration = 0;
  const acknowledgedCreates = new Map<string, number>();

  onDestroy(() => {
    mounted = false;
  });

  let sessionPrefix = $derived(`session:${campaignId}:`);
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
    const createGenerationAtStart = createAcknowledgmentGeneration;
    const prefix = `session:${requestedCampaign}:`;
    const acknowledgmentsAtStart = untrack(
      () =>
        new Map(
          draftCoordinator
            .listByPrefix<SessionDraftValue>(prefix)
            .map((draft) => [draft.scope, draft.lastAcknowledgedAttemptId]),
        ),
    );
    loading = true;

    getSessions(requestedCampaign).then(
      (loaded) => {
        if (!mounted || request !== sessionRequest || campaignId !== requestedCampaign) return;
        const reconciled = [...loaded];
        const loadedIds = new Set(loaded.map(({ id }) => id));
        for (const [scope, generation] of acknowledgedCreates) {
          if (generation <= createGenerationAtStart || !scope.startsWith(prefix)) continue;
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
        }
        for (const [scope, generation] of acknowledgedCreates) {
          if (scope.startsWith(prefix) && generation <= createGenerationAtStart) {
            acknowledgedCreates.delete(scope);
          }
        }
        backendSessions = reconciled;
        loading = false;
      },
      (error: unknown) => {
        if (!mounted || request !== sessionRequest || campaignId !== requestedCampaign) return;
        console.error('Failed to load sessions:', error);
        backendSessions = [];
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

  async function handleNewSession() {
    const requestedCampaign = campaignId;
    const nextNumber =
      sessions.length === 0 ? 1 : Math.max(...sessions.map((s) => s.session_number)) + 1;
    const today = new Date().toISOString().slice(0, 10);
    try {
      const created = await createSession(requestedCampaign, {
        sessionNumber: nextNumber,
        title: i18n.t('sessions.defaultTitle', { number: nextNumber }),
        datePlayed: today,
        notes: '',
      });
      if (!mounted || campaignId !== requestedCampaign) return;
      const scope = sessionScope(requestedCampaign, created.id);
      draftCoordinator.open(
        scope,
        `session:${created.id}`,
        sessionDraftValue(created),
        'authoritative-list',
      );
      createAcknowledgmentGeneration += 1;
      acknowledgedCreates.set(scope, createAcknowledgmentGeneration);
      backendSessions = [
        ...backendSessions.filter((session) => session.campaign_id === requestedCampaign),
        created,
      ];
    } catch (e) {
      console.error('Failed to create session:', e);
    }
  }

  function handleUpdate(updated: Session) {
    backendSessions = backendSessions.map((session) =>
      session.id === updated.id ? updated : session,
    );
  }

  function handleDelete(id: string) {
    acknowledgedCreates.delete(sessionScope(campaignId, id));
    backendSessions = backendSessions.filter((session) => session.id !== id);
  }
</script>

<div class="session-log">
  <div class="session-log-head">
    <div>
      <h1>{i18n.t('sessions.title')}</h1>
      <p class="sub">{i18n.t('sessions.subtitle')}</p>
    </div>
    <Button onclick={handleNewSession}>+ {i18n.t('sessions.newSession')}</Button>
  </div>
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

  .empty {
    color: var(--fg-3);
    font-size: 14px;
    padding: 32px 0;
    text-align: center;
  }
</style>
