<script lang="ts">
  import { flushSync, onDestroy, tick, untrack } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import { getRuleEntries, updateRuleNotes, redoRuleEntry, type RuleEntry } from '../lib/commands';
  import { i18n } from '../lib/locale.svelte';
  import type { MessageKey } from '../lib/i18n/messages';
  import SaveStatus from './SaveStatus.svelte';
  import Button from './ui/Button.svelte';
  import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
  import { shouldAutoSaveAfterBlur } from '../lib/drafts/draft-focus-policy';
  import {
    ruleScope,
    statusOf,
    type DraftRecord,
    type DraftValue,
  } from '../lib/drafts/draft-state';
  import {
    forgetRuleRecovery,
    rememberedRuleRecovery,
    rememberRuleRecovery,
  } from '../lib/drafts/rule-note-presentation';

  interface RuleNoteDraftValue extends Readonly<Record<string, DraftValue>> {
    readonly notes: string;
  }

  interface PresentedRuleEntry {
    entry: RuleEntry;
    unavailable: boolean;
  }

  interface Props {
    campaignId?: string | null;
    collectionId: string;
    draftCoordinator?: DraftCoordinator;
  }

  let {
    campaignId = null,
    collectionId,
    draftCoordinator = new DraftCoordinator(),
  }: Props = $props();

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

  let entries = $state.raw<RuleEntry[]>([]);
  let loading = $state(false);
  let search = $state('');
  let expandedId = $state<string | null>(null);
  let redoOpenId = $state<string | null>(null);
  let objectionDraft = $state('');
  let error = $state<string | null>(null);
  let redoError = $state<string | null>(null);
  let panelElement = $state<HTMLDivElement>();
  let loadGeneration = 0;
  let destroyed = false;
  let activeProjectionPrefix: string | null = null;
  const projectionLeases = new SvelteMap<string, () => void>();

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
    if (!draftCoordinator.get(scope)) forgetRuleRecovery(draftCoordinator, scope);
  }

  function targetFor(ruleId: string): string {
    return `rule:${ruleId}`;
  }

  function valueFrom(entry: RuleEntry): RuleNoteDraftValue {
    return { notes: entry.notes ?? '' };
  }

  let scopePrefix = $derived(ruleScope(campaignId, collectionId, ''));
  let retainedDrafts = $derived(draftCoordinator.listByPrefix<RuleNoteDraftValue>(scopePrefix));

  let presentedEntries = $derived.by(() => {
    const rows: PresentedRuleEntry[] = entries.map((entry) => ({ entry, unavailable: false }));
    const listedIds = new Set(entries.map((entry) => entry.id));
    for (const draft of retainedDrafts) {
      if (statusOf(draft) === 'saved') continue;
      const ruleId = draft.target?.startsWith('rule:') ? draft.target.slice('rule:'.length) : '';
      if (!ruleId || listedIds.has(ruleId)) continue;
      const remembered = rememberedRuleRecovery(draftCoordinator, draft.scope);
      if (!remembered) continue;
      rows.push({
        entry: {
          id: remembered.ruleId,
          name: remembered.title,
          category: 'entry',
          body: '',
          notes: draft.value.notes === '' ? null : draft.value.notes,
          page_refs: [],
          stale: false,
        },
        unavailable: true,
      });
    }
    return rows;
  });

  async function load(requestCampaignId = campaignId, requestCollectionId = collectionId) {
    const requestGeneration = ++loadGeneration;
    const requestPrefix = ruleScope(requestCampaignId, requestCollectionId, '');
    const acknowledgmentsAtStart = new Map(
      draftCoordinator
        .listByPrefix<RuleNoteDraftValue>(requestPrefix)
        .map((draft) => [draft.scope, draft.lastAcknowledgedAttemptId] as const),
    );
    loading = true;
    try {
      const loaded = await getRuleEntries(requestCollectionId);
      if (
        destroyed ||
        requestGeneration !== loadGeneration ||
        requestCampaignId !== campaignId ||
        requestCollectionId !== collectionId
      ) {
        return;
      }

      for (const entry of loaded) {
        const scope = ruleScope(requestCampaignId, requestCollectionId, entry.id);
        const current = draftCoordinator.get<RuleNoteDraftValue>(scope);
        const startAcknowledgment = acknowledgmentsAtStart.get(scope);
        if (
          !current ||
          (startAcknowledgment !== undefined &&
            current.lastAcknowledgedAttemptId === startAcknowledgment)
        ) {
          draftCoordinator.open(scope, targetFor(entry.id), valueFrom(entry), 'authoritative-list');
          leaseProjection(scope);
        }
      }
      const loadedScopes = new Set(
        loaded.map(({ id }) => ruleScope(requestCampaignId, requestCollectionId, id)),
      );
      for (const scope of [...projectionLeases.keys()]) {
        if (scope.startsWith(requestPrefix) && !loadedScopes.has(scope)) releaseProjection(scope);
      }
      entries = loaded;
      error = null;
    } catch (e) {
      if (!destroyed && requestGeneration === loadGeneration) error = String(e);
    } finally {
      if (!destroyed && requestGeneration === loadGeneration) loading = false;
    }
  }

  function releaseCleanProjection(prefix: string): void {
    for (const scope of [...projectionLeases.keys()]) {
      if (scope.startsWith(prefix)) releaseProjection(scope);
    }
  }

  $effect(() => {
    const requestCampaignId = campaignId;
    const requestCollectionId = collectionId;
    const requestPrefix = ruleScope(requestCampaignId, requestCollectionId, '');
    untrack(() => {
      if (activeProjectionPrefix && activeProjectionPrefix !== requestPrefix) {
        releaseCleanProjection(activeProjectionPrefix);
      }
      for (const draft of draftCoordinator.listByPrefix<RuleNoteDraftValue>(requestPrefix)) {
        leaseProjection(draft.scope);
      }
      activeProjectionPrefix = requestPrefix;
      void load(requestCampaignId, requestCollectionId);
    });
  });

  onDestroy(() => {
    destroyed = true;
    loadGeneration += 1;
    for (const scope of [...projectionLeases.keys()]) releaseProjection(scope);
  });

  let filtered = $derived(
    presentedEntries.filter(({ entry }) => entry.name.toLowerCase().includes(search.toLowerCase())),
  );

  let grouped = $derived.by(() => {
    const groups: Record<string, PresentedRuleEntry[]> = {};
    for (const row of filtered) {
      (groups[row.entry.category] ??= []).push(row);
    }
    const orderedCats = [
      ...CATEGORY_ORDER,
      ...Object.keys(groups).filter((cat) => !CATEGORY_ORDER.includes(cat)),
    ];
    return orderedCats
      .map((cat) => ({ category: cat, entries: groups[cat] ?? [] }))
      .filter((g) => g.entries.length > 0);
  });

  function ensureDraft(entry: RuleEntry): DraftRecord<RuleNoteDraftValue> {
    const scope = ruleScope(campaignId, collectionId, entry.id);
    const draft =
      draftCoordinator.get<RuleNoteDraftValue>(scope) ??
      draftCoordinator.open(scope, targetFor(entry.id), valueFrom(entry), 'authoritative-list');
    leaseProjection(scope);
    return draft;
  }

  function toggleExpand(entry: RuleEntry) {
    if (expandedId === entry.id) {
      expandedId = null;
      return;
    }
    ensureDraft(entry);
    expandedId = entry.id;
    redoOpenId = null;
  }

  function reviseNotes(entry: RuleEntry, notes: string): void {
    const scope = ruleScope(campaignId, collectionId, entry.id);
    ensureDraft(entry);
    const revised = draftCoordinator.revise(scope, { notes });
    if (statusOf(revised) === 'saved') {
      forgetRuleRecovery(draftCoordinator, scope);
      return;
    }
    rememberRuleRecovery(draftCoordinator, scope, {
      ruleId: entry.id,
      title: entry.name,
      collectionId,
    });
  }

  async function saveDraft(entry: RuleEntry): Promise<void> {
    const saveCampaignId = campaignId;
    const saveCollectionId = collectionId;
    const saveRuleId = entry.id;
    const saveScope = ruleScope(saveCampaignId, saveCollectionId, saveRuleId);
    ensureDraft(entry);
    const releaseOperationLease = draftCoordinator.acquireLease(saveScope);
    try {
      await draftCoordinator.requestSave<RuleNoteDraftValue>(saveScope, async (attempt) => {
        try {
          const saved = await updateRuleNotes(
            saveRuleId,
            attempt.notes === '' ? null : attempt.notes,
          );
          if (saved.id !== saveRuleId) {
            throw new Error(`Rule-note save acknowledged the wrong target: ${saved.id}`);
          }
          return { notes: saved.notes ?? '' };
        } catch (saveError) {
          if (isStructuredError(saveError) && saveError.code === 'NOT_FOUND') {
            throw new Error(i18n.t('drafts.targetUnavailable'), { cause: saveError });
          }
          throw saveError;
        }
      });
      const savedDraft = draftCoordinator.get<RuleNoteDraftValue>(saveScope);
      if (!savedDraft || statusOf(savedDraft) === 'saved') {
        forgetRuleRecovery(draftCoordinator, saveScope);
        if (
          savedDraft &&
          (destroyed || saveCampaignId !== campaignId || saveCollectionId !== collectionId)
        ) {
          draftCoordinator.release(saveScope);
        }
      }
    } finally {
      releaseOperationLease();
    }
  }

  function elementFor<T extends HTMLElement>(selector: string, ruleId: string): T | undefined {
    return Array.from(panelElement?.querySelectorAll<T>(selector) ?? []).find(
      (element) => element.dataset.ruleId === ruleId,
    );
  }

  function shouldSaveFromBlur(entry: RuleEntry, event?: FocusEvent): boolean {
    const editingScope = elementFor<HTMLElement>('[data-rule-id]', entry.id);
    return shouldAutoSaveAfterBlur(editingScope, event?.relatedTarget ?? null, [
      editingScope?.querySelector('.rule-save-status'),
    ]);
  }

  async function handleNotesBlur(entry: RuleEntry, event?: FocusEvent): Promise<void> {
    if (
      draftCoordinator.isCloseDecisionActive() ||
      (draftCoordinator.isNavigationTransitionActive() && event?.relatedTarget === null) ||
      !shouldSaveFromBlur(entry, event)
    )
      return;
    const draft = draftCoordinator.get<RuleNoteDraftValue>(
      ruleScope(campaignId, collectionId, entry.id),
    );
    if (!draft || draft.error) return;
    try {
      await saveDraft(entry);
    } catch (saveError) {
      console.error('Failed to coordinate rule-note save:', saveError);
    }
  }

  function focusStableRulesTab(): void {
    panelElement
      ?.closest('.coll')
      ?.querySelector<HTMLButtonElement>('[role="tab"][aria-selected="true"]')
      ?.focus();
  }

  async function retrySave(entry: RuleEntry): Promise<void> {
    const retryCampaignId = campaignId;
    const retryCollectionId = collectionId;
    const retryScope = ruleScope(retryCampaignId, retryCollectionId, entry.id);
    const retryGeneration = loadGeneration;
    await saveDraft(entry);
    if (
      destroyed ||
      retryGeneration !== loadGeneration ||
      retryCampaignId !== campaignId ||
      retryCollectionId !== collectionId
    ) {
      return;
    }
    flushSync();
    const latest = draftCoordinator.get<RuleNoteDraftValue>(retryScope);
    if (latest && statusOf(latest) === 'failed') {
      panelElement?.querySelector<HTMLButtonElement>('.rule-save-status .failure button')?.focus();
      return;
    }
    const textarea = elementFor<HTMLTextAreaElement>('textarea[data-rule-notes]', entry.id);
    if (textarea) textarea.focus();
    else focusStableRulesTab();
  }

  async function discardDraft(entry: RuleEntry, unavailable: boolean): Promise<void> {
    const scope = ruleScope(campaignId, collectionId, entry.id);
    if (draftCoordinator.discard(scope) !== 'discarded') return;
    forgetRuleRecovery(draftCoordinator, scope);
    if (unavailable) releaseProjection(scope);
    await tick();
    if (unavailable) {
      focusStableRulesTab();
      return;
    }
    elementFor<HTMLButtonElement>('button[data-rule-header]', entry.id)?.focus();
  }

  function isStructuredError(value: unknown): value is { code: string; message?: string } {
    return (
      typeof value === 'object' &&
      value !== null &&
      'code' in value &&
      typeof value.code === 'string'
    );
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

<div class="rules-panel" bind:this={panelElement}>
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
  {:else if presentedEntries.length === 0}
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
          {#each group.entries as row (`${scopePrefix}${row.entry.id}`)}
            {@const entry = row.entry}
            {@const draft = draftCoordinator.get<RuleNoteDraftValue>(
              ruleScope(campaignId, collectionId, entry.id),
            )}
            {@const draftStatus = draft ? statusOf(draft) : 'saved'}
            {@const unavailableStatusId = `rule-unavailable-${campaignId ?? 'none'}-${collectionId}-${entry.id}`}
            {@const compactStatusId = `rule-save-${campaignId ?? 'none'}-${collectionId}-${entry.id}`}
            <li class="entry-item" data-rule-id={entry.id}>
              <button
                class="entry-name"
                aria-expanded={expandedId === entry.id}
                aria-describedby={expandedId !== entry.id && draftStatus !== 'saved'
                  ? compactStatusId
                  : row.unavailable
                    ? unavailableStatusId
                    : undefined}
                data-rule-header={entry.id}
                data-rule-id={entry.id}
                onclick={() => toggleExpand(entry)}
              >
                {entry.name}
                {#if entry.stale}
                  <span class="chip-stale">{i18n.t('entityUi.stale')}</span>
                {/if}
              </button>
              {#if expandedId !== entry.id && draftStatus !== 'saved'}
                <div id={compactStatusId} class="compact-rule-status">
                  <SaveStatus status={draftStatus} />
                </div>
              {/if}
              {#if row.unavailable}
                <span id={unavailableStatusId} class="sr-only">
                  {draftStatus === 'failed'
                    ? i18n.t('drafts.couldNotSave')
                    : i18n.t('drafts.unsavedChanges')}
                </span>
              {/if}
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
                    data-rule-notes={entry.id}
                    data-rule-id={entry.id}
                    value={draft?.value.notes ?? entry.notes ?? ''}
                    oninput={(e) => {
                      reviseNotes(entry, (e.target as HTMLTextAreaElement).value);
                    }}
                    onblur={(event) => handleNotesBlur(entry, event)}
                  ></textarea>

                  {#if draft}
                    <div class="rule-save-status">
                      <SaveStatus
                        status={draftStatus}
                        error={draft.error}
                        retainedThisSession={draftStatus !== 'saved'}
                        onRetry={() => retrySave(entry)}
                        onDiscard={draftStatus === 'saved'
                          ? undefined
                          : () => discardDraft(entry, row.unavailable)}
                        discardBlocked={draft.inFlight !== null}
                      />
                    </div>
                  {/if}

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
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
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
  .rule-save-status {
    margin-top: var(--s-2);
  }
  .compact-rule-status {
    padding: 0 var(--s-2) var(--s-2);
  }
  .compact-rule-status :global(.save-status) {
    font-size: 0.75rem;
  }
  .compact-rule-status :global(.failure) {
    padding: 0;
    border: 0;
    background: transparent;
  }
  .compact-rule-status :global(.message) {
    min-width: 0;
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
