<script lang="ts">
  import Icon from './Icon.svelte';
  import EyeMark from './EyeMark.svelte';
  import type { RulingData } from '../views/ruling-parse';

  let { data, defaultOpen = false }: { data: RulingData; defaultOpen?: boolean } = $props();
  // Writable $derived: which citation is expanded. Seeds from defaultOpen and
  // re-seeds if defaultOpen changes, while click handlers can still override it
  // (the write persists until defaultOpen changes again). Depends only on
  // defaultOpen — not data — so toggling survives parent re-renders.
  let open = $derived(defaultOpen ? 0 : -1);
</script>

<div class="msg">
  <div class="who-av eye-badge"><EyeMark size={28} /></div>
  <div class="ruling">
    <div class="who">
      <span>Chronacle</span>
      <span class="tag">· ruling</span>
    </div>
    {#if data.verdict}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <p class="verdict">{@html data.verdict}</p>
    {/if}
    {#if data.why}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <p class="why">{@html data.why}</p>
    {/if}
    {#if data.cites.length > 0}
      <div class="cite-row">
        {#each data.cites as c, i (c.label + i)}
          <button class="cite" onclick={() => (open = open === i ? -1 : i)}>
            <Icon name="quote" size={14} />
            {c.label}
            <Icon name={open === i ? 'chevron-up' : 'chevron-down'} size={13} />
          </button>
        {/each}
      </div>
      {#if open >= 0 && data.cites[open]}
        <div class="passage">
          <div class="src">{data.cites[open].src}</div>
          <div class="quote">{data.cites[open].quote || 'No supporting quote available.'}</div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .msg {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    margin: 18px 0;
  }
  .who-av {
    flex: none;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .ruling {
    flex: 1;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 14px 16px;
    box-shadow: var(--shadow-card);
    min-width: 0;
  }
  .who {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--fg-2);
    margin-bottom: 6px;
  }
  .tag {
    color: var(--fg-3);
    font-weight: 500;
  }
  .verdict {
    font-family: var(--font-serif);
    font-size: 18px;
    line-height: 1.45;
    color: var(--fg-1);
    margin: 0 0 8px;
  }
  .why {
    font-family: var(--font-serif);
    font-size: 16px;
    line-height: 1.65;
    color: var(--fg-2);
    margin: 0 0 12px;
  }
  .cite-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .cite {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: var(--r-full);
    border: 1px solid var(--line);
    color: var(--arcane-300);
    font-family: var(--font-mono);
    font-size: 12px;
    background: rgba(91, 120, 255, 0.06);
  }
  .cite:hover {
    border-color: var(--line-strong);
    color: var(--gem);
  }
  .passage {
    margin-top: 10px;
    padding: 12px 14px;
    background: var(--bg-inset);
    border: 1px solid var(--line-faint);
    border-radius: var(--r-md);
  }
  .src {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-3);
    margin-bottom: 6px;
    letter-spacing: 0.02em;
  }
  .quote {
    font-family: var(--font-serif);
    font-style: italic;
    color: var(--fg-2);
    font-size: 14.5px;
    line-height: 1.55;
  }
</style>
