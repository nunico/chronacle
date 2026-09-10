<script module lang="ts">
  import type { Session as SessionRecord } from '../lib/commands';
  import type { DraftValue } from '../lib/drafts/draft-state';

  /** Normalized, JSON-like value retained for one campaign session. */
  export interface SessionDraftValue extends Readonly<Record<string, DraftValue>> {
    readonly sessionNumber: number;
    readonly title: string;
    readonly datePlayed: string;
    readonly notes: string;
  }

  export function sessionDraftValue(session: SessionRecord): SessionDraftValue {
    return {
      sessionNumber: session.session_number,
      title: session.title,
      datePlayed: session.date_played,
      notes: session.notes,
    };
  }

  export function sessionWithDraft(
    session: SessionRecord,
    draft: SessionDraftValue,
  ): SessionRecord {
    return {
      ...session,
      session_number: draft.sessionNumber,
      title: draft.title,
      date_played: draft.datePlayed,
      notes: draft.notes,
    };
  }
</script>

<script lang="ts">
  import { flushSync, tick, untrack } from 'svelte';
  import {
    updateSession,
    deleteSession,
    getSessionEntities,
    type Session,
    type GraphNode,
    type EntityError,
  } from '../lib/commands';
  import WikiText from './WikiText.svelte';
  import WikiLinkEditor from './WikiLinkEditor.svelte';
  import { i18n } from '../lib/locale.svelte';
  import Button from './ui/Button.svelte';
  import { formatDate } from '../lib/locale.svelte';
  import SaveStatus from './SaveStatus.svelte';
  import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
  import { shouldAutoSaveAfterBlur } from '../lib/drafts/draft-focus-policy';
  import { sessionScope, statusOf } from '../lib/drafts/draft-state';

  interface Props {
    session: Session;
    entityMap: Map<string, { id: string; kind: string }>;
    onUpdate: (session: Session) => void;
    onDelete: (id: string) => void;
    onDiscardUnavailable?: (id: string) => void | Promise<void>;
    campaignId?: string;
    draftCoordinator?: DraftCoordinator;
  }

  let {
    session,
    entityMap,
    onUpdate,
    onDelete,
    onDiscardUnavailable,
    campaignId,
    draftCoordinator = new DraftCoordinator(),
  }: Props = $props();

  let expanded = $state(false);
  let linkedEntities = $state<GraphNode[]>([]);
  let loadingEntities = $state(false);
  let entitiesLoaded = $state(false);
  let rowElement = $state<HTMLDivElement>();
  let headerButton = $state<HTMLButtonElement>();
  let deletionPending = $state(false);

  let resolvedCampaignId = $derived.by(() => {
    const resolved = campaignId ?? session.campaign_id;
    if (!resolved) throw new Error('A session draft requires a campaign identity');
    return resolved;
  });
  let scope = $derived(sessionScope(resolvedCampaignId, session.id));
  let target = $derived(`session:${session.id}`);
  let draft = $derived(draftCoordinator.get<SessionDraftValue>(scope));
  let value = $derived(draft?.value ?? sessionDraftValue(session));
  let status = $derived(draft ? statusOf(draft) : 'saved');
  let activeSaveBlocksDelete = $derived(draft?.inFlight != null);
  let deleteBlocked = $derived(deletionPending || activeSaveBlocksDelete);
  let deleteHelpId = $derived(`delete-help-${resolvedCampaignId}-${session.id}`);
  let openedCoordinator: DraftCoordinator | undefined;
  let openedScope = '';
  let openedTarget = '';
  let openedBaseline = '';

  $effect(() => {
    const coordinator = draftCoordinator;
    const currentScope = scope;
    const currentTarget = target;
    const baseline = sessionDraftValue(session);
    const baselineKey = JSON.stringify(baseline);
    if (
      coordinator === openedCoordinator &&
      currentScope === openedScope &&
      currentTarget === openedTarget &&
      baselineKey === openedBaseline
    ) {
      return;
    }
    openedCoordinator = coordinator;
    openedScope = currentScope;
    openedTarget = currentTarget;
    openedBaseline = baselineKey;
    untrack(() => coordinator.open(currentScope, currentTarget, baseline));
  });

  function revise(next: Partial<SessionDraftValue>) {
    if (deletionPending) return;
    const current = draftCoordinator.get<SessionDraftValue>(scope);
    if (!current) return;
    draftCoordinator.revise(scope, {
      sessionNumber: next.sessionNumber ?? current.value.sessionNumber,
      title: next.title ?? current.value.title,
      datePlayed: next.datePlayed ?? current.value.datePlayed,
      notes: next.notes ?? current.value.notes,
    });
  }

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

  async function saveDraft() {
    if (deletionPending) return;
    const saveScope = scope;
    const saveSessionId = session.id;
    await draftCoordinator.requestSave<SessionDraftValue>(saveScope, async (attempt) => {
      try {
        const updated = await updateSession(saveSessionId, {
          sessionNumber: attempt.sessionNumber,
          title: attempt.title,
          datePlayed: attempt.datePlayed,
          notes: attempt.notes,
        });
        if (updated.id !== saveSessionId) {
          throw new Error(`Session save acknowledged the wrong target: ${updated.id}`);
        }
        onUpdate(updated);
        return sessionDraftValue(updated);
      } catch (error) {
        if (isEntityError(error) && error.code === 'NOT_FOUND') {
          throw new Error(i18n.t('drafts.targetUnavailable'), { cause: error });
        }
        throw error;
      }
    });
  }

  function shouldSaveFromBlur(event?: FocusEvent): boolean {
    return shouldAutoSaveAfterBlur(rowElement, event?.relatedTarget ?? null, [
      rowElement?.querySelector('.session-save-status'),
      rowElement?.querySelector('.session-actions'),
    ]);
  }

  async function saveFromBlur(event?: FocusEvent) {
    if (deletionPending || draftCoordinator.isCloseDecisionActive() || !shouldSaveFromBlur(event))
      return;
    const current = draftCoordinator.get<SessionDraftValue>(scope);
    if (current?.error) return;
    await saveDraft();
  }

  async function retrySave() {
    if (deletionPending) return;
    await saveDraft();
    flushSync();
    const latest = draftCoordinator.get<SessionDraftValue>(scope);
    if (latest && statusOf(latest) === 'failed') {
      rowElement?.querySelector<HTMLButtonElement>('.save-status.failure button')?.focus();
      return;
    }
    const mountedTitle = rowElement?.querySelector<HTMLInputElement>(
      '.session-body input[type="text"]',
    );
    (mountedTitle ?? headerButton)?.focus();
  }

  async function discardDraft() {
    if (deletionPending) return;
    const releaseUnavailable = onDiscardUnavailable;
    if (draftCoordinator.discard(scope) !== 'discarded') return;
    await releaseUnavailable?.(session.id);
    await tick();
    if (headerButton?.isConnected) headerButton.focus();
  }

  async function handleDelete() {
    if (deleteBlocked) return;
    if (!confirm(i18n.t('dialog.confirmDelete'))) return;
    const deleteScope = scope;
    const deleteSessionId = session.id;
    deletionPending = true;
    try {
      await deleteSession(deleteSessionId);
      const cleanup = draftCoordinator.removeAfterDelete(deleteScope);
      if (cleanup !== 'removed') {
        deletionPending = false;
        console.error(`Failed to remove deleted session draft: ${cleanup}`);
        return;
      }
      onDelete(deleteSessionId);
    } catch (e) {
      deletionPending = false;
      console.error('Failed to delete session:', e);
    }
  }

  function isEntityError(error: unknown): error is EntityError {
    return (
      typeof error === 'object' &&
      error !== null &&
      'code' in error &&
      typeof error.code === 'string'
    );
  }
</script>

<div class="session-row" class:expanded bind:this={rowElement} aria-busy={deletionPending}>
  <!-- Collapsed header — always visible, click to toggle -->
  <button
    type="button"
    class="session-header"
    onclick={toggleExpand}
    aria-expanded={expanded}
    bind:this={headerButton}
  >
    <span class="session-number">#{value.sessionNumber}</span>
    <span class="session-title">{value.title}</span>
    {#if draft && draft.value.title !== draft.baseline.title}
      <span class="saved-title">{i18n.t('drafts.saved')}: {draft.baseline.title}</span>
    {/if}
    <span class="session-date">{formatDate(value.datePlayed)}</span>
    {#if linkedEntities.length > 0}
      <span class="session-events"
        >{i18n.t('entityUi.events', { count: linkedEntities.length })}</span
      >
    {/if}
    <span class="chevron" class:rotated={expanded}>›</span>
  </button>

  <fieldset class="session-controls" disabled={deletionPending}>
    {#if draft}
      <div class="session-save-status">
        <SaveStatus
          {status}
          error={draft.error}
          retainedThisSession={status !== 'saved'}
          onRetry={retrySave}
          onDiscard={status === 'saved' ? undefined : discardDraft}
          discardBlocked={draft.inFlight !== null}
        />
      </div>
    {/if}

    {#if expanded}
      <div class="session-body">
        <div class="field-row">
          <label for="title-{session.id}" class="field-label">{i18n.t('entityUi.name')}</label>
          <input
            id="title-{session.id}"
            class="field-input"
            type="text"
            bind:value={() => value.title, (title) => revise({ title })}
            onblur={saveFromBlur}
          />
        </div>

        <div class="field-row">
          <label for="date-{session.id}" class="field-label">{i18n.t('entityUi.datePlayed')}</label>
          <input
            id="date-{session.id}"
            class="field-input"
            type="date"
            bind:value={() => value.datePlayed, (datePlayed) => revise({ datePlayed })}
            onblur={saveFromBlur}
          />
        </div>

        <div class="field-col">
          <label for="notes-{session.id}" class="field-label">{i18n.t('entityUi.notes')}</label>
          <WikiLinkEditor
            id="notes-{session.id}"
            bind:value={() => value.notes, (notes) => revise({ notes })}
            entities={entityMap}
            onblur={saveFromBlur}
            rows={6}
            placeholder={i18n.t('entityUi.sessionNotesPlaceholder')}
          />
          {#if value.notes}
            <div class="wiki-preview">
              <WikiText text={value.notes} entities={entityMap} />
            </div>
          {/if}
        </div>

        {#if loadingEntities}
          <p class="muted">{i18n.t('entityUi.loadingLinkedEvents')}</p>
        {:else if linkedEntities.length > 0}
          <div class="linked-entities">
            {#each linkedEntities as e (e.id)}
              <span class="entity-badge" title={e.kind}>{e.name}</span>
            {/each}
          </div>
        {/if}

        <div class="session-actions">
          {#if activeSaveBlocksDelete}
            <span id={deleteHelpId} class="delete-help">
              {i18n.t('drafts.waitForSavingBeforeDiscard')}
            </span>
          {/if}
          <Button
            variant="danger"
            onclick={handleDelete}
            disabled={deleteBlocked}
            ariaDescribedby={activeSaveBlocksDelete ? deleteHelpId : undefined}
            >{i18n.t('common.delete')}</Button
          >
        </div>
      </div>
    {/if}
  </fieldset>
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

  .session-save-status {
    padding: 8px 16px;
    border-top: 1px solid var(--line);
  }

  .session-controls {
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .saved-title {
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

  .delete-help {
    color: var(--fg-3);
    font-size: 12px;
  }
</style>
