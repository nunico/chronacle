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
  import AliasField from './AliasField.svelte';
  import { i18n } from '../lib/locale.svelte';
  import type { MessageKey } from '../lib/i18n/messages';
  import Button from './ui/Button.svelte';
  import FormField from './ui/FormField.svelte';

  interface Props {
    kind: EntityKind;
    node?: GraphNode | null;
    error?: { code: string; message: string; field?: string } | null;
    onsave?: (input: EntityInput) => void;
    oncancel?: () => void;
    sessions?: Session[]; // list of campaign sessions for event dropdown
    entityMap?: Map<string, { id: string; kind: string }>; // for wikilink autocomplete
    onOpenEntity?: (id: string, kind: string) => void;
    initialName?: string;
    ondirtychange?: (dirty: boolean) => void;
  }

  let {
    kind,
    node = null,
    error = null,
    onsave,
    oncancel,
    sessions = [],
    entityMap = new Map(),
    onOpenEntity,
    initialName,
    ondirtychange,
  }: Props = $props();

  // Writable $derived: each field seeds from `node` and recomputes when a
  // different entity is selected, while remaining editable via bind:value
  // (user edits override the derived until `node` changes again).
  let name = $derived(node?.name ?? initialName ?? '');
  let aliases = $derived(node?.aliases ?? []);
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

  function snapshot(fields: {
    name: string;
    aliases: string[];
    summary: string;
    notes: string;
    dateStart: string;
    dateEnd: string;
    isOngoing: boolean;
    sequenceIndex: string;
    era: string;
    durationLabel: string;
    sessionId: string;
    playerName: string;
    characterClass: string;
    characterLevel: string;
    status: string;
  }) {
    return JSON.stringify(fields);
  }

  const initialSnapshot = $derived.by(() =>
    snapshot({
      name: node?.name ?? initialName ?? '',
      aliases: node?.aliases ?? [],
      summary: node?.summary ?? '',
      notes: node?.notes ?? '',
      dateStart: node?.date_start ?? '',
      dateEnd: node?.date_end ?? '',
      isOngoing: node?.is_ongoing ?? false,
      sequenceIndex: node?.sequence_index?.toString() ?? '',
      era: node?.era ?? '',
      durationLabel: node?.duration_label ?? '',
      sessionId: node?.session_id ?? '',
      playerName: node?.player_name ?? '',
      characterClass: node?.character_class ?? '',
      characterLevel: node?.character_level?.toString() ?? '',
      status: node?.status ?? '',
    }),
  );

  function notifyDirty() {
    const current = snapshot({
      name,
      aliases,
      summary,
      notes,
      dateStart,
      dateEnd,
      isOngoing,
      sequenceIndex,
      era,
      durationLabel,
      sessionId,
      playerName,
      characterClass,
      characterLevel,
      status,
    });
    ondirtychange?.(current !== initialSnapshot);
  }

  $effect(() => {
    notifyDirty();
  });

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
      (result) => {
        if (!cancelled) relations = result;
      },
      (err) => {
        // Log and degrade gracefully — never block the form
        if (!cancelled) {
          console.error('Failed to fetch entity relations:', err);
          relations = [];
        }
      },
    );
    return () => {
      cancelled = true;
    };
  });

  const KIND_LABEL: Record<string, MessageKey> = {
    npc: 'entityUi.kindNpc',
    location: 'entityUi.kindLocation',
    faction: 'entityUi.kindFaction',
    creature: 'entityUi.kindCreature',
    item: 'entityUi.kindItem',
    event: 'entityUi.kindEvent',
    player_character: 'entityUi.kindPlayerCharacter',
    misc: 'entityUi.kindMisc',
  };

  function kindLabel(value: string): string {
    const key = KIND_LABEL[value];
    return key ? i18n.t(key) : value;
  }

  function handleSubmit() {
    nameError = '';
    if (!name.trim()) {
      nameError = i18n.t('errors.validationRequired', { field: i18n.t('entityUi.name') });
      return;
    }
    const input: EntityInput = {
      name: name.trim(),
      // Always the complete array — omitting this field means "preserve" on
      // the backend, so a partial edit would otherwise silently no-op.
      aliases,
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
    {i18n.t('entityUi.vaultConflict', { path: conflict.sidecarKey })}
  </div>
{/if}

<form
  data-testid="entity-form"
  aria-label={i18n.t('entityUi.entityForm')}
  oninput={notifyDirty}
  onchange={notifyDirty}
  onsubmit={(e) => {
    e.preventDefault();
    handleSubmit();
  }}
>
  <FormField
    label={i18n.t('entityUi.name')}
    controlId="ef-name"
    errorText={nameError || (error?.field === 'name' ? error.message : undefined)}
    alertError={false}
  >
    <input id="ef-name" data-testid="entity-form-name" type="text" bind:value={name} />
  </FormField>

  <div class="field">
    <AliasField {aliases} onchange={(a) => (aliases = a)} />
  </div>

  <FormField label={i18n.t('entityUi.summary')} controlId="ef-summary">
    <input id="ef-summary" type="text" bind:value={summary} />
  </FormField>

  <FormField label={i18n.t('entityUi.notes')} controlId="ef-notes">
    <WikiLinkEditor
      id="ef-notes"
      bind:value={notes}
      entities={entityMap}
      rows={4}
      placeholder={i18n.t('entityUi.wikiLinkHint')}
    />
  </FormField>

  {#if kind === 'event'}
    <FormField label={i18n.t('entityUi.dateStart')} controlId="ef-date-start">
      <input id="ef-date-start" type="text" bind:value={dateStart} />
    </FormField>
    <FormField label={i18n.t('entityUi.dateEnd')} controlId="ef-date-end">
      <input id="ef-date-end" type="text" bind:value={dateEnd} />
    </FormField>
    <FormField label={i18n.t('entityUi.sequenceIndex')} controlId="ef-seq">
      <input id="ef-seq" type="number" bind:value={sequenceIndex} />
    </FormField>
    <FormField label={i18n.t('entityUi.era')} controlId="ef-era">
      <input id="ef-era" type="text" bind:value={era} />
    </FormField>
    <FormField label={i18n.t('entityUi.durationLabel')} controlId="ef-dur">
      <input id="ef-dur" type="text" bind:value={durationLabel} />
    </FormField>
    <div class="field checkbox">
      <label>
        <input type="checkbox" bind:checked={isOngoing} />
        {i18n.t('entityUi.ongoing')}
      </label>
    </div>
    <FormField label={i18n.t('entityUi.session')} controlId="ef-session">
      <select id="ef-session" bind:value={sessionId}>
        <option value="">{i18n.t('entityUi.none')}</option>
        {#each sessions as s (s.id)}
          <option value={s.id}>#{s.session_number}: {s.title}</option>
        {/each}
      </select>
    </FormField>
  {/if}

  {#if kind === 'player_character'}
    <FormField label={i18n.t('entityUi.playerName')} controlId="ef-player">
      <input id="ef-player" type="text" bind:value={playerName} />
    </FormField>
    <FormField label={i18n.t('entityUi.characterClass')} controlId="ef-class">
      <input id="ef-class" type="text" bind:value={characterClass} />
    </FormField>
    <FormField label={i18n.t('entityUi.characterLevel')} controlId="ef-level">
      <input id="ef-level" type="number" min="1" max="20" bind:value={characterLevel} />
    </FormField>
    <FormField label={i18n.t('entityUi.status')} controlId="ef-status">
      <select id="ef-status" bind:value={status}>
        <option value="">{i18n.t('entityUi.select')}</option>
        <option value="active">{i18n.t('entityUi.active')}</option>
        <option value="retired">{i18n.t('entityUi.retired')}</option>
        <option value="deceased">{i18n.t('entityUi.deceased')}</option>
        <option value="missing">{i18n.t('entityUi.missing')}</option>
        <option value="on_hiatus">{i18n.t('entityUi.onHiatus')}</option>
      </select>
    </FormField>
  {/if}

  {#if error && !error.field}
    <p class="form-error">{error.message}</p>
  {/if}

  <div class="actions">
    <Button testId="entity-form-submit" type="submit"
      >{node ? i18n.t('common.save') : i18n.t('entityUi.create')}</Button
    >
    <Button testId="entity-form-cancel" variant="ghost" onclick={() => oncancel?.()}
      >{i18n.t('common.cancel')}</Button
    >
  </div>

  {#if node?.id}
    <div class="relationships-section">
      <h3 class="relationships-heading">{i18n.t('entityUi.relationships')}</h3>
      {#if relations.length === 0}
        <p class="relationships-empty">{i18n.t('entityUi.noRelationships')}</p>
      {:else}
        <ul class="relationships-list">
          {#each relations as rel (rel.id + rel.direction + rel.rel_type)}
            <li class="rel-row">
              {#if onOpenEntity}
                <button
                  type="button"
                  class="rel-row-btn"
                  onclick={() => onOpenEntity(rel.id, rel.kind)}
                  aria-label={i18n.t('entityUi.openEntity', { name: rel.name })}
                >
                  <span class="rel-direction">{rel.direction === 'outbound' ? '→' : '←'}</span>
                  <span class="rel-name">{rel.name}</span>
                  <span class="rel-kind">{kindLabel(rel.kind)}</span>
                  <span class="rel-type">{rel.rel_type}</span>
                </button>
              {:else}
                <span class="rel-row-inner">
                  <span class="rel-direction">{rel.direction === 'outbound' ? '→' : '←'}</span>
                  <span class="rel-name">{rel.name}</span>
                  <span class="rel-kind">{kindLabel(rel.kind)}</span>
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
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  label {
    font-size: 0.85rem;
    color: var(--fg-3);
  }
  input,
  select {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--fg-1);
    padding: 6px 10px;
    font-size: 0.9rem;
  }
  .form-error {
    color: var(--danger);
    font-size: 0.8rem;
    margin: 0;
  }
  .conflict-banner {
    background: var(--danger-bg);
    color: var(--danger);
    border: 1px solid var(--danger);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 0.85rem;
    margin-bottom: 4px;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
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
