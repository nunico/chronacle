<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getProposals,
    acceptProposal,
    rejectProposal,
    type CodexProposal,
  } from '../lib/commands';

  interface Props {
    onCountsChanged?: () => void;
  }
  let { onCountsChanged }: Props = $props();

  let tab = $state<'proposals'>('proposals');
  let proposals = $state<CodexProposal[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null); // proposal id being resolved

  const KIND_LABELS: Record<string, string> = {
    entity_article_update: 'Article update',
    entity_notes_update: 'Notes suggestion',
    rule_entry_update: 'Rule update',
    new_entity: 'New entity',
    new_rule_entry: 'New rule',
  };

  async function refresh() {
    loading = true;
    error = null;
    try {
      proposals = await getProposals('pending');
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function resolve(id: string, action: 'accept' | 'reject') {
    busy = id;
    try {
      if (action === 'accept') await acceptProposal(id);
      else await rejectProposal(id);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  onMount(() => void refresh());
</script>

<div class="maintenance">
  <h2 class="heading">Maintenance</h2>

  <div class="toolbar" role="tablist" aria-label="Maintenance sections">
    <button
      role="tab"
      aria-selected={tab === 'proposals'}
      class:active={tab === 'proposals'}
      onclick={() => (tab = 'proposals')}
    >
      Proposals
    </button>
  </div>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if proposals.length === 0}
    <p class="muted">No pending proposals</p>
  {:else}
    <ul class="proposal-list">
      {#each proposals as p (p.id)}
        <li class="proposal-card">
          <div class="proposal-head">
            <span class="chip-kind">{KIND_LABELS[p.kind] ?? p.kind}</span>
            <span class="target-name">{p.target_name ?? p.payload.name ?? '(new)'}</span>
            <span class="chip-origin">{p.origin_kind}</span>
          </div>
          <p class="rationale">{p.payload.rationale}</p>
          <div class="diff">
            <div class="diff-pane">
              <h4>Current</h4>
              <pre class="diff-text">{p.current_text ?? '(none)'}</pre>
            </div>
            <div class="diff-pane">
              <h4>Proposed</h4>
              <pre class="diff-text">{p.payload.proposed_text}</pre>
            </div>
          </div>
          <div class="proposal-actions">
            <button
              type="button"
              aria-label="Accept proposal"
              disabled={busy === p.id}
              onclick={() => resolve(p.id, 'accept')}
            >
              Accept
            </button>
            <button
              type="button"
              class="btn-ghost"
              aria-label="Reject proposal"
              disabled={busy === p.id}
              onclick={() => resolve(p.id, 'reject')}
            >
              Reject
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .maintenance {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .heading {
    font-family: var(--font-display);
    color: var(--fg-1);
    margin: 0;
  }
  .toolbar {
    display: flex;
    gap: 6px;
  }
  .toolbar button {
    padding: 6px 14px;
    border-radius: var(--r-md);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    color: var(--fg-2);
    cursor: pointer;
  }
  .toolbar button.active {
    color: var(--fg-1);
    border-color: var(--violet-400);
  }
  .proposal-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .proposal-card {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 12px 14px;
  }
  .proposal-head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .chip-kind,
  .chip-origin {
    font-size: 0.7rem;
    text-transform: uppercase;
    color: var(--fg-3);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 2px 8px;
  }
  .target-name {
    color: var(--fg-1);
    font-weight: 600;
  }
  .rationale {
    color: var(--fg-2);
    font-size: 0.85rem;
    margin: 0 0 10px;
  }
  .diff {
    display: flex;
    gap: 12px;
    margin-bottom: 10px;
  }
  .diff-pane {
    flex: 1;
    min-width: 0;
  }
  .diff-pane h4 {
    margin: 0 0 4px;
    font-size: 0.75rem;
    color: var(--fg-3);
    text-transform: uppercase;
  }
  .diff-text {
    white-space: pre-wrap;
    font-size: 0.8rem;
    color: var(--fg-2);
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 8px;
    margin: 0;
  }
  .proposal-actions {
    display: flex;
    gap: 8px;
  }
  .proposal-actions button {
    padding: 6px 14px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--violet-400);
    color: var(--bg-abyss);
    cursor: pointer;
    font-size: 0.85rem;
  }
  .proposal-actions button.btn-ghost {
    background: transparent;
    color: var(--fg-3);
  }
  .proposal-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .muted {
    color: var(--fg-3);
    font-size: 0.85rem;
  }
  .error {
    padding: 8px 12px;
    background: var(--danger-bg);
    color: var(--danger);
    border: 1px solid rgba(242, 103, 75, 0.4);
    border-radius: var(--r-md);
    font-size: 13px;
  }
</style>
