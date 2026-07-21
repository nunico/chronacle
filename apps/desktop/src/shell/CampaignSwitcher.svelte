<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import type { Campaign } from '../lib/commands';
  import { i18n } from '../lib/locale.svelte';

  let {
    campaigns,
    activeCampaignId,
    onSelect,
    onManage,
    onClose,
  }: {
    campaigns: Campaign[];
    activeCampaignId: string | null;
    onSelect: (id: string) => void;
    onManage: () => void;
    onClose: () => void;
  } = $props();

  let popoverEl = $state<HTMLDivElement | undefined>(undefined);

  $effect(() => {
    popoverEl?.querySelector<HTMLButtonElement>('.row')?.focus();
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  function onBackdropClick(e: MouseEvent) {
    // Close only when clicking the backdrop itself, not the popover content.
    if (e.target === e.currentTarget) onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="backdrop"
  role="presentation"
  onclick={onBackdropClick}
  onkeydown={() => {
    /* empty */
  }}
></div>

<div
  class="popover"
  role="dialog"
  aria-label={i18n.t('shell.switchCampaign')}
  bind:this={popoverEl}
>
  {#if campaigns.length === 0}
    <div class="empty">
      {i18n.t('campaign.noCampaignYet')} — {i18n.t('shell.createCampaignHint')}.
    </div>
  {/if}
  {#each campaigns as c (c.id)}
    <button
      class="row"
      class:active={activeCampaignId === c.id}
      onclick={() => {
        onSelect(c.id);
        onClose();
      }}
    >
      <span class="gem-dot"></span>
      <span class="nm">{c.name}</span>
      <span class="mt">{c.system ?? i18n.t('campaign.systemDash')}</span>
      {#if activeCampaignId === c.id}
        <Icon name="check" size={14} />
      {/if}
    </button>
  {/each}
  <div class="sep"></div>
  <button
    class="row manage"
    onclick={() => {
      onManage();
      onClose();
    }}
  >
    <Icon name="settings" size={14} />
    <span class="nm">{i18n.t('campaign.manageCampaigns')}…</span>
  </button>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
  }
  .popover {
    position: absolute;
    top: 68px;
    left: 12px;
    width: 232px;
    z-index: 100;
    padding: 6px;
    background: rgba(16, 19, 42, 0.8);
    backdrop-filter: blur(14px);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-3);
  }
  .row {
    width: 100%;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 13px;
    text-align: left;
    overflow-wrap: anywhere;
    background: none;
    border: 0;
  }
  .row:hover {
    background: rgba(124, 148, 255, 0.07);
    color: var(--fg-1);
  }
  .row.active {
    color: var(--fg-1);
  }
  .row.active .gem-dot {
    box-shadow: var(--glow-arcane);
  }
  .gem-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--grad-gem);
    flex: none;
  }
  .row .nm {
    flex: 1;
    min-width: 0;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row .mt {
    min-width: 0;
    max-width: 36%;
    overflow: hidden;
    overflow-wrap: anywhere;
    word-break: break-word;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-3);
  }
  .sep {
    height: 1px;
    background: var(--line-faint);
    margin: 6px 4px;
  }
  .manage {
    color: var(--arcane-300);
  }
  .empty {
    padding: 12px 10px;
    font-size: 12.5px;
    color: var(--fg-3);
    font-family: var(--font-sans);
    text-align: center;
  }
</style>
