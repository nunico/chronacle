<script lang="ts">
  import { getRuleEntries, updateRuleNotes, redoRuleEntry, type RuleEntry } from '../lib/commands';
  import { i18n } from '../lib/locale.svelte';
  import type { MessageKey } from '../lib/i18n/messages';
  import Button from './ui/Button.svelte';

  interface Props {
    collectionId: string;
  }

  let { collectionId }: Props = $props();

  const CATEGORY_ORDER = [
    'mechanic',
    'ability',
    'state',
    'procedure',
    'resource',
    'statistic',
    'entry',
  ];

  const CATEGORY_LABEL: Record<string, MessageKey> = {
    mechanic: 'entityUi.categoryMechanic',
    ability: 'entityUi.categoryAbility',
    state: 'entityUi.categoryState',
    procedure: 'entityUi.categoryProcedure',
    resource: 'entityUi.categoryResource',
    statistic: 'entityUi.categoryStatistic',
    entry: 'entityUi.categoryEntry',
  };

  let entries = $state<RuleEntry[]>([]);
  let loading = $state(false);
  let search = $state('');
  let expandedId = $state<string | null>(null);
  let notesDraft = $state<Record<string, string>>({});
  let redoOpenId = $state<string | null>(null);
  let objectionDraft = $state('');
  let error = $state<string | null>(null);
  let redoError = $state<string | null>(null);

  async function load() {
    loading = true;
    try {
      entries = await getRuleEntries(collectionId);
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void collectionId;
    load();
  });

  let filtered = $derived(
    entries.filter((e) => e.name.toLowerCase().includes(search.toLowerCase())),
  );

  let grouped = $derived.by(() => {
    const groups: Record<string, RuleEntry[]> = {};
    for (const e of filtered) {
      (groups[e.category] ??= []).push(e);
    }
    const orderedCats = [
      ...CATEGORY_ORDER,
      ...Object.keys(groups).filter((cat) => !CATEGORY_ORDER.includes(cat)),
    ];
    return orderedCats
      .map((cat) => ({ category: cat, entries: groups[cat] ?? [] }))
      .filter((g) => g.entries.length > 0);
  });

  function toggleExpand(entry: RuleEntry) {
    if (expandedId === entry.id) {
      expandedId = null;
      return;
    }
    expandedId = entry.id;
    if (!(entry.id in notesDraft)) {
      notesDraft[entry.id] = entry.notes ?? '';
    }
    redoOpenId = null;
  }

  async function handleNotesBlur(entry: RuleEntry) {
    const value = notesDraft[entry.id] ?? '';
    if (value === (entry.notes ?? '')) return;
    const saved = value.length > 0 ? value : null;
    await updateRuleNotes(entry.id, saved);
    entry.notes = saved;
  }

  function openRedo(entry: RuleEntry) {
    redoOpenId = entry.id;
    objectionDraft = '';
    redoError = null;
  }

  async function submitRedo(entry: RuleEntry) {
    try {
      await redoRuleEntry(entry.id, objectionDraft);
      redoOpenId = null;
      objectionDraft = '';
      redoError = null;
      await load();
    } catch (e) {
      redoError = String(e);
    }
  }
</script>

<div class="rules-panel">
  <div class="search-row">
    <input
      class="search-input"
      type="text"
      aria-label={i18n.t('entityUi.searchRules')}
      placeholder={i18n.t('entityUi.searchRules')}
      bind:value={search}
    />
  </div>

  {#if loading}
    <p class="muted">{i18n.t('entityUi.loadingRules')}</p>
  {:else if error}
    <p class="error" role="alert">{i18n.t('entityUi.loadRulesFailed', { error })}</p>
  {:else if entries.length === 0}
    <p class="muted">{i18n.t('entityUi.noRules')}</p>
  {:else if grouped.length === 0}
    <p class="muted">{i18n.t('entityUi.noMatchingRules')}</p>
  {:else}
    {#each grouped as group (group.category)}
      <div class="category-group">
        <h3>
          {CATEGORY_LABEL[group.category] ? i18n.t(CATEGORY_LABEL[group.category]) : group.category}
        </h3>
        <ul class="entry-list">
          {#each group.entries as entry (entry.id)}
            <li class="entry-item">
              <button
                class="entry-name"
                aria-expanded={expandedId === entry.id}
                onclick={() => toggleExpand(entry)}
              >
                {entry.name}
                {#if entry.stale}
                  <span class="chip-stale">{i18n.t('entityUi.stale')}</span>
                {/if}
              </button>
              {#if expandedId === entry.id}
                <div class="entry-body">
                  <p class="body">{entry.body}</p>
                  {#if entry.page_refs.length > 0}
                    <p class="page-refs">
                      {#each entry.page_refs as ref, i (i)}
                        {i > 0 ? ' · ' : ''}{ref.source_name}
                        {i18n.t('entityUi.pageAbbreviation')}{ref.page_start}{ref.page_start ===
                        ref.page_end
                          ? ''
                          : `-${ref.page_end}`}
                      {/each}
                    </p>
                  {/if}

                  <label class="notes-label" for="notes-{entry.id}"
                    >{i18n.t('entityUi.tableNotes')}</label
                  >
                  <textarea
                    id="notes-{entry.id}"
                    aria-label={i18n.t('entityUi.tableNotes')}
                    value={notesDraft[entry.id] ?? entry.notes ?? ''}
                    oninput={(e) => {
                      notesDraft[entry.id] = (e.target as HTMLTextAreaElement).value;
                    }}
                    onblur={() => handleNotesBlur(entry)}
                  ></textarea>

                  {#if redoOpenId === entry.id}
                    <div class="redo-dialog">
                      <label class="objection-label" for="objection-{entry.id}"
                        >{i18n.t('entityUi.objection')}</label
                      >
                      <textarea
                        id="objection-{entry.id}"
                        aria-label={i18n.t('entityUi.objection')}
                        bind:value={objectionDraft}
                      ></textarea>
                      {#if redoError}
                        <p class="error" role="alert">
                          {i18n.t('entityUi.redoFailed', { error: redoError })}
                        </p>
                      {/if}
                      <div class="redo-actions">
                        <Button onclick={() => submitRedo(entry)}
                          >{i18n.t('entityUi.submit')}</Button
                        >
                        <Button
                          variant="ghost"
                          onclick={() => {
                            redoOpenId = null;
                          }}>{i18n.t('common.cancel')}</Button
                        >
                      </div>
                    </div>
                  {:else}
                    <Button variant="ghost" onclick={() => openRedo(entry)}
                      >{i18n.t('entityUi.redoWithObjections')}</Button
                    >
                  {/if}
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      </div>
    {/each}
  {/if}
</div>

<style>
  .rules-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .search-row {
    display: flex;
  }
  .search-input {
    flex: 1;
    padding: 6px 10px;
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--fg-1);
    font-size: 0.85rem;
  }
  .category-group h3 {
    margin: 0 0 6px;
    font-size: 0.9rem;
    color: var(--fg-2);
    text-transform: capitalize;
  }
  .entry-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .entry-item {
    border-bottom: 1px solid var(--line);
  }
  .entry-name {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: var(--fg-1);
    padding: 8px 4px;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .chip-stale {
    background: var(--danger);
    color: var(--bg-abyss);
    border-radius: 10px;
    padding: 2px 8px;
    font-size: 0.7rem;
    font-weight: 600;
  }
  .entry-body {
    padding: 4px 8px 12px;
  }
  .body {
    white-space: pre-wrap;
    font-size: 0.85rem;
    color: var(--fg-2);
    line-height: 1.5;
    margin: 0 0 6px;
  }
  .page-refs {
    font-size: 0.75rem;
    color: var(--fg-3);
    margin: 0 0 10px;
  }
  .notes-label,
  .objection-label {
    display: block;
    font-size: 0.8rem;
    color: var(--fg-3);
    margin-bottom: 4px;
  }
  textarea {
    width: 100%;
    min-height: 60px;
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--fg-1);
    font-size: 0.85rem;
    padding: 6px 8px;
    resize: vertical;
    box-sizing: border-box;
  }
  .redo-dialog {
    margin-top: 8px;
  }
  .redo-actions {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }
  .muted {
    color: var(--fg-3);
    font-size: 0.85rem;
    padding: 16px;
  }
  .error {
    color: var(--danger);
    font-size: 0.85rem;
    padding: 16px;
  }
</style>
