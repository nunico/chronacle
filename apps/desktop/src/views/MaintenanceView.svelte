<script lang="ts">
  import { onMount } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import { fade, slide, type TransitionConfig } from 'svelte/transition';
  import {
    getProposals,
    acceptProposal,
    rejectProposal,
    getLintFindings,
    runLint,
    resolveLintFinding,
    deleteRelation,
    compileEntity,
    confirmAliasSuggestion,
    undoAutoAlias,
    resolveAliasCollision,
    type CodexProposal,
    type LintFinding,
    type EntityKind,
  } from '../lib/commands';
  import MergeDialog from '../components/MergeDialog.svelte';

  interface Props {
    onCountsChanged?: () => void;
    activeCampaignId?: string | null;
    onOpenEntity?: (id: string, kind: string) => void;
    onCreateMissingArticle?: (name: string, sourceFindingId: string) => void;
  }
  let { onCountsChanged, activeCampaignId, onOpenEntity, onCreateMissingArticle }: Props =
    $props();

  let tab = $state<'proposals' | 'findings'>('proposals');
  let proposals = $state<CodexProposal[]>([]);
  let findings = $state<LintFinding[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null); // proposal/finding id being resolved
  let compilingId = $state<string | null>(null); // stale finding whose article is compiling
  let checking = $state(false);
  let lintNote = $state<string | null>(null);
  let mergeTarget = $state<{
    a: { id: string; kind: string };
    b: { id: string; kind: string };
  } | null>(null);

  const KIND_LABELS: Record<string, string> = {
    entity_article_update: 'Article update',
    entity_notes_update: 'Notes suggestion',
    rule_entry_update: 'Rule update',
    new_entity: 'New entity',
    new_rule_entry: 'New rule',
  };

  const FINDING_LABELS: Record<string, string> = {
    orphaned_edge: 'Orphaned edge',
    scope_violation: 'Scope violation',
    broken_wikilink: 'Wikilinks',
    stale_article: 'Stale article',
    duplicate_entity: 'Possible duplicate',
    alias_collision: 'Naming conflict',
  };

  const CARD_OUTRO_MS = 280;

  function cardOutro(node: Element): TransitionConfig {
    const faded = fade(node, { duration: CARD_OUTRO_MS });
    const slid = slide(node, { duration: CARD_OUTRO_MS });

    return {
      duration: CARD_OUTRO_MS,
      css: (t, u) => `${slid.css?.(t, u) ?? ''}${faded.css?.(t, u) ?? ''}`,
    };
  }

  const findingsByKind = $derived.by(() => {
    const groups = new SvelteMap<string, LintFinding[]>();
    for (const f of findings) {
      const list = groups.get(f.kind) ?? [];
      list.push(f);
      groups.set(f.kind, list);
    }
    return groups;
  });

  function entityRef(v: unknown): { id: string; kind: string } | null {
    if (typeof v !== 'string' || !v.includes(':')) return null;
    const [kind, id] = v.split(':', 2);
    return { id, kind };
  }

  interface AliasCandidate {
    id: string;
    name: string;
    similarity: number;
  }

  function candidatesOf(f: LintFinding): AliasCandidate[] {
    const c = f.payload.candidates;
    return Array.isArray(c) ? (c as AliasCandidate[]) : [];
  }

  function hasCandidates(f: LintFinding): boolean {
    return candidatesOf(f).length > 0;
  }

  function brokenWikilinkLabel(f: LintFinding): string {
    return hasCandidates(f) ? 'Possible name mismatch' : 'Missing article';
  }

  async function refresh({ showLoading = false }: { showLoading?: boolean } = {}) {
    if (showLoading) loading = true;
    error = null;
    try {
      const [p, f] = await Promise.all([getProposals('pending'), getLintFindings()]);
      proposals = p;
      findings = f;
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

  async function resolveFinding(id: string) {
    busy = id;
    try {
      await resolveLintFinding(id);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function openEntityRef(v: unknown) {
    const ref = entityRef(v);
    if (ref) onOpenEntity?.(ref.id, ref.kind);
  }

  async function compileAndResolve(f: LintFinding) {
    const ref = entityRef(f.payload.entity);
    if (!ref) return;
    busy = f.id;
    compilingId = f.id;
    try {
      await compileEntity(ref.kind, ref.id);
      await resolveLintFinding(f.id);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
      compilingId = null;
    }
  }

  async function deleteEdgeAndResolve(f: LintFinding) {
    busy = f.id;
    try {
      await deleteRelation(String(f.payload.edge));
      await resolveLintFinding(f.id);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  /** "Did you mean X?" — the GM confirms the top candidate as an alternate name. */
  async function confirmSuggestion(f: LintFinding, candidate: AliasCandidate) {
    busy = f.id;
    try {
      await confirmAliasSuggestion(candidate.id, String(f.payload.link_text));
      await resolveLintFinding(f.id);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  /** Undo an alternate name the fuzzy resolver auto-linked without asking. */
  async function undoSuggestion(f: LintFinding) {
    busy = f.id;
    try {
      await undoAutoAlias(String(f.payload.entity), String(f.payload.alias), f.id);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  async function resolveCollision(f: LintFinding, keepId: string, dropId: string) {
    busy = f.id;
    try {
      await resolveAliasCollision(f.id, keepId, dropId);
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  /** Prefer the enriched name; fall back to the record id for deleted parties. */
  function partyName(f: LintFinding, side: 'a' | 'b'): string {
    const name = f.payload[`${side}_name`];
    if (typeof name === 'string' && name) return name;
    return entityRef(f.payload[side])?.id ?? String(f.payload[side]);
  }

  /** Enriched name for a single-entity finding's `entity` field; id fallback. */
  function entityName(f: LintFinding): string {
    const name = f.payload.entity_name;
    if (typeof name === 'string' && name) return name;
    return entityRef(f.payload.entity)?.id ?? String(f.payload.entity ?? '');
  }

  function openMerge(f: LintFinding) {
    const a = entityRef(f.payload.a);
    const b = entityRef(f.payload.b);
    if (a && b) mergeTarget = { a, b };
  }

  async function handleMerged() {
    mergeTarget = null;
    await refresh();
    onCountsChanged?.();
  }

  async function checkCampaign() {
    if (!activeCampaignId) return;
    checking = true;
    try {
      const s = await runLint(activeCampaignId);
      lintNote = `${s.new_findings} new finding${s.new_findings === 1 ? '' : 's'} · ${s.unresolved_total} open`;
      await refresh();
      onCountsChanged?.();
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  }

  onMount(() => void refresh({ showLoading: true }));
</script>

<div class="maintenance">
  <div class="header-row">
    <h2 class="heading">Maintenance</h2>
    {#if tab === 'findings'}
      <div class="check-campaign">
        <button type="button" disabled={checking} onclick={() => void checkCampaign()}>
          {checking ? 'Checking…' : 'Check campaign'}
        </button>
        {#if lintNote}
          <span class="lint-note">{lintNote}</span>
        {/if}
      </div>
    {/if}
  </div>

  <div class="toolbar" role="tablist" aria-label="Maintenance sections">
    <button
      role="tab"
      aria-selected={tab === 'proposals'}
      class:active={tab === 'proposals'}
      onclick={() => (tab = 'proposals')}
    >
      Proposals
    </button>
    <button
      role="tab"
      aria-selected={tab === 'findings'}
      class:active={tab === 'findings'}
      onclick={() => (tab = 'findings')}
    >
      Findings
    </button>
  </div>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if tab === 'proposals'}
    {#if proposals.length === 0}
      <p class="muted">No pending proposals</p>
    {:else}
      <ul class="proposal-list">
        {#each proposals as p (p.id)}
          <li class="proposal-card motion-list-card" out:cardOutro>
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
  {:else if findings.length === 0}
    <p class="muted">No unresolved findings</p>
  {:else}
    <div class="finding-groups">
      {#each [...findingsByKind.entries()] as [kind, items] (kind)}
        <section class="finding-group">
          {#if kind === 'auto_alias'}
            <details class="auto-alias-details">
              <summary class="finding-kind-heading">
                Auto-linked ({items.length}) — reviewable, not required
              </summary>
              <ul class="finding-list">
                {#each items as f (f.id)}
                  <li class="finding-card motion-list-card" out:cardOutro>
                    <p class="finding-detail">
                      <strong>{String(f.payload.alias)}</strong> was auto-linked to
                      <strong>{entityRef(f.payload.entity)?.id ?? String(f.payload.entity)}</strong>
                    </p>
                    <div class="finding-actions">
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.entity)}
                      >
                        Open entity
                      </button>
                      <button
                        type="button"
                        class="btn-ghost"
                        disabled={busy === f.id}
                        onclick={() => undoSuggestion(f)}
                      >
                        Undo
                      </button>
                    </div>
                  </li>
                {/each}
              </ul>
            </details>
          {:else}
            <h3 class="finding-kind-heading">{FINDING_LABELS[kind] ?? kind}</h3>
            <ul class="finding-list">
              {#each items as f (f.id)}
                <li class="finding-card motion-list-card" out:cardOutro>
                  {#if kind === 'broken_wikilink'}
                    <div class="proposal-head">
                      <span class="chip-kind">{brokenWikilinkLabel(f)}</span>
                    </div>
                    <p class="finding-detail">
                      [[{String(f.payload.link_text)}]] in {entityName(f)}
                    </p>
                    {#if candidatesOf(f).length > 0}
                      {@const candidate = candidatesOf(f)[0]}
                      <p class="finding-detail">
                        Suggested match: <strong>{candidate.name}</strong>
                      </p>
                    {/if}
                    <div class="finding-actions">
                      {#if candidatesOf(f).length > 0}
                        <button
                          type="button"
                          disabled={busy === f.id}
                          onclick={() => confirmSuggestion(f, candidatesOf(f)[0])}
                        >
                          Use suggestion
                        </button>
                      {/if}
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() =>
                          onCreateMissingArticle?.(String(f.payload.link_text), f.id)}
                      >
                        Create article
                      </button>
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.entity)}
                      >
                        Open source
                      </button>
                      <button
                        type="button"
                        class="btn-ghost"
                        disabled={busy === f.id}
                        onclick={() => resolveFinding(f.id)}
                      >
                        Dismiss
                      </button>
                    </div>
                  {:else if kind === 'stale_article'}
                    <p class="finding-detail">
                      <strong>{entityName(f)}</strong> — {f.payload.reason}
                    </p>
                    {#if compilingId === f.id}
                      <div class="compiling-status" role="status" aria-live="polite">
                        <span class="spinner" aria-hidden="true"></span>
                        Compiling…
                      </div>
                    {:else}
                      <div class="finding-actions">
                        <button
                          type="button"
                          disabled={busy === f.id}
                          onclick={() => compileAndResolve(f)}
                        >
                          Compile
                        </button>
                        <button
                          type="button"
                          class="btn-ghost"
                          disabled={busy === f.id}
                          onclick={() => resolveFinding(f.id)}
                        >
                          Dismiss
                        </button>
                      </div>
                    {/if}
                  {:else if kind === 'scope_violation'}
                    <p class="finding-detail">
                      {String(f.payload.from)} → {String(f.payload.to)}
                    </p>
                    <div class="finding-actions">
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => deleteEdgeAndResolve(f)}
                      >
                        Delete edge
                      </button>
                      <button
                        type="button"
                        class="btn-ghost"
                        disabled={busy === f.id}
                        onclick={() => resolveFinding(f.id)}
                      >
                        Dismiss
                      </button>
                    </div>
                  {:else if kind === 'duplicate_entity'}
                    <p class="finding-detail">
                      Possible duplicate:
                      <strong>{partyName(f, 'a')}</strong>
                      and
                      <strong>{partyName(f, 'b')}</strong>
                    </p>
                    <div class="finding-actions">
                      <button type="button" disabled={busy === f.id} onclick={() => openMerge(f)}>
                        Merge
                      </button>
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.a)}
                      >
                        Open A
                      </button>
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.b)}
                      >
                        Open B
                      </button>
                      <button
                        type="button"
                        class="btn-ghost"
                        disabled={busy === f.id}
                        onclick={() => resolveFinding(f.id)}
                      >
                        Dismiss
                      </button>
                    </div>
                  {:else if kind === 'alias_collision'}
                    {@const aName = partyName(f, 'a')}
                    {@const bName = partyName(f, 'b')}
                    {@const aIsName = f.payload.a_is_name === true}
                    {@const bIsName = f.payload.b_is_name === true}
                    {@const aResolved = typeof f.payload.a_name === 'string'}
                    {@const bResolved = typeof f.payload.b_name === 'string'}
                    <p class="finding-detail">
                      <strong>{String(f.payload.alias)}</strong> is claimed by two entities:
                    </p>
                    <div class="conflict-parties">
                      <div class="party">
                        <span class="party-name">{aName}</span>
                        <span class="party-kind">{entityRef(f.payload.a)?.kind ?? ''}</span>
                        <span class="party-tag">{aIsName ? 'as name' : 'as alias'}</span>
                      </div>
                      <div class="party">
                        <span class="party-name">{bName}</span>
                        <span class="party-kind">{entityRef(f.payload.b)?.kind ?? ''}</span>
                        <span class="party-tag">{bIsName ? 'as name' : 'as alias'}</span>
                      </div>
                    </div>
                    <div class="finding-actions">
                      {#if aResolved && bResolved && !bIsName}
                        <button
                          type="button"
                          disabled={busy === f.id}
                          onclick={() =>
                            resolveCollision(f, String(f.payload.a), String(f.payload.b))}
                        >
                          Keep on {aName}
                        </button>
                      {/if}
                      {#if aResolved && bResolved && !aIsName}
                        <button
                          type="button"
                          disabled={busy === f.id}
                          onclick={() =>
                            resolveCollision(f, String(f.payload.b), String(f.payload.a))}
                        >
                          Keep on {bName}
                        </button>
                      {/if}
                      <button type="button" disabled={busy === f.id} onclick={() => openMerge(f)}>
                        Merge…
                      </button>
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.a)}
                      >
                        Open {aName}
                      </button>
                      <button
                        type="button"
                        disabled={busy === f.id}
                        onclick={() => void openEntityRef(f.payload.b)}
                      >
                        Open {bName}
                      </button>
                      <button
                        type="button"
                        class="btn-ghost"
                        disabled={busy === f.id}
                        onclick={() => resolveFinding(f.id)}
                      >
                        Dismiss
                      </button>
                    </div>
                  {:else}
                    <p class="finding-detail">Orphaned relation edge</p>
                    <div class="finding-actions">
                      <button
                        type="button"
                        class="btn-ghost"
                        disabled={busy === f.id}
                        onclick={() => resolveFinding(f.id)}
                      >
                        Dismiss
                      </button>
                    </div>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/each}
    </div>
  {/if}

  {#if mergeTarget}
    <MergeDialog
      idA={mergeTarget.a.id}
      kindA={mergeTarget.a.kind as EntityKind}
      idB={mergeTarget.b.id}
      kindB={mergeTarget.b.kind as EntityKind}
      onclose={() => (mergeTarget = null)}
      onmerged={handleMerged}
    />
  {/if}
</div>

<style>
  .maintenance {
    height: 100%;
    overflow-y: auto;
    box-sizing: border-box;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .heading {
    font-family: var(--font-display);
    color: var(--fg-1);
    margin: 0;
  }
  .check-campaign {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .check-campaign button {
    padding: 6px 14px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--violet-400);
    color: var(--bg-abyss);
    cursor: pointer;
    font-size: 0.85rem;
  }
  .check-campaign button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .lint-note {
    color: var(--fg-3);
    font-size: 0.8rem;
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
  .proposal-list,
  .finding-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .proposal-card,
  .finding-card {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 12px 14px;
  }
  .motion-list-card {
    will-change: opacity, height, margin, padding;
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
  .proposal-actions,
  .finding-actions {
    display: flex;
    gap: 8px;
  }
  .compiling-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 33px;
    color: var(--fg-2);
    font-size: 0.85rem;
  }
  .proposal-actions button,
  .finding-actions button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--violet-400);
    color: var(--bg-abyss);
    cursor: pointer;
    font-size: 0.85rem;
  }
  .compiling-status .spinner {
    display: inline-block;
    flex: 0 0 auto;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid var(--line);
    border-top-color: var(--violet-300);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .motion-list-card {
      animation-duration: 280ms !important;
      animation-iteration-count: 1 !important;
    }
    .compiling-status .spinner {
      animation-duration: 2s !important;
      animation-iteration-count: infinite !important;
    }
  }
  .proposal-actions button.btn-ghost,
  .finding-actions button.btn-ghost {
    background: transparent;
    color: var(--fg-3);
  }
  .proposal-actions button:disabled,
  .finding-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .finding-groups {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .finding-kind-heading {
    font-size: 0.85rem;
    color: var(--fg-2);
    text-transform: uppercase;
    margin: 0 0 8px;
  }
  .auto-alias-details summary.finding-kind-heading {
    cursor: pointer;
    text-transform: none;
    letter-spacing: normal;
  }
  .auto-alias-details .finding-list {
    margin-top: 8px;
  }
  .finding-detail {
    color: var(--fg-2);
    font-size: 0.85rem;
    margin: 0 0 10px;
  }
  .conflict-parties {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 10px;
  }
  .party {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .party-name {
    color: var(--fg-1);
    font-weight: 600;
  }
  .party-kind {
    font-size: 0.7rem;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .party-tag {
    font-size: 0.7rem;
    color: var(--fg-3);
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 1px 8px;
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
