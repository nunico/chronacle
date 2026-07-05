<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    getCampaigns,
    getCollections,
    createCollection,
    uploadSource,
    getMruCollectionId,
    setMruCollectionId,
    getEmbeddingModelMismatch,
    reindexAllSources,
    getEntityCounts,
    getSessions,
    getMaintenanceCounts,
    type Campaign,
    type Collection,
    type EmbeddingModelMismatch,
  } from '../lib/commands';
  import { onEmbeddingModelMismatch } from '../lib/events';
  import { modalBehavior } from '../lib/actions/modal';
  import { showToast } from '../lib/toast.svelte';
  import Toast from '../components/Toast.svelte';
  import CampaignRail, { type View } from './CampaignRail.svelte';
  import CampaignSwitcher from './CampaignSwitcher.svelte';
  import Topbar from './Topbar.svelte';
  import OracleView from '../views/OracleView.svelte';
  import CampaignView from '../views/CampaignView.svelte';
  import NotesView from '../views/NotesView.svelte';
  import SettingsView from '../views/SettingsView.svelte';
  import UploadProgress from '../UploadProgress.svelte';
  import EntityManager from '../components/EntityManager.svelte';
  import EntityGraph from '../components/EntityGraph.svelte';
  import SessionLogView from '../views/SessionLogView.svelte';
  import TimelineView from '../views/TimelineView.svelte';
  import MaintenanceView from '../views/MaintenanceView.svelte';
  import { findCategory, type NoteCategoryId } from './note-categories';
  import type { EntityKind } from '../lib/commands';
  import {
    resolveNavChord,
    isEditableTarget,
    SHORTCUT_HELP,
    type NavTarget,
  } from '../lib/shortcuts';

  const ENTITY_KIND_MAP: Partial<Record<NoteCategoryId, EntityKind>> = {
    npcs: 'npc',
    locations: 'location',
    factions: 'faction',
    creatures: 'creature',
    items: 'item',
    events: 'event',
    player_characters: 'player_character',
    misc: 'misc',
  };

  const KIND_TO_CATEGORY = Object.fromEntries(
    Object.entries(ENTITY_KIND_MAP).map(([cat, kind]) => [kind, cat]),
  ) as Record<EntityKind, NoteCategoryId>;

  let pendingOpen = $state<{ id: string; kind: EntityKind } | null>(null);
  let graphFor = $state<{ id: string; kind: string } | null>(null);

  function openEntity(id: string, kind: EntityKind) {
    const cat = KIND_TO_CATEGORY[kind];
    if (!cat) return;
    pendingOpen = { id, kind };
    view = { kind: 'notebook', category: cat };
  }

  const ACTIVE_KEY = 'chronacle_active_campaign_id';

  let view = $state<View>('oracle');
  let campaigns = $state<Campaign[]>([]);
  let activeCampaignId = $state<string | null>(null);
  let switcherOpen = $state(false);
  let railCounts = $state<Partial<Record<NoteCategoryId, number>>>({});
  let maintenanceCount = $state(0);

  async function refreshMaintenanceCount() {
    try {
      const counts = await getMaintenanceCounts();
      maintenanceCount = counts.pending_proposals + counts.unresolved_findings;
    } catch (e) {
      console.error('Failed to load maintenance counts:', e);
    }
  }

  async function refreshRailCounts(campaignId: string | null) {
    if (!campaignId) {
      railCounts = {};
      return;
    }
    try {
      const [counts, sessions] = await Promise.all([
        getEntityCounts(campaignId),
        getSessions(campaignId),
      ]);
      const next: Partial<Record<NoteCategoryId, number>> = { sessions: sessions.length };
      for (const [cat, kind] of Object.entries(ENTITY_KIND_MAP) as Array<
        [NoteCategoryId, EntityKind]
      >) {
        next[cat] = counts[kind] ?? 0;
      }
      railCounts = next;
    } catch {
      // Counts are decorative — fall back to placeholders on failure.
      railCounts = {};
    }
  }

  // Refresh whenever the campaign changes or the user navigates (e.g. after
  // creating or deleting entities in a manager view).
  $effect(() => {
    void view;
    refreshRailCounts(activeCampaignId);
  });

  // ── Keyboard shortcuts (Vim-style g-chords; see lib/shortcuts.ts) ──────────
  let leader = $state(false); // true while a `g`-chord awaits its second key
  let leaderTimer: ReturnType<typeof setTimeout> | null = null;
  let showHelp = $state(false);
  // Nonces: bumping one signals the relevant child view to act (focus / create).
  let chatFocusNonce = $state(0);
  let entityCreateNonce = $state(0);

  function clearLeader() {
    leader = false;
    if (leaderTimer) {
      clearTimeout(leaderTimer);
      leaderTimer = null;
    }
  }

  function navTo(target: NavTarget) {
    if (target === 'oracle' || target === 'settings' || target === 'timeline') view = target;
    else view = { kind: 'notebook', category: target.category };
  }

  function handleWindowKey(e: KeyboardEvent) {
    // Escape always cancels a pending chord and closes the help overlay.
    if (e.key === 'Escape') {
      clearLeader();
      showHelp = false;
      return;
    }
    // We only bind bare keys — never hijack OS/browser modifier combos.
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    // Don't fire while the GM is typing in a field.
    if (isEditableTarget(e.target)) return;
    // While a modal/picker owns the screen, let it have the keyboard.
    if (switcherOpen || showPicker || graphFor) return;
    // With the help overlay open, only `?` (toggle off) is live; Esc handled above.
    if (showHelp) {
      if (e.key === '?') {
        e.preventDefault();
        showHelp = false;
      }
      return;
    }

    // Second key of a `g`-chord → navigate.
    if (leader) {
      const target = resolveNavChord(e.key);
      clearLeader();
      if (target) {
        e.preventDefault();
        navTo(target);
      }
      return;
    }

    switch (e.key) {
      case 'g':
        e.preventDefault();
        leader = true;
        leaderTimer = setTimeout(clearLeader, 1500);
        break;
      case '?':
        e.preventDefault();
        showHelp = true;
        break;
      case '/':
        e.preventDefault();
        view = 'oracle';
        chatFocusNonce++;
        break;
      case 'c':
        e.preventDefault();
        entityCreateNonce++;
        break;
    }
  }

  // Upload dialog state (lifted from old App.svelte)
  type UploadPhase = 'idle' | 'active' | 'done' | 'error';
  let uploadPhase = $state<UploadPhase>('idle');
  let uploadProgress = $state(0);
  let uploadStatus = $state('');
  let uploadedSourceName = $state('');
  let collections = $state<Collection[]>([]);
  let pendingPath = $state<string | null>(null);
  let pendingName = $state<string | null>(null);
  let showPicker = $state(false);
  let pickerCollectionId = $state('');
  let pickerNewName = $state('');
  let showNewCollectionInput = $state(false);
  let pickerError = $state('');

  // Embedding model mismatch (ADR-003): banner shown when indexed sources
  // were embedded with a different model than the active provider.
  let mismatch = $state<EmbeddingModelMismatch | null>(null);
  let reindexing = $state(false);
  let mismatchDismissed = $state(false);
  let reindexProgress = $state<{ current: number; total: number; step: string } | null>(null);
  let reindexError = $state('');

  onMount(async () => {
    try {
      campaigns = await getCampaigns();
    } catch (e) {
      console.error('Failed to load campaigns:', e);
    }
    // Defensive: `localStorage` can be undefined during async teardown in
    // some test environments. Treat any throw as "no stored selection".
    let stored: string | null = null;
    try {
      stored = localStorage.getItem(ACTIVE_KEY);
    } catch {
      stored = null;
    }
    if (stored && campaigns.some((c) => c.id === stored)) {
      activeCampaignId = stored;
    } else if (campaigns.length > 0) {
      setActiveCampaignId(campaigns[0].id);
    } else {
      activeCampaignId = null;
      view = 'campaign';
    }
    // Initial mismatch check covers reloads after the startup event already
    // fired. The $effect listener below catches the live event from `setup`.
    try {
      const report = await getEmbeddingModelMismatch();
      if (report.stale.length > 0) mismatch = report;
    } catch (e) {
      console.error('mismatch check failed:', e);
    }
    await refreshMaintenanceCount();
  });

  // Listen for the startup mismatch event. $effect handles unsubscribe on
  // teardown even with async listener setup.
  $effect(() => {
    let unlisten: (() => void) | null = null;
    onEmbeddingModelMismatch((payload) => {
      if (payload.stale.length > 0) mismatch = payload;
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  });

  async function handleReindex() {
    if (reindexing) return;
    reindexing = true;
    reindexError = '';
    reindexProgress = null;
    let unlistenProgress: UnlistenFn | null = null;
    try {
      unlistenProgress = await listen<{
        source_id: string;
        current: number;
        total: number;
        progress: number;
        step: string;
      }>('reindex-progress', (event) => {
        reindexProgress = {
          current: event.payload.current,
          total: event.payload.total,
          step: event.payload.step,
        };
      });
      await reindexAllSources();
      const report = await getEmbeddingModelMismatch();
      if (report.stale.length === 0) {
        mismatch = null;
        mismatchDismissed = false;
      } else {
        mismatch = report;
      }
    } catch (e) {
      reindexError = `Re-indexing failed: ${String(e)}`;
    } finally {
      if (unlistenProgress) unlistenProgress();
      reindexProgress = null;
      reindexing = false;
    }
  }

  let totalStaleSources = $derived(
    mismatch ? mismatch.stale.reduce((acc, s) => acc + s.source_count, 0) : 0,
  );

  function setActiveCampaignId(id: string | null) {
    activeCampaignId = id;
    // Same defensive guard as the read in onMount: `localStorage` can be
    // undefined in some test environments.
    try {
      if (id) localStorage.setItem(ACTIVE_KEY, id);
      else localStorage.removeItem(ACTIVE_KEY);
    } catch {
      /* persistence is best-effort */
    }
  }

  async function refreshCampaigns() {
    campaigns = await getCampaigns();
    if (activeCampaignId && !campaigns.some((c) => c.id === activeCampaignId)) {
      // Active campaign was deleted — fall back to the first remaining one
      // (or null if there are none, which forces the user to the campaign view).
      if (campaigns.length > 0) {
        setActiveCampaignId(campaigns[0].id);
      } else {
        setActiveCampaignId(null);
        view = 'campaign';
      }
    } else if (!activeCampaignId && campaigns.length > 0) {
      setActiveCampaignId(campaigns[0].id);
    }
  }

  let activeCampaign = $derived(campaigns.find((c) => c.id === activeCampaignId) ?? null);

  // Topbar copy
  let head = $derived.by(() => {
    if (view === 'oracle')
      return { title: 'Oracle', sub: 'Ask in plain language — answers come cited' };
    if (view === 'campaign')
      return { title: 'Campaign', sub: 'Manage details & subscribed source collections' };
    if (view === 'settings') return { title: 'Settings', sub: 'Provider, models, and re-indexing' };
    if (view === 'timeline')
      return { title: 'Timeline', sub: 'Your campaign in chronological and session order' };
    if (view === 'maintenance')
      return { title: 'Maintenance', sub: 'Codex proposals and lint findings' };
    const cat = findCategory(view.category);
    return { title: cat.label, sub: cat.sub };
  });

  async function openFilePicker(initialCollectionId?: string) {
    if (uploadPhase === 'active') {
      showToast('An upload is already in progress — wait for it to finish.', 'info');
      return;
    }
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;
    const path = typeof selected === 'string' ? selected : selected[0];
    const name = path.split('/').pop()?.split('\\').pop() ?? 'document.pdf';
    pendingPath = path;
    pendingName = name;

    if (initialCollectionId) {
      // Skip the picker dialog: upload straight into the given collection.
      await startUpload(path, name, initialCollectionId);
      return;
    }

    try {
      collections = await getCollections();
    } catch (e) {
      console.error('Failed to load collections:', e);
      collections = [];
    }
    const mru = getMruCollectionId();
    pickerCollectionId =
      mru && collections.some((c) => c.id === mru) ? mru : (collections[0]?.id ?? '');
    showPicker = true;
    pickerError = '';
    showNewCollectionInput = false;
    pickerNewName = '';
  }

  async function handlePickerCreateNew() {
    if (!pickerNewName.trim()) return;
    pickerError = '';
    try {
      const newCol = await createCollection(pickerNewName.trim());
      collections = [...collections, newCol];
      pickerCollectionId = newCol.id;
      pickerNewName = '';
      showNewCollectionInput = false;
    } catch (e) {
      pickerError = String(e);
    }
  }

  async function confirmUpload() {
    if (!pickerCollectionId || !pendingPath || !pendingName) return;
    pickerError = '';
    const path = pendingPath;
    const name = pendingName;
    const colId = pickerCollectionId;
    showPicker = false;
    pendingPath = null;
    pendingName = null;
    setMruCollectionId(colId);
    await startUpload(path, name, colId);
  }

  function cancelPicker() {
    showPicker = false;
    pendingPath = null;
    pendingName = null;
  }

  function resetUpload() {
    uploadPhase = 'idle';
    uploadStatus = '';
    uploadProgress = 0;
    uploadedSourceName = '';
  }

  async function startUpload(path: string, name: string, collectionId: string) {
    if (uploadPhase === 'active') return;
    uploadPhase = 'active';
    uploadProgress = 0;
    uploadStatus = 'Uploading…';
    uploadedSourceName = name;
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    try {
      unlistenProgress = await listen<{
        source_id: string;
        status: string;
        progress: number;
        step?: string;
        current?: number | null;
        total?: number | null;
      }>('ingestion-progress', (event) => {
        uploadProgress = Math.round(event.payload.progress * 100);
        if (event.payload.status === 'done') {
          uploadStatus = 'Ready!';
          uploadProgress = 100;
          uploadPhase = 'done';
        } else if (event.payload.step) {
          uploadStatus = event.payload.step;
        } else {
          uploadStatus = 'Indexing PDF…';
        }
      });
      unlistenError = await listen<{ source_id: string; error: string }>(
        'ingestion-error',
        (event) => {
          uploadStatus = `Error: ${event.payload.error}`;
          uploadPhase = 'error';
          showToast(`"${name}" failed to index: ${event.payload.error}`, 'error');
        },
      );
      await uploadSource(path, name, 'rules', collectionId);
      // The 'done' progress event normally lands before the command resolves,
      // but don't leave the strip stuck on 'active' if it was dropped.
      if (uploadPhase === 'active') {
        uploadStatus = 'Ready!';
        uploadProgress = 100;
        uploadPhase = 'done';
      }
    } catch (e) {
      // The ingestion-error event usually fires first with a cleaner message;
      // only surface the rejection if it didn't.
      if (uploadPhase !== 'error') {
        uploadStatus = `Upload failed: ${String(e)}`;
        uploadPhase = 'error';
        showToast(`"${name}" failed to upload: ${String(e)}`, 'error');
      }
    } finally {
      if (unlistenProgress) unlistenProgress();
      if (unlistenError) unlistenError();
    }
  }
</script>

<svelte:window onkeydown={handleWindowKey} />

<div class="app">
  <CampaignRail
    {view}
    {activeCampaign}
    counts={railCounts}
    {maintenanceCount}
    setView={(v) => (view = v)}
    onOpenSwitcher={() => (switcherOpen = true)}
    onOpenUpload={() => openFilePicker()}
  />

  {#if switcherOpen}
    <CampaignSwitcher
      {campaigns}
      {activeCampaignId}
      onSelect={setActiveCampaignId}
      onManage={() => (view = 'campaign')}
      onClose={() => (switcherOpen = false)}
    />
  {/if}

  {#if showHelp}
    <div
      class="help-backdrop"
      role="button"
      tabindex="-1"
      aria-label="Close shortcuts"
      onclick={() => (showHelp = false)}
      onkeydown={(e) => e.key === 'Enter' && (showHelp = false)}
    >
      <div
        class="help-card"
        role="dialog"
        aria-label="Keyboard shortcuts"
        onclick={(e) => e.stopPropagation()}
        onkeydown={() => {
          /* swallow so backdrop key handler doesn't double-fire */
        }}
        tabindex="-1"
      >
        <h2>Keyboard shortcuts</h2>
        <dl class="help-list">
          {#each SHORTCUT_HELP as row (row.keys)}
            <div class="help-row">
              <dt><kbd>{row.keys}</kbd></dt>
              <dd>{row.label}</dd>
            </div>
          {/each}
        </dl>
        <p class="help-hint">Press <kbd>?</kbd> or <kbd>Esc</kbd> to close.</p>
      </div>
    </div>
  {/if}

  <main class="main">
    <Topbar title={head.title} sub={head.sub} />
    {#if mismatch && !mismatchDismissed}
      <div class="mismatch-banner" role="status" data-testid="mismatch-banner">
        <div class="mismatch-text">
          <strong>Embedding model changed.</strong>
          {totalStaleSources}
          source{totalStaleSources === 1 ? '' : 's'} indexed with a different model ({mismatch.stale
            .map((s) => s.embed_model)
            .join(', ')}). Retrieval quality will suffer until they are re-indexed with the active
          model ({mismatch.active_model}).
          {#if reindexProgress}
            <div class="mismatch-progress">
              Re-indexing {reindexProgress.current}/{reindexProgress.total} — {reindexProgress.step}
              <div class="mismatch-progress-bar">
                <div
                  class="mismatch-progress-fill"
                  style="width: {reindexProgress.total > 0
                    ? Math.round((reindexProgress.current / reindexProgress.total) * 100)
                    : 0}%"
                ></div>
              </div>
            </div>
          {/if}
          {#if reindexError}
            <div class="mismatch-error">{reindexError}</div>
          {/if}
        </div>
        <div class="mismatch-actions">
          <button
            class="mismatch-reindex-btn"
            onclick={handleReindex}
            disabled={reindexing}
            data-testid="mismatch-reindex"
          >
            {reindexing ? 'Re-indexing…' : 'Re-index now'}
          </button>
          <button
            class="mismatch-dismiss-btn"
            onclick={() => (mismatchDismissed = true)}
            disabled={reindexing}
            data-testid="mismatch-dismiss"
          >
            Dismiss
          </button>
        </div>
      </div>
    {/if}
    {#if view === 'oracle'}
      <OracleView
        {activeCampaignId}
        onOpenUpload={() => openFilePicker()}
        focusNonce={chatFocusNonce}
        onSavedToCodex={() => refreshMaintenanceCount()}
      />
    {:else if view === 'campaign'}
      <CampaignView
        {activeCampaignId}
        {campaigns}
        {setActiveCampaignId}
        onOpenUpload={(colId) => openFilePicker(colId)}
        {refreshCampaigns}
      />
    {:else if view === 'settings'}
      <SettingsView />
    {:else if view === 'timeline' && activeCampaignId}
      <TimelineView campaignId={activeCampaignId} onOpenEntity={(e) => openEntity(e.id, e.kind)} />
    {:else if view === 'timeline'}
      <div class="no-campaign-msg">
        <p>Create or select a campaign to see its timeline.</p>
      </div>
    {:else if view === 'maintenance'}
      <MaintenanceView onCountsChanged={refreshMaintenanceCount} />
    {:else if typeof view === 'object' && view.category === 'sessions' && activeCampaignId}
      <SessionLogView campaignId={activeCampaignId} />
    {:else if typeof view === 'object' && view.category === 'sessions'}
      <div class="no-campaign-msg">
        <p>Select a campaign to view sessions.</p>
      </div>
    {:else if ENTITY_KIND_MAP[view.category] && activeCampaignId}
      <EntityManager
        campaignId={activeCampaignId}
        kind={ENTITY_KIND_MAP[view.category] as EntityKind}
        createNonce={entityCreateNonce}
        openId={pendingOpen && pendingOpen.kind === ENTITY_KIND_MAP[view.category] ? pendingOpen.id : null}
        onOpenIdConsumed={() => (pendingOpen = null)}
        onViewGraph={(n) => (graphFor = { id: n.id, kind: n.kind })}
        onOpenEntity={(id, kind) => openEntity(id, kind as EntityKind)}
      />
    {:else if ENTITY_KIND_MAP[view.category]}
      <div class="no-campaign-msg">
        <p>Select a campaign to manage entities.</p>
      </div>
    {:else}
      <NotesView category={view.category} />
    {/if}

    <UploadProgress
      phase={uploadPhase}
      filename={uploadedSourceName}
      status={uploadStatus}
      progress={uploadProgress}
      onDismiss={resetUpload}
    />
  </main>

  <Toast />

  {#if graphFor}
    <div
      class="graph-overlay"
      role="dialog"
      aria-modal="true"
      aria-label="Entity relationships"
      tabindex="-1"
      onclick={(e) => { if (e.target === e.currentTarget) graphFor = null; }}
      onkeydown={() => { /* Esc is handled by modalBehavior on the inner panel */ }}
    >
      <div class="graph-panel" use:modalBehavior={{ onClose: () => (graphFor = null) }}>
        <EntityGraph
          entityId={graphFor.id}
          entityKind={graphFor.kind}
          onClose={() => (graphFor = null)}
          onOpenEntity={(n) => {
            graphFor = null;
            openEntity(n.id, n.kind as EntityKind);
          }}
        />
      </div>
    </div>
  {/if}

  {#if showPicker}
    <div class="picker-overlay">
      <div
        class="picker-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="picker-title"
        use:modalBehavior={{ onClose: cancelPicker }}
      >
        <h3 id="picker-title">Add "{pendingName}" to collection</h3>
        {#if pickerError}
          <div class="picker-error">{pickerError}</div>
        {/if}
        {#if collections.length > 0}
          <select bind:value={pickerCollectionId} class="picker-select" data-autofocus>
            {#each collections as col (col.id)}
              <option value={col.id}>{col.name}</option>
            {/each}
          </select>
        {:else}
          <p class="picker-hint">No collections yet.</p>
        {/if}
        {#if showNewCollectionInput}
          <div class="picker-new">
            <input
              bind:value={pickerNewName}
              placeholder="New collection name"
              onkeydown={(e) => e.key === 'Enter' && handlePickerCreateNew()}
            />
            <button class="picker-create-btn" onclick={handlePickerCreateNew}>Create</button>
            <button class="picker-cancel-btn" onclick={() => (showNewCollectionInput = false)}
              >Cancel</button
            >
          </div>
        {:else}
          <button class="picker-new-btn" onclick={() => (showNewCollectionInput = true)}
            >+ Create new collection</button
          >
        {/if}
        <div class="picker-actions">
          <button class="picker-cancel-btn" data-testid="picker-cancel" onclick={cancelPicker}
            >Cancel</button
          >
          <button class="picker-confirm-btn" disabled={!pickerCollectionId} onclick={confirmUpload}
            >Upload</button
          >
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* ── Keyboard-shortcuts help overlay ─────────────────────────────────── */
  .help-backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
    cursor: default;
  }
  .help-card {
    width: min(420px, 90vw);
    max-height: 80vh;
    overflow-y: auto;
    padding: 20px 24px;
    border: 1px solid var(--line-strong, var(--line));
    border-radius: var(--r-md, 10px);
    background: var(--bg-panel);
    color: var(--fg-1);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
  }
  .help-card h2 {
    margin: 0 0 14px;
    font-size: 1rem;
    color: var(--fg-1);
  }
  .help-list {
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .help-row {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  .help-row dt {
    flex: 0 0 64px;
    margin: 0;
    text-align: right;
  }
  .help-row dd {
    margin: 0;
    color: var(--fg-2);
    font-size: 0.9rem;
  }
  .help-hint {
    margin: 16px 0 0;
    font-size: 0.8rem;
    color: var(--fg-3);
  }
  kbd {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--line);
    border-radius: 4px;
    background: var(--bg-panel-2, var(--bg-inset));
    color: var(--violet-300, #a78bfa);
    font-family: var(--font-mono, monospace);
    font-size: 0.78rem;
  }

  .app {
    display: grid;
    grid-template-columns: 264px 1fr;
    height: 100%;
    background:
      radial-gradient(70% 80% at 100% 0%, rgba(123, 92, 255, 0.1), transparent 55%),
      var(--bg-void) var(--tex-starfield);
    background-size: auto, 900px;
    color: var(--fg-1);
    font-family: var(--font-sans);
    position: relative;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .picker-overlay {
    position: fixed;
    inset: 0;
    background: var(--bg-scrim);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .picker-dialog {
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-3);
    padding: 18px;
    width: 340px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-family: var(--font-sans);
  }
  .picker-dialog h3 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 16px;
    color: var(--fg-1);
  }
  .picker-error {
    color: var(--danger);
    background: var(--danger-bg);
    border-radius: var(--r-sm);
    padding: 6px 10px;
    font-size: 12.5px;
  }
  .picker-select,
  .picker-new input {
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg-inset);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
  }
  .picker-hint {
    font-size: 13px;
    color: var(--fg-3);
    margin: 0;
  }
  .picker-new {
    display: flex;
    gap: 6px;
  }
  .picker-new input {
    flex: 1;
  }
  .picker-new-btn {
    background: none;
    border: 1px dashed var(--line);
    border-radius: var(--r-md);
    padding: 6px 12px;
    font-size: 12.5px;
    color: var(--fg-3);
    font-family: var(--font-sans);
  }
  .picker-new-btn:hover {
    border-color: var(--line-glow);
    color: var(--arcane-300);
  }
  .picker-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .picker-cancel-btn {
    background: none;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 7px 12px;
    font-size: 13px;
    color: var(--fg-2);
    font-family: var(--font-sans);
  }
  .picker-confirm-btn,
  .picker-create-btn {
    border: 0;
    border-radius: var(--r-md);
    padding: 7px 14px;
    font-size: 13px;
    font-weight: 600;
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
    font-family: var(--font-sans);
  }
  .picker-confirm-btn:disabled {
    opacity: 0.5;
    box-shadow: none;
  }
  .mismatch-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin: 10px 16px 0;
    padding: 10px 14px;
    border: 1px solid var(--line-strong);
    border-left: 3px solid var(--danger, #d97757);
    border-radius: var(--r-md);
    background: var(--danger-bg, rgba(217, 119, 87, 0.08));
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .mismatch-text {
    flex: 1;
    line-height: 1.45;
  }
  .mismatch-text strong {
    color: var(--fg-1);
  }
  .mismatch-progress {
    margin-top: 6px;
    font-size: 12px;
    color: var(--fg-2);
  }
  .mismatch-progress-bar {
    margin-top: 4px;
    height: 3px;
    background: var(--line);
    border-radius: 2px;
    overflow: hidden;
    max-width: 320px;
  }
  .mismatch-progress-fill {
    height: 100%;
    background: var(--grad-arcane);
    transition: width 0.3s ease;
  }
  .mismatch-error {
    margin-top: 6px;
    font-size: 12px;
    color: var(--danger);
  }
  .mismatch-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }
  .mismatch-reindex-btn {
    border: 0;
    border-radius: var(--r-md);
    padding: 7px 12px;
    font-size: 12.5px;
    font-weight: 600;
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
    font-family: var(--font-sans);
    cursor: pointer;
  }
  .mismatch-reindex-btn:disabled {
    opacity: 0.6;
    cursor: progress;
  }
  .mismatch-dismiss-btn {
    background: none;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 7px 12px;
    font-size: 12.5px;
    color: var(--fg-2);
    font-family: var(--font-sans);
    cursor: pointer;
  }
  .mismatch-dismiss-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .no-campaign-msg {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-3);
    font-family: var(--font-sans);
    font-size: 14px;
  }
  /* ── Entity relationship graph overlay ───────────────────────────────── */
  .graph-overlay {
    position: fixed;
    inset: 0;
    background: var(--bg-scrim);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    cursor: default;
  }
  .graph-panel {
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-lg);
    box-shadow:
      var(--shadow-3),
      0 0 0 1px var(--line-faint) inset,
      var(--glow-violet);
    padding: 12px;
    position: relative;
    /* Prevent the backdrop click handler from firing when clicking the panel */
    cursor: default;
    /* Large responsive viewport: EntityGraph fills the panel via width/height 100%. */
    width: min(1100px, 92vw);
    height: min(820px, 88vh);
    display: flex;
    flex-direction: column;
  }
</style>
