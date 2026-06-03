<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { findCategory, type NoteCategoryId } from '../shell/note-categories.ts';

  let { category }: { category: NoteCategoryId } = $props();
  let cat = $derived(findCategory(category));
</script>

<div class="scroll">
  <div class="notes">
    <div class="notes-head">
      <div>
        <h1>{cat.label}</h1>
        <p class="sub">{cat.sub}</p>
        <div class="notes-path">
          <Icon name="folder" size={12} />
          {cat.folder}/
        </div>
      </div>
    </div>

    <div class="empty">
      <div class="glyph">✦</div>
      <h2>Coming in Phase 2</h2>
      <p>
        Your campaign's <code>{cat.folder}/</code> will live here — searchable notes, file-backed,
        linked to entities Chronacle can answer about.
      </p>
    </div>
  </div>
</div>

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .notes {
    max-width: 820px;
    margin: 0 auto;
    padding: 30px 26px 40px;
  }
  .notes-head {
    margin-bottom: 22px;
  }
  .notes-head h1 {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 28px;
    margin: 0;
    color: var(--fg-1);
  }
  .notes-head .sub {
    font-family: var(--font-serif);
    font-size: 15px;
    color: var(--fg-2);
    margin: 6px 0 8px;
    max-width: 60ch;
  }
  .notes-path {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--fg-3);
  }
  .empty {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 36px 28px;
    box-shadow: var(--shadow-card);
    text-align: center;
  }
  .glyph {
    font-family: var(--font-display);
    font-size: 28px;
    color: var(--arcane-300);
    margin-bottom: 10px;
  }
  .empty h2 {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 20px;
    margin: 0 0 8px;
    color: var(--fg-1);
  }
  .empty p {
    font-family: var(--font-serif);
    font-size: 15px;
    color: var(--fg-2);
    max-width: 56ch;
    margin: 0 auto;
    line-height: 1.55;
  }
  .empty code {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--arcane-300);
  }
</style>
