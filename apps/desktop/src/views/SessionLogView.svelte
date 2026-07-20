<script lang="ts">
  import { onMount } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import {
    getSessions,
    createSession,
    getEntities,
    type Session,
    type EntityKind,
  } from '../lib/commands';
  import SessionList from '../components/SessionList.svelte';
  import Button from '../components/ui/Button.svelte';
  import { i18n } from '../lib/locale.svelte';

  interface Props {
    campaignId: string;
  }

  const { campaignId }: Props = $props();

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

  let sessions = $state<Session[]>([]);
  let loading = $state(true);
  let entityMap = new SvelteMap<string, { id: string; kind: string }>();

  onMount(async () => {
    await Promise.all([loadSessions(), loadEntities()]);
    loading = false;
  });

  async function loadSessions() {
    try {
      sessions = await getSessions(campaignId);
    } catch (e) {
      console.error('Failed to load sessions:', e);
      sessions = [];
    }
  }

  async function loadEntities() {
    try {
      const results = await Promise.all(ALL_KINDS.map((k) => getEntities(campaignId, k)));
      entityMap.clear();
      for (const list of results) {
        for (const node of list) {
          entityMap.set(node.name, { id: node.id, kind: node.kind });
        }
      }
    } catch (e) {
      console.error('Failed to load entities for WikiText:', e);
    }
  }

  async function handleNewSession() {
    const nextNumber =
      sessions.length === 0 ? 1 : Math.max(...sessions.map((s) => s.session_number)) + 1;
    const today = new Date().toISOString().slice(0, 10);
    try {
      const created = await createSession(campaignId, {
        sessionNumber: nextNumber,
        title: i18n.t('sessions.defaultTitle', { number: nextNumber }),
        datePlayed: today,
        notes: '',
      });
      sessions = [...sessions, created];
    } catch (e) {
      console.error('Failed to create session:', e);
    }
  }

  function handleUpdate(updated: Session) {
    sessions = sessions.map((s) => (s.id === updated.id ? updated : s));
  }

  function handleDelete(id: string) {
    sessions = sessions.filter((s) => s.id !== id);
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
    <SessionList {sessions} {entityMap} onUpdate={handleUpdate} onDelete={handleDelete} />
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
