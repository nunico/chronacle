<script lang="ts">
  import type { EntityKind, GraphNode, EntityInput } from '../lib/commands';

  interface Props {
    kind: EntityKind;
    node?: GraphNode | null;
    error?: { code: string; message: string; field?: string } | null;
    onsave?: (input: EntityInput) => void;
    oncancel?: () => void;
  }

  let { kind, node = null, error = null, onsave, oncancel }: Props = $props();

  let name = $state(node?.name ?? '');
  let summary = $state(node?.summary ?? '');
  let notes = $state(node?.notes ?? '');
  // event fields
  let dateStart = $state(node?.date_start ?? '');
  let dateEnd = $state(node?.date_end ?? '');
  let isOngoing = $state(node?.is_ongoing ?? false);
  let sequenceIndex = $state(node?.sequence_index?.toString() ?? '');
  let era = $state(node?.era ?? '');
  let durationLabel = $state(node?.duration_label ?? '');
  // pc fields
  let playerName = $state(node?.player_name ?? '');
  let characterClass = $state(node?.character_class ?? '');
  let characterLevel = $state(node?.character_level?.toString() ?? '');
  let status = $state(node?.status ?? '');

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
    <textarea id="ef-notes" bind:value={notes} rows="4"></textarea>
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
  input, textarea, select {
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
