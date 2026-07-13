<script lang="ts">
  import {
    getEntityRelations,
    listVaultConflicts,
    type EntityKind,
    type GraphNode,
    type EntityInput,
    type Session,
    type RelatedEntity,
    type VaultConflict,
  } from '../lib/commands';
  import WikiLinkEditor from './WikiLinkEditor.svelte';

  interface Props {
    kind: EntityKind;
    node?: GraphNode | null;
    error?: { code: string; message: string; field?: string } | null;
    onsave?: (input: EntityInput) => void;
    oncancel?: () => void;
    sessions?: Session[]; // list of campaign sessions for event dropdown
    entityMap?: Map<string, { id: string; kind: string }>; // for wikilink autocomplete
    onOpenEntity?: (id: string, kind: string) => void;
  }

  let { kind, node = null, error = null, onsave, oncancel, sessions = [], entityMap = new Map(), onOpenEntity }: Props = $props();

  // Writable $derived: each field seeds from `node` and recomputes when a
  // different entity is selected, while remaining editable via bind:value
  // (user edits override the derived until `node` changes again).
  let name = $derived(node?.name ?? '');
  let summary = $derived(node?.summary ?? '');
  let notes = $derived(node?.notes ?? '');
  // event fields
  let dateStart = $derived(node?.date_start ?? '');
  let dateEnd = $derived(node?.date_end ?? '');
  let isOngoing = $derived(node?.is_ongoing ?? false);
  let sequenceIndex = $derived(node?.sequence_index?.toString() ?? '');
  let era = $derived(node?.era ?? '');
  let durationLabel = $derived(node?.duration_label ?? '');
  // event session FK
  let sessionId = $derived(node?.session_id ?? '');
  // pc fields
  let playerName = $derived(node?.player_name ?? '');
  let characterClass = $derived(node?.character_class ?? '');
  let characterLevel = $derived(node?.character_level?.toString() ?? '');
  let status = $derived(node?.status ?? '');

  let nameError = $state('');

  // Relationships section — only fetched for existing (saved) entities.
  let relations = $state<RelatedEntity[]>([]);

  // Vault conflict banner — checked whenever the open entity changes. When no
  // vault is configured, listVaultConflicts() resolves to [] and this simply
  // never matches.
  let conflict = $state<VaultConflict | null>(null);

  $effect(() => {
    const currentId = node?.id;
    const currentKind = kind;
    if (!currentId) {
      conflict = null;
      return;
    }
    let cancelled = false;
    listVaultConflicts().then(
      (result) => {
        if (cancelled) return;
        const list = Array.isArray(result) ? result : [];
        conflict = list.find((c) => c.id === currentId && c.kind === currentKind) ?? null;
      },
      () => {
        if (!cancelled) conflict = null;
      },
    );
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const currentId = node?.id;
    const currentKind = kind;
    if (!currentId) {
      relations = [];
      return;
    }
    // Guard against a stale in-flight fetch overwriting a newer entity's
    // relations if the user switches entities mid-request.
    let cancelled = false;
    getEntityRelations(currentId, currentKind).then(
      (result) => { if (!cancelled) relations = result; },
      (err) => {
        // Log and degrade gracefully — never block the form
        if (!cancelled) {
          console.error('Failed to fetch entity relations:', err);
          relations = [];
        }
      },
    );
    return () => { cancelled = true; };
  });

  const KIND_LABEL: Record<string, string> = {
    npc: 'NPC',
    location: 'Location',
    faction: 'Faction',
    creature: 'Creature',
    item: 'Item',
    event: 'Event',
    player_character: 'PC',
    misc: 'Misc',
  };

  function handleSubmit() {
    nameError = '';
    if (!name.trim()) {
      nameError = 'Name is required';
      return;
    }
    const input: EntityInput = {
      name: name.trim(),
      summary: summary || null,
      notes: notes || null,
      dateStart: dateStart || null,
      dateEnd: dateEnd || null,
      isOngoing: isOngoing || null,
      sequenceIndex: sequenceIndex ? parseInt(sequenceIndex, 10) : null,
      era: era || null,
      durationLabel: durationLabel || null,
      sessionId: sessionId || null,
      playerName: playerName || null,
      characterClass: characterClass || null,
      characterLevel: characterLevel ? parseInt(characterLevel, 10) : null,
      status: status || null,
    };
    onsave?.(input);
  }
</script>

{#if conflict}
  <div class="conflict-banner" role="alert">
    This record has unsynced vault edits in conflict — resolve in your vault ({conflict.sidecarKey}).
  </div>
{/if}

<form aria-label="entity form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
  <div class="field">
    <label for="ef-name">Name</label>
    <input id="ef-name" type="text" bind:value={name} />
    {#if nameError}<p class="field-error">{nameError}</p>{/if}
    {#if error?.field === 'name'}<p class="field-error">{error.message}</p>{/if}
  </div>

  <div class="field">
    <label for="ef-summary">Summary</label>
    <input id="ef-summary" type="text" bind:value={summary} />
  </div>

  <div class="field">
    <label for="ef-notes">Notes</label>
    <WikiLinkEditor
      id="ef-notes"
      bind:value={notes}
      entities={entityMap}
      rows={4}
      placeholder="Use [[Entity Name]] to link."
    />
  </div>

  {#if kind === 'event'}
    <div class="field">
      <label for="ef-date-start">Date Start</label>
      <input id="ef-date-start" type="text" bind:value={dateStart} />
    </div>
    <div class="field">
      <label for="ef-date-end">Date End</label>
      <input id="ef-date-end" type="text" bind:value={dateEnd} />
    </div>
    <div class="field">
      <label for="ef-seq">Sequence Index</label>
      <input id="ef-seq" type="number" bind:value={sequenceIndex} />
    </div>
    <div class="field">
      <label for="ef-era">Era</label>
      <input id="ef-era" type="text" bind:value={era} />
    </div>
    <div class="field">
      <label for="ef-dur">Duration Label</label>
      <input id="ef-dur" type="text" bind:value={durationLabel} />
    </div>
    <div class="field checkbox">
      <label>
        <input type="checkbox" bind:checked={isOngoing} />
        Ongoing
      </label>
    </div>
    <div class="field">
      <label for="ef-session">Session</label>
      <select id="ef-session" bind:value={sessionId}>
        <option value="">— none —</option>
        {#each sessions as s (s.id)}
          <option value={s.id}>#{s.session_number}: {s.title}</option>
        {/each}
      </select>
    </div>
  {/if}

  {#if kind === 'player_character'}
    <div class="field">
      <label for="ef-player">Player Name</label>
      <input id="ef-player" type="text" bind:value={playerName} />
    </div>
    <div class="field">
      <label for="ef-class">Character Class</label>
      <input id="ef-class" type="text" bind:value={characterClass} />
    </div>
    <div class="field">
      <label for="ef-level">Character Level</label>
      <input id="ef-level" type="number" min="1" max="20" bind:value={characterLevel} />
    </div>
    <div class="field">
      <label for="ef-status">Status</label>
      <select id="ef-status" bind:value={status}>
        <option value="">— select —</option>
        <option value="active">Active</option>
        <option value="retired">Retired</option>
        <option value="deceased">Deceased</option>
        <option value="missing">Missing</option>
        <option value="on_hiatus">On Hiatus</option>
      </select>
    </div>
  {/if}

  {#if error && !error.field}
    <p class="form-error">{error.message}</p>
  {/if}

  <div class="actions">
    <button type="submit" class="btn-primary">{node ? 'Save' : 'Create'}</button>
    <button type="button" class="btn-ghost" onclick={() => oncancel?.()}>Cancel</button>
  </div>

  {#if node?.id}
    <div class="relationships-section">
      <h3 class="relationships-heading">Relationships</h3>
      {#if relations.length === 0}
        <p class="relationships-empty">No relationships yet.</p>
      {:else}
        <ul class="relationships-list">
          {#each relations as rel (rel.id + rel.direction + rel.rel_type)}
            <li class="rel-row">
              {#if onOpenEntity}
                <button
                  type="button"
                  class="rel-row-btn"
                  onclick={() => onOpenEntity(rel.id, rel.kind)}
                  aria-label="Open {rel.name}"
                >
                  <span class="rel-direction">{rel.direction === 'outbound' ? '→' : '←'}</span>
                  <span class="rel-name">{rel.name}</span>
                  <span class="rel-kind">{KIND_LABEL[rel.kind] ?? rel.kind}</span>
                  <span class="rel-type">{rel.rel_type}</span>
                </button>
              {:else}
                <span class="rel-row-inner">
                  <span class="rel-direction">{rel.direction === 'outbound' ? '→' : '←'}</span>
                  <span class="rel-name">{rel.name}</span>
                  <span class="rel-kind">{KIND_LABEL[rel.kind] ?? rel.kind}</span>
                  <span class="rel-type">{rel.rel_type}</span>
                </span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</form>

<style>
  form { display: flex; flex-direction: column; gap: 12px; }
  .field { display: flex; flex-direction: column; gap: 4px; }
  label { font-size: 0.85rem; color: var(--fg-3); }
  input, select {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--fg-1);
    padding: 6px 10px;
    font-size: 0.9rem;
  }
  .field-error, .form-error { color: var(--danger); font-size: 0.8rem; margin: 0; }
  .conflict-banner {
    background: var(--danger-bg, rgba(220, 38, 38, 0.12));
    color: var(--danger);
    border: 1px solid var(--danger);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 0.85rem;
    margin-bottom: 4px;
  }
  .actions { display: flex; gap: 8px; margin-top: 8px; }
  .btn-primary {
    background: var(--violet-300);
    color: var(--bg-abyss);
    border: none;
    border-radius: 6px;
    padding: 6px 16px;
    cursor: pointer;
    font-weight: 600;
  }
  .btn-ghost {
    background: transparent;
    color: var(--fg-3);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 6px 16px;
    cursor: pointer;
  }

  /* ── Relationships section ─────────────────────────────────────────── */
  .relationships-section {
    border-top: 1px solid var(--line);
    padding-top: 12px;
    margin-top: 4px;
  }
  .relationships-heading {
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--fg-3);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 0 0 8px;
  }
  .relationships-empty {
    font-size: 0.85rem;
    color: var(--fg-4, var(--fg-3));
    margin: 0;
    font-style: italic;
  }
  .relationships-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .rel-row {
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--bg-panel-2);
    overflow: hidden;
  }
  /* Shared inner layout for both interactive (button) and static (span) rows */
  .rel-row-btn,
  .rel-row-inner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    font-size: 0.85rem;
    width: 100%;
    text-align: left;
  }
  .rel-row-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: inherit;
    font-family: inherit;
  }
  .rel-row-btn:hover {
    background: var(--bg-inset, var(--bg-panel-2));
  }
  .rel-direction {
    flex: 0 0 16px;
    text-align: center;
    color: var(--fg-3);
    font-size: 0.9rem;
  }
  .rel-name {
    flex: 1;
    color: var(--fg-1);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rel-kind {
    flex: 0 0 auto;
    font-size: 0.72rem;
    color: var(--fg-3);
    background: var(--bg-panel, var(--bg-panel-2));
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 1px 5px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .rel-type {
    flex: 0 0 auto;
    font-size: 0.72rem;
    color: var(--violet-300, #a78bfa);
    background: rgba(124, 92, 255, 0.1);
    border-radius: 4px;
    padding: 1px 6px;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
