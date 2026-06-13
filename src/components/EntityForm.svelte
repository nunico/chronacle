<script lang="ts">
  import type { EntityKind, GraphNode, EntityInput, Session } from '../lib/commands';
  import WikiLinkEditor from './WikiLinkEditor.svelte';

  interface Props {
    kind: EntityKind;
    node?: GraphNode | null;
    error?: { code: string; message: string; field?: string } | null;
    onsave?: (input: EntityInput) => void;
    oncancel?: () => void;
    sessions?: Session[]; // list of campaign sessions for event dropdown
    entityMap?: Map<string, { id: string; kind: string }>; // for wikilink autocomplete
  }

  let { kind, node = null, error = null, onsave, oncancel, sessions = [], entityMap = new Map() }: Props = $props();

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
</style>
