<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { NOTE_CATEGORIES } from './note-categories.ts';
  import type { Campaign } from '../lib/commands';
  import type { NoteCategoryId } from './note-categories.ts';

  export type View =
    | 'oracle'
    | 'campaign'
    | 'settings'
    | 'timeline'
    | { kind: 'notebook'; category: NoteCategoryId };

  let {
    view,
    activeCampaign,
    counts = {},
    setView,
    onOpenSwitcher,
    onOpenUpload,
  }: {
    view: View;
    activeCampaign: Campaign | null;
    counts?: Partial<Record<NoteCategoryId, number>>;
    setView: (v: View) => void;
    onOpenSwitcher: () => void;
    onOpenUpload: () => void;
  } = $props();

  function isNotebook(v: View, cat: NoteCategoryId): boolean {
    return typeof v === 'object' && v.kind === 'notebook' && v.category === cat;
  }
</script>

<aside class="rail" aria-label="Campaign rail">
  <div class="rail-head">
    <div class="rail-mark" aria-hidden="true"></div>
    <div class="rail-word">Chron<b>a</b>cle</div>
  </div>

  <button
    class="campaign"
    class:active={view === 'campaign'}
    title="Switch campaign"
    aria-label="Switch campaign"
    onclick={onOpenSwitcher}
  >
    <span class="gem"></span>
    <span class="campaign-text">
      <span class="nm">{activeCampaign?.name ?? 'No campaign'}</span>
      <span class="mt">{activeCampaign?.system ?? 'create one to start'}</span>
    </span>
    <Icon name="chevrons-up-down" size={15} className="chev" />
  </button>

  <nav class="nav primary">
    <button
      class="nav-item"
      class:active={view === 'oracle'}
      onclick={() => setView('oracle')}
    >
      <Icon name="sparkles" size={18} className="ic" />
      Oracle
    </button>
    <button
      class="nav-item"
      class:active={view === 'timeline'}
      onclick={() => setView('timeline')}
    >
      <Icon name="milestone" size={18} className="ic" />
      Timeline
    </button>
  </nav>

  <div class="rail-scroll">
    {#each ['Notebook', 'Entities'] as group (group)}
      <div class="rail-section">{group}</div>
      <nav class="nav">
        {#each NOTE_CATEGORIES.filter((c) => c.group === group) as c (c.id)}
          <button
            class="nav-item"
            class:active={isNotebook(view, c.id)}
            onclick={() => setView({ kind: 'notebook', category: c.id })}
          >
            <Icon name={c.icon} size={18} className="ic" />
            {c.label}
            <span class="ct">{counts[c.id] ?? '—'}</span>
          </button>
        {/each}
      </nav>
    {/each}
  </div>

  <div class="rail-foot">
    <button class="foot-btn" onclick={onOpenUpload} title="Upload a PDF">
      <Icon name="upload" size={16} />
      Upload PDF
    </button>
    <button
      class="foot-btn"
      class:active={view === 'campaign'}
      onclick={() => setView('campaign')}
      title="Manage campaign and source collections"
    >
      <Icon name="library" size={16} />
      Campaign &amp; sources
    </button>
    <button
      class="foot-btn icon-only"
      class:active={view === 'settings'}
      onclick={() => setView('settings')}
      title="Settings"
      aria-label="Settings"
    >
      <Icon name="settings" size={16} />
    </button>
  </div>
</aside>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: linear-gradient(180deg, rgba(16, 19, 42, 0.86), rgba(10, 12, 26, 0.86));
    border-right: 1px solid var(--line);
    backdrop-filter: blur(12px);
    position: relative;
  }
  .rail-head {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 16px 16px 14px;
  }
  .rail-mark {
    width: 36px;
    height: 36px;
    border-radius: 11px;
    background: var(--brand-mark) center/cover;
    box-shadow: 0 0 0 1px var(--line), var(--glow-arcane);
    flex: none;
  }
  .rail-word {
    font-family: var(--font-display);
    font-weight: 800;
    font-size: 19px;
    letter-spacing: 0.04em;
    color: var(--fg-1);
  }
  .rail-word b {
    color: var(--violet-400);
  }
  .campaign {
    margin: 4px 12px 12px;
    padding: 11px 12px;
    border-radius: var(--r-md);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 10px;
    width: calc(100% - 24px);
    text-align: left;
  }
  .campaign:hover {
    border-color: var(--line-strong);
  }
  .campaign.active {
    border-color: var(--line-glow);
    box-shadow: var(--glow-arcane);
  }
  .campaign .gem {
    width: 26px;
    height: 26px;
    border-radius: var(--r-full);
    background: var(--grad-gem);
    box-shadow: var(--glow-violet);
    flex: none;
  }
  .campaign-text {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .campaign .nm {
    font-weight: 700;
    font-size: 13.5px;
    color: var(--fg-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .campaign .mt {
    font-size: 11px;
    color: var(--fg-3);
    font-family: var(--font-mono);
  }
  .nav {
    padding: 6px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 9px 11px;
    border-radius: var(--r-sm);
    color: var(--fg-2);
    font-weight: 600;
    font-size: 14px;
    background: none;
    border: 0;
    text-align: left;
  }
  .nav-item:hover {
    background: rgba(124, 148, 255, 0.07);
    color: var(--fg-1);
  }
  .nav-item.active {
    background: rgba(91, 120, 255, 0.14);
    color: var(--fg-1);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .nav-item .ct {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-3);
  }
  .rail-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 8px;
  }
  .rail-section {
    margin-top: 14px;
    padding: 0 18px 8px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .rail-foot {
    margin-top: auto;
    padding: 12px;
    border-top: 1px solid var(--line-faint);
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 6px;
  }
  .foot-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 8px 10px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--bg-panel);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 12.5px;
  }
  .foot-btn:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .foot-btn.active {
    border-color: var(--line-glow);
    color: var(--fg-1);
    box-shadow: var(--glow-arcane);
  }
  .foot-btn.icon-only {
    padding: 8px;
  }
</style>
