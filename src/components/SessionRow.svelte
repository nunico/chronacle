<script lang="ts">
  import {
    updateSession,
    deleteSession,
    getSessionEntities,
    type Session,
    type GraphNode,
  } from '../lib/commands';
  import WikiText from './WikiText.svelte';
  import WikiLinkEditor from './WikiLinkEditor.svelte';
  import { formatDate } from '../lib/locale.svelte';

  interface Props {
    session: Session;
    entityMap: Map<string, { id: string; kind: string }>;
    onUpdate: (session: Session) => void;
    onDelete: (id: string) => void;
  }

  const { session, entityMap, onUpdate, onDelete }: Props = $props();

  let expanded = $state(false);
  // Writable $derived: edit fields seed from `session` and re-seed when the
  // prop changes (the row instance is reused across updates since the list is
  // keyed by session.id), while bind:value edits still override until then.
  let editTitle = $derived(session.title);
  let editDate = $derived(session.date_played);
  let editNotes = $derived(session.notes);
  let editGmOnly = $derived(session.is_gm_only);
  let linkedEntities = $state<GraphNode[]>([]);
  let loadingEntities = $state(false);
  let entitiesLoaded = $state(false);

  async function toggleExpand() {
    expanded = !expanded;
    if (expanded && !entitiesLoaded && !loadingEntities) {
      loadingEntities = true;
      try {
        linkedEntities = await getSessionEntities(session.id);
      } catch (e) {
        console.error('Failed to load session entities:', e);
      } finally {
        loadingEntities = false;
        entitiesLoaded = true;
      }
    }
  }

  async function saveField() {
    if (
      editTitle === session.title &&
      editDate === session.date_played &&
      editNotes === session.notes &&
      editGmOnly === session.is_gm_only
    ) {
      return;
    }
    try {
      const updated = await updateSession(session.id, {
        sessionNumber: session.session_number,
        title: editTitle,
        datePlayed: editDate,
        notes: editNotes,
        isGmOnly: editGmOnly,
      });
      onUpdate(updated);
    } catch (e) {
      console.error('Failed to update session:', e);
    }
  }

  async function handleDelete() {
    if (!confirm(`Delete "${session.title}"? This cannot be undone.`)) return;
    try {
      await deleteSession(session.id);
      onDelete(session.id);
    } catch (e) {
      console.error('Failed to delete session:', e);
    }
  }
</script>

<div class="session-row" class:expanded>
  <!-- Collapsed header — always visible, click to toggle -->
  <button type="button" class="session-header" onclick={toggleExpand} aria-expanded={expanded}>
    <span class="session-number">#{session.session_number}</span>
    <span class="session-title">{session.title}</span>
    <span class="session-date">{formatDate(session.date_played)}</span>
    {#if linkedEntities.length > 0}
      <span class="session-events"
        >{linkedEntities.length} event{linkedEntities.length === 1 ? '' : 's'}</span
      >
    {/if}
    <span class="chevron" class:rotated={expanded}>›</span>
  </button>

  {#if expanded}
    <div class="session-body">
      <div class="field-row">
        <label for="title-{session.id}" class="field-label">Title</label>
        <input
          id="title-{session.id}"
          class="field-input"
          type="text"
          bind:value={editTitle}
          onblur={saveField}
        />
      </div>

      <div class="field-row">
        <label for="date-{session.id}" class="field-label">Date played</label>
        <input
          id="date-{session.id}"
          class="field-input"
          type="date"
          bind:value={editDate}
          onblur={saveField}
        />
      </div>

      <div class="field-col">
        <label for="notes-{session.id}" class="field-label">Notes</label>
        <WikiLinkEditor
          id="notes-{session.id}"
          bind:value={editNotes}
          entities={entityMap}
          onblur={saveField}
          rows={6}
          placeholder="Session recap, rewards, open threads… Use [[Entity Name]] to link."
        />
        {#if editNotes}
          <div class="wiki-preview">
            <WikiText text={editNotes} entities={entityMap} />
          </div>
        {/if}
      </div>

      <div class="field-row">
        <label class="gm-toggle">
          <input type="checkbox" bind:checked={editGmOnly} onchange={saveField} />
          GM only (secret)
        </label>
      </div>

      {#if loadingEntities}
        <p class="muted">Loading linked events…</p>
      {:else if linkedEntities.length > 0}
        <div class="linked-entities">
          {#each linkedEntities as e (e.id)}
            <span class="entity-badge" title={e.kind}>{e.name}</span>
          {/each}
        </div>
      {/if}

      <div class="session-actions">
        <button type="button" class="btn-delete" onclick={handleDelete}>Delete</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .session-row {
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg-panel);
    overflow: hidden;
    font-family: var(--font-sans);
  }

  .session-row.expanded {
    border-color: var(--line-strong);
  }

  .session-header {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: none;
    border: none;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 14px;
    cursor: pointer;
    text-align: left;
  }

  .session-header:hover {
    background: var(--bg-hover, rgba(255, 255, 255, 0.04));
  }

  .session-number {
    font-size: 12px;
    color: var(--fg-3);
    flex-shrink: 0;
    min-width: 28px;
  }

  .session-title {
    flex: 1;
    font-weight: 500;
    color: var(--fg-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-date {
    font-size: 12.5px;
    color: var(--fg-3);
    flex-shrink: 0;
  }

  .session-events {
    font-size: 12px;
    color: var(--arcane-300, #a78bfa);
    flex-shrink: 0;
  }

  .chevron {
    font-size: 16px;
    color: var(--fg-3);
    flex-shrink: 0;
    transition: transform 0.15s ease;
    display: inline-block;
  }

  .chevron.rotated {
    transform: rotate(90deg);
  }

  .session-body {
    padding: 0 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-top: 1px solid var(--line);
  }

  .field-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .field-col {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field-label {
    font-size: 12px;
    color: var(--fg-3);
    min-width: 80px;
    flex-shrink: 0;
  }

  .gm-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--fg-3);
    cursor: pointer;
  }

  .field-input {
    flex: 1;
    padding: 7px 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--bg-inset);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
  }

  .field-input:focus {
    outline: none;
    border-color: var(--line-glow);
  }

  .wiki-preview {
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--bg-void, rgba(0, 0, 0, 0.2));
    font-size: 13px;
    color: var(--fg-2);
    line-height: 1.5;
  }

  .linked-entities {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .muted {
    font-size: 12.5px;
    color: var(--fg-3);
    margin: 0;
  }

  .session-actions {
    display: flex;
    justify-content: flex-end;
    padding-top: 4px;
  }

  .btn-delete {
    background: none;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    padding: 6px 12px;
    font-size: 12.5px;
    color: var(--fg-3);
    font-family: var(--font-sans);
    cursor: pointer;
  }

  .btn-delete:hover {
    border-color: var(--danger, #d97757);
    color: var(--danger, #d97757);
  }
</style>
