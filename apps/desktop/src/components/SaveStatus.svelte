<script lang="ts">
  import type { DraftStatus } from '../lib/drafts/draft-state';
  import { i18n } from '../lib/locale.svelte';
  import Button from './ui/Button.svelte';

  interface Props {
    status: DraftStatus;
    error?: string | null;
    retainedThisSession?: boolean;
    onRetry?: (event: MouseEvent) => void;
    onDiscard?: () => void;
    discardBlocked?: boolean;
  }

  let {
    status,
    error = null,
    retainedThisSession = false,
    onRetry,
    onDiscard,
    discardBlocked = false,
  }: Props = $props();

  let statusLabel = $derived(
    status === 'pending'
      ? i18n.t('drafts.unsavedChanges')
      : status === 'saving'
        ? i18n.t('drafts.saving')
        : i18n.t('drafts.saved'),
  );
  let discardBlockedReason = $derived(
    onDiscard && discardBlocked ? i18n.t('drafts.waitForSavingBeforeDiscard') : undefined,
  );
</script>

{#if status === 'failed'}
  <div class="save-status failure" role="alert">
    <div class="message">
      <strong>{i18n.t('drafts.couldNotSave')}</strong>
      {#if error}<span class="detail">{error}</span>{/if}
    </div>
    {#if onRetry || onDiscard}
      <div class="actions">
        {#if onRetry}
          <Button variant="secondary" onclick={onRetry}>{i18n.t('drafts.retry')}</Button>
        {/if}
        {#if onDiscard}
          <Button
            variant="ghost"
            onclick={onDiscard}
            disabled={discardBlocked}
            title={discardBlockedReason}>{i18n.t('drafts.discardChanges')}</Button
          >
        {/if}
      </div>
    {/if}
  </div>
{:else}
  <div class="save-status-row">
    <div class={['save-status', status]} role="status" aria-live="polite">
      <span class="state">{statusLabel}</span>
      {#if retainedThisSession}
        <span class="retention">{i18n.t('drafts.retainedThisSession')}</span>
      {/if}
      {#if discardBlockedReason}
        <span class="blocked-reason">{discardBlockedReason}</span>
      {/if}
    </div>
    {#if onDiscard}
      <div class="actions">
        <Button
          variant="ghost"
          onclick={onDiscard}
          disabled={discardBlocked}
          title={discardBlockedReason}>{i18n.t('drafts.discardChanges')}</Button
        >
      </div>
    {/if}
  </div>
{/if}

<style>
  .save-status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
    color: var(--fg-3);
    font: 500 0.8125rem/1.4 var(--font-sans);
  }

  .save-status-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
  }

  .state {
    font-weight: 650;
  }

  .pending .state,
  .saving .state {
    color: var(--warning);
  }

  .saved .state {
    color: var(--success);
  }

  .retention {
    color: var(--fg-3);
  }

  .failure {
    align-items: flex-start;
    padding: var(--s-3);
    border: 1px solid color-mix(in srgb, var(--danger) 45%, transparent);
    border-radius: var(--r-md);
    background: var(--danger-bg);
    color: var(--danger);
  }

  .message {
    display: flex;
    min-width: min(100%, 18rem);
    flex: 1;
    flex-direction: column;
    gap: var(--s-1);
  }

  .detail {
    color: var(--fg-2);
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
</style>
