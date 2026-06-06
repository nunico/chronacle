<script lang="ts">
  import { tick } from 'svelte';

  interface Props {
    id?: string;
    value: string;
    entities: Map<string, { id: string; kind: string }>;
    onblur?: () => void;
    rows?: number;
    placeholder?: string;
  }

  let {
    id,
    value = $bindable(''),
    entities,
    onblur,
    rows = 4,
    placeholder = '',
  }: Props = $props();

  let textarea = $state<HTMLTextAreaElement | null>(null);
  let prefix = $state<string | null>(null);
  let selectedIndex = $state(0);

  const suggestions = $derived.by(() => {
    if (prefix === null) return [] as string[];
    const q = prefix.toLowerCase();
    const names = [...entities.keys()];
    const starts = names.filter(n => n.toLowerCase().startsWith(q));
    const contains = names.filter(n => !n.toLowerCase().startsWith(q) && n.toLowerCase().includes(q));
    return [...starts.sort(), ...contains.sort()].slice(0, 8);
  });

  function getPrefix(text: string, pos: number): string | null {
    const before = text.slice(0, pos);
    const match = before.match(/\[\[([^\]\n]*)$/);
    return match ? match[1] : null;
  }

  function handleInput() {
    if (!textarea) return;
    prefix = getPrefix(value, textarea.selectionStart ?? 0);
    selectedIndex = 0;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (suggestions.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % suggestions.length;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + suggestions.length) % suggestions.length;
    } else if (e.key === 'Enter' || e.key === 'Tab') {
      e.preventDefault();
      selectSuggestion(suggestions[selectedIndex]);
    } else if (e.key === 'Escape') {
      prefix = null;
    }
  }

  async function selectSuggestion(name: string) {
    if (!textarea) return;
    const pos = textarea.selectionStart ?? 0;
    const before = value.slice(0, pos);
    const after = value.slice(pos);
    const match = before.match(/\[\[([^\]\n]*)$/);
    if (!match) return;
    const insertStart = before.length - match[0].length;
    value = before.slice(0, insertStart) + `[[${name}]]` + after;
    prefix = null;
    selectedIndex = 0;
    await tick();
    const newPos = insertStart + name.length + 4;
    textarea.selectionStart = newPos;
    textarea.selectionEnd = newPos;
    textarea.focus();
  }
</script>

<div class="wiki-editor">
  <textarea
    bind:this={textarea}
    {id}
    bind:value
    oninput={handleInput}
    onkeydown={handleKeydown}
    {onblur}
    {rows}
    {placeholder}
    class="editor-textarea"
    spellcheck="false"
  ></textarea>
  {#if suggestions.length > 0}
    <div class="suggestions" role="listbox">
      {#each suggestions as name, i (name)}
        <button
          type="button"
          role="option"
          aria-selected={i === selectedIndex}
          class="suggestion-item"
          class:active={i === selectedIndex}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => selectSuggestion(name)}
        >
          <span class="bracket">[[</span>{name}<span class="bracket">]]</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .wiki-editor {
    position: relative;
    width: 100%;
  }

  .editor-textarea {
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: var(--bg-inset);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
    resize: vertical;
    box-sizing: border-box;
    display: block;
  }

  .editor-textarea:focus {
    outline: none;
    border-color: var(--line-glow);
  }

  .suggestions {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    z-index: 50;
    background: var(--bg-panel);
    border: 1px solid var(--line-strong, var(--line));
    border-radius: var(--r-sm);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    overflow: hidden;
    margin-top: 2px;
  }

  .suggestion-item {
    display: block;
    width: 100%;
    padding: 7px 12px;
    background: none;
    border: none;
    border-bottom: 1px solid var(--line);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }

  .suggestion-item:last-child {
    border-bottom: none;
  }

  .suggestion-item:hover,
  .suggestion-item.active {
    background: var(--bg-hover, rgba(255, 255, 255, 0.06));
    color: var(--fg-1);
  }

  .bracket {
    color: var(--arcane-400, #a78bfa);
    font-size: 11px;
    opacity: 0.7;
  }
</style>
