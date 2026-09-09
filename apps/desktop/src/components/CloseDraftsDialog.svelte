<script lang="ts">
  import { i18n } from '../lib/locale.svelte';
  import type { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
  import Dialog from './ui/Dialog.svelte';

  interface Props {
    draftCoordinator: DraftCoordinator;
    oncancel: () => void;
    ondestroy: () => Promise<void> | void;
  }

  let { draftCoordinator, oncancel, ondestroy }: Props = $props();
  let closing = $state(false);
  let hasActiveWrites = $derived(draftCoordinator.hasActiveWrites());
  let hasAtRiskDrafts = $derived(draftCoordinator.atRiskCount() > 0);
  let safeToClose = $derived(!hasActiveWrites && !hasAtRiskDrafts);

  async function closeWindow(): Promise<void> {
    if (closing || hasActiveWrites) return;

    if (hasAtRiskDrafts && draftCoordinator.discardAll() !== 'discarded') return;

    closing = true;
    try {
      await ondestroy();
    } catch {
      // Native destruction can fail. Keep the prevented window open and allow
      // the GM to explicitly try Close again without leaking a rejection.
      closing = false;
    }
  }

  function handleCloseKey(event: KeyboardEvent): void {
    if (event.key !== 'Enter' && event.key !== ' ') return;
    event.preventDefault();
    void closeWindow();
  }
</script>

{#snippet closeBody()}
  <p>{i18n.t('drafts.retainedThisSession')}</p>
  {#if hasActiveWrites}
    <p id="window-close-status" class="close-status" role="status" aria-live="polite">
      {i18n.t('drafts.waitForSavingBeforeClose')}
    </p>
  {:else if safeToClose}
    <p id="window-close-status" class="close-status" role="status" aria-live="polite">
      {i18n.t('drafts.safeToClose')}
    </p>
  {/if}
{/snippet}

{#snippet closeActions()}
  <button
    type="button"
    class="dialog-button ghost"
    data-autofocus
    disabled={closing}
    onclick={oncancel}
  >
    {i18n.t('common.cancel')}
  </button>
  {#if safeToClose}
    <button
      type="button"
      class="dialog-button primary"
      disabled={closing}
      onclick={closeWindow}
      onkeydown={handleCloseKey}>{i18n.t('common.close')}</button
    >
  {:else}
    <button
      type="button"
      class="dialog-button danger"
      disabled={hasActiveWrites || closing}
      title={hasActiveWrites ? i18n.t('drafts.waitForSavingBeforeClose') : undefined}
      aria-describedby={hasActiveWrites ? 'window-close-status' : undefined}
      onclick={closeWindow}
      onkeydown={handleCloseKey}>{i18n.t('drafts.discardAndClose')}</button
    >
  {/if}
{/snippet}

<Dialog
  title={i18n.t('drafts.unsavedChanges')}
  body={closeBody}
  actions={closeActions}
  onclose={oncancel}
/>

<style>
  p {
    margin: 0;
  }

  .close-status {
    margin-top: var(--s-3);
    color: var(--rune-gold, var(--fg-1));
  }

  .dialog-button {
    min-height: 36px;
    padding: var(--s-2) var(--s-3);
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    font: 600 0.875rem/1.2 var(--font-sans);
    cursor: pointer;
  }

  .dialog-button.ghost {
    border-color: var(--line);
    background: transparent;
    color: var(--fg-2);
  }

  .dialog-button.primary {
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
  }

  .dialog-button.danger {
    border-color: color-mix(in srgb, var(--danger) 55%, transparent);
    background: var(--danger-bg);
    color: var(--danger);
  }

  .dialog-button:hover:not(:disabled) {
    border-color: var(--line-glow);
  }

  .dialog-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }
</style>
