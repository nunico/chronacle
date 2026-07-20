<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    getVaultPath,
    setVaultPath,
    vaultSyncNow,
    listVaultConflicts,
    type ReconcileReport,
    type VaultConflict,
  } from '../lib/commands';
  import { i18n } from '../lib/locale.svelte';
  import Button from './ui/Button.svelte';

  let path = $state<string | null>(null);
  let busy = $state(false);
  let report = $state<ReconcileReport | null>(null);
  let error = $state<string | null>(null);
  let conflicts = $state<VaultConflict[]>([]);

  $effect(() => {
    getVaultPath().then((p) => (path = p));
  });

  $effect(() => {
    loadConflicts();
  });

  // Swallows its own errors: the conflict list is supplementary, so a failure
  // to load it must never break the panel or surface as an unhandled rejection
  // (it is also called from a bare $effect and from disconnect(), neither of
  // which has a surrounding catch).
  async function loadConflicts() {
    try {
      const result = await listVaultConflicts();
      conflicts = Array.isArray(result) ? result : [];
    } catch {
      conflicts = [];
    }
  }

  async function choose() {
    const selected = await open({ directory: true });
    if (typeof selected !== 'string') return;
    error = null;
    try {
      await setVaultPath(selected);
      path = selected;
      report = null;
      await loadConflicts();
    } catch (e) {
      error = String(e);
    }
  }

  async function disconnect() {
    await setVaultPath(null);
    path = null;
    report = null;
    error = null;
    await loadConflicts();
  }

  async function syncNow() {
    busy = true;
    error = null;
    try {
      report = await vaultSyncNow();
      await loadConflicts();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="config-section">
  <h3>{i18n.t('entityUi.markdownVault')}</h3>
  <p class="muted">
    Sync entities, sessions, and collections to a folder of Markdown files — edit them in any text
    editor and Chronacle reconciles the changes back in. Text inside the marked compiled block is
    overwritten by Chronacle.
  </p>

  <div class="vault-path">
    {#if path}
      <span class="path-value">{path}</span>
    {:else}
      <span class="path-value muted">{i18n.t('entityUi.noVaultConfigured')}</span>
    {/if}
  </div>
  <p class="muted path-hint">{i18n.t('entityUi.vaultPathHint')}</p>

  <div class="actions">
    <Button variant="ghost" onclick={choose}>{i18n.t('entityUi.chooseFolder')}</Button>
    {#if path}
      <Button variant="ghost" onclick={disconnect}>{i18n.t('entityUi.disconnect')}</Button>
    {/if}
    <Button
      disabled={path === null || busy}
      loading={busy}
      loadingText={i18n.t('entityUi.syncing')}
      onclick={syncNow}>{i18n.t('entityUi.syncNow')}</Button
    >
  </div>

  {#if error}
    <div class="reindex-error">{i18n.t('entityUi.syncFailed', { error })}</div>
  {/if}

  {#if report}
    <div class="reindex-success">
      {report.exported} exported · {report.unchanged} unchanged · {report.applied} applied
      {#if report.conflicts > 0}
        · {report.conflicts} conflicts
      {/if}
      {#if report.resolved > 0}
        · {report.resolved} resolved
      {/if}
      {#if report.soft_deleted > 0}
        · {report.soft_deleted} soft-deleted
      {/if}
      {#if report.invalid > 0}
        · {report.invalid} invalid
      {/if}
      {#if report.failed > 0}
        · {report.failed} failed
      {/if}
    </div>
  {/if}

  {#if conflicts.length > 0}
    <div class="conflicts-section">
      <h4>
        {i18n.t('entityUi.conflicts', { count: conflicts.length })}
      </h4>
      <ul class="conflicts-list">
        {#each conflicts as c (c.id)}
          <li class="conflict-row">
            <span class="conflict-name">{c.name}</span>
            <span class="conflict-kind">{c.kind}</span>
            <span class="conflict-path">{c.key}</span>
            <span class="conflict-path">{c.sidecarKey}</span>
          </li>
        {/each}
      </ul>
      <p class="muted conflict-hint">
        Merge the two files in your vault, then delete the .conflict.md file — Chronacle applies
        your version on the next sync.
      </p>
    </div>
  {/if}
</section>

<style>
  .config-section {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 18px 18px 16px;
    margin-bottom: 16px;
    box-shadow: var(--shadow-card);
  }
  h3 {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--arcane-300);
    margin: 0 0 12px;
  }
  .muted {
    font-size: 13px;
    color: var(--fg-3);
    margin: 0 0 10px;
  }
  .vault-path {
    margin: 10px 0;
    font-size: 14px;
    color: var(--fg-2);
  }
  .path-value {
    word-break: break-all;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .reindex-error {
    margin-top: 10px;
    padding: 8px 12px;
    border-radius: var(--r-md);
    background: var(--danger-bg);
    color: var(--danger);
    font-size: 13px;
  }
  .reindex-success {
    margin-top: 10px;
    padding: 8px 12px;
    border-radius: var(--r-md);
    background: var(--success-bg);
    color: var(--success);
    font-size: 13px;
  }
  .path-hint {
    margin-top: -4px;
  }
  .conflicts-section {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .conflicts-section h4 {
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--danger);
    margin: 0 0 8px;
  }
  .conflicts-list {
    list-style: none;
    margin: 0 0 8px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .conflict-row {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
    padding: 6px 8px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--bg-panel-2, var(--bg-panel));
    font-size: 12px;
  }
  .conflict-name {
    font-weight: 700;
    color: var(--fg-1);
  }
  .conflict-kind {
    color: var(--fg-3);
    text-transform: uppercase;
    font-size: 11px;
  }
  .conflict-path {
    color: var(--fg-3);
    word-break: break-all;
    flex-basis: 100%;
  }
  .conflict-hint {
    margin: 0;
  }
</style>
