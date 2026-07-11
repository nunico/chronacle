<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { getVaultPath, setVaultPath, vaultSyncNow, type ReconcileReport } from '../lib/commands';

  let path = $state<string | null>(null);
  let busy = $state(false);
  let report = $state<ReconcileReport | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    getVaultPath().then((p) => (path = p));
  });

  async function choose() {
    const selected = await open({ directory: true });
    if (typeof selected !== 'string') return;
    error = null;
    try {
      await setVaultPath(selected);
      path = selected;
      report = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function disconnect() {
    await setVaultPath(null);
    path = null;
    report = null;
    error = null;
  }

  async function syncNow() {
    busy = true;
    error = null;
    try {
      report = await vaultSyncNow();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="config-section">
  <h3>Markdown vault</h3>
  <p class="muted">
    Sync entities, sessions, and collections to a folder of Markdown files —
    edit them in any text editor and Chronacle reconciles the changes back in.
  </p>

  <div class="vault-path">
    {#if path}
      <span class="path-value">{path}</span>
    {:else}
      <span class="path-value muted">No vault configured</span>
    {/if}
  </div>

  <div class="actions">
    <button class="small-btn" onclick={choose}>Choose folder…</button>
    {#if path}
      <button class="small-btn" onclick={disconnect}>Disconnect</button>
    {/if}
    <button class="small-btn primary" disabled={path === null || busy} onclick={syncNow}>
      {busy ? 'Syncing…' : 'Sync now'}
    </button>
  </div>

  {#if error}
    <div class="reindex-error">Sync failed: {error}</div>
  {/if}

  {#if report}
    <div class="reindex-success">
      {report.exported} exported · {report.unchanged} unchanged
      {#if report.failed > 0}
        · {report.failed} failed
      {/if}
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
  .small-btn {
    background: none;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-2);
    cursor: pointer;
    font-size: 12px;
    padding: 4px 9px;
    font-family: var(--font-sans);
  }
  .small-btn:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .small-btn.primary {
    background: var(--grad-arcane);
    border-color: transparent;
    color: var(--fg-on-accent);
  }
  .small-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
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
</style>
