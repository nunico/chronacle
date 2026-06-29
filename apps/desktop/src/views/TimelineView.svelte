<script lang="ts">
  import { onMount } from 'svelte';
  import { getEventsTimeline, getSessions, type GraphNode, type Session } from '../lib/commands';
  import { groupByEra, type EraGroup } from '../lib/timeline-groups';
  import { groupBySession, type SessionLane } from '../lib/timeline-sessions';

  interface Props {
    campaignId: string;
    onOpenEntity?: (e: GraphNode) => void; // deep-link: open this event in its notebook
  }
  const { campaignId, onOpenEntity }: Props = $props();

  let mode = $state<'chronicle' | 'sessions'>('chronicle');
  let events = $state<GraphNode[]>([]);
  let sessions = $state<Session[]>([]);
  let loading = $state(true);
  let error = $state('');

  const eraGroups = $derived<EraGroup[]>(groupByEra(events));
  const sessionLanes = $derived<SessionLane[]>(groupBySession(sessions, events));

  onMount(load);

  async function load() {
    loading = true;
    error = '';
    try {
      [events, sessions] = await Promise.all([
        getEventsTimeline(campaignId),
        getSessions(campaignId),
      ]);
    } catch (e) {
      console.error('Failed to load timeline:', e);
      error = 'Failed to load timeline';
      events = [];
      sessions = [];
    } finally {
      loading = false;
    }
  }

  function dateLabel(e: GraphNode): string {
    if (e.duration_label) return e.duration_label;
    if (e.date_start && e.date_end) return `${e.date_start} – ${e.date_end}`;
    return e.date_start ?? e.date_end ?? '';
  }
</script>

<div class="timeline">
  <div class="toolbar" role="tablist" aria-label="Timeline mode">
    <button role="tab" aria-selected={mode === 'chronicle'} class:active={mode === 'chronicle'}
      onclick={() => (mode = 'chronicle')}>Chronicle</button>
    <button role="tab" aria-selected={mode === 'sessions'} class:active={mode === 'sessions'}
      onclick={() => (mode = 'sessions')}>Sessions</button>
  </div>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if mode === 'chronicle'}
    {#if eraGroups.length === 0 && !error}
      <p class="muted">No events yet. Add events in the Events notebook to build your timeline.</p>
    {:else}
      <ol class="spine">
        {#each eraGroups as group (group.era ?? '__none__')}
          <li class="era">
            <h3 class="era-head">{group.era ?? 'Unordered'}</h3>
            <ol class="events">
              {#each group.events as e (e.id)}
                {@const when = dateLabel(e)}
                <li class="event">
                  <button class="name" onclick={() => onOpenEntity?.(e)}>{e.name}</button>
                  {#if when}<span class="when">{when}</span>{/if}
                  {#if e.is_ongoing}<span class="ongoing">ongoing</span>{/if}
                </li>
              {/each}
            </ol>
          </li>
        {/each}
      </ol>
    {/if}
  {:else}
    {#if sessions.length === 0 && events.length === 0 && !error}
      <p class="muted">No sessions or events yet.</p>
    {:else}
      <ol class="lanes">
        {#each sessionLanes as lane (lane.session?.id ?? '__none__')}
          <li class="lane">
            <h3 class="lane-head">{lane.session?.title ?? 'Unscheduled'}</h3>
            <ol class="events">
              {#each lane.events as e (e.id)}
                <li class="event"><button class="name" onclick={() => onOpenEntity?.(e)}>{e.name}</button></li>
              {/each}
            </ol>
          </li>
        {/each}
      </ol>
    {/if}
  {/if}
</div>

<style>
  .timeline { padding: 20px; }
  .toolbar { display: flex; gap: 6px; margin-bottom: 16px; }
  .toolbar button { padding: 6px 14px; border-radius: var(--r-md); background: var(--bg-panel); border: 1px solid var(--line); color: var(--fg-2); cursor: pointer; }
  .toolbar button.active { color: var(--fg-1); border-color: var(--violet-400); }
  .spine { list-style: none; margin: 0; padding: 0; }
  .era-head { font-family: var(--font-display); color: var(--violet-400); margin: 18px 0 8px; }
  .events { list-style: none; margin: 0; padding-left: 16px; border-left: 2px solid var(--line); }
  .event { padding: 6px 0; display: flex; gap: 10px; align-items: baseline; }
  .event .name { background: none; border: none; padding: 0; color: var(--fg-1); cursor: pointer; font: inherit; text-align: left; }
  .event .name:hover { color: var(--violet-400); }
  .event .when { color: var(--fg-3); font-size: 12px; }
  .event .ongoing { color: var(--violet-400); font-size: 11px; text-transform: uppercase; }
  .lanes { list-style: none; margin: 0; padding: 0; }
  .lane { padding-bottom: 12px; }
  .lane-head { font-family: var(--font-display); color: var(--violet-400); margin: 18px 0 8px; }
  .muted { color: var(--fg-3); }
  .error {
    padding: 8px 12px;
    background: var(--danger-bg);
    color: var(--danger);
    border: 1px solid rgba(242, 103, 75, 0.4);
    border-radius: var(--r-md);
    margin-bottom: 14px;
    font-size: 13px;
  }
</style>
