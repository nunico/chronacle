<script lang="ts">
  import { i18n } from '../lib/locale.svelte';
  import Button from './ui/Button.svelte';
  interface Props {
    /** The entity's current alternate names. Controlled — this component never
     * mutates it; every add/remove reports the full resulting array via `onchange`. */
    aliases: string[];
    /** Called with the COMPLETE alternate-name array after every add/remove. */
    onchange?: (aliases: string[]) => void;
  }

  let { aliases, onchange }: Props = $props();

  let draft = $state('');

  function addAlias() {
    const name = draft.trim();
    if (!name) return;
    if (aliases.some((a) => a.toLowerCase() === name.toLowerCase())) {
      draft = '';
      return;
    }
    onchange?.([...aliases, name]);
    draft = '';
  }

  function removeAlias(name: string) {
    onchange?.(aliases.filter((a) => a !== name));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addAlias();
    }
  }
</script>

<div class="alias-field">
  <label for="alias-field-input">{i18n.t('entityUi.alternateNames')}</label>
  <p class="hint">{i18n.t('entityUi.alternateNamesHint')}</p>

  {#if aliases.length > 0}
    <ul class="chip-list">
      {#each aliases as name (name)}
        <li class="chip">
          <span class="chip-name">{name}</span>
          <button
            type="button"
            class="chip-remove"
            aria-label={i18n.t('entityUi.removeName', { name })}
            onclick={() => removeAlias(name)}
          >
            ×
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="add-row">
    <input
      id="alias-field-input"
      type="text"
      placeholder={i18n.t('entityUi.addAlternateName')}
      bind:value={draft}
      onkeydown={handleKeydown}
    />
    <Button variant="ghost" onclick={addAlias}>{i18n.t('common.add')}</Button>
  </div>
</div>

<style>
  .alias-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  label {
    font-size: 0.85rem;
    color: var(--fg-3);
  }
  .hint {
    margin: 0;
    font-size: 0.78rem;
    color: var(--fg-4, var(--fg-3));
  }
  .chip-list {
    list-style: none;
    margin: 4px 0 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 4px;
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 2px 4px 2px 10px;
    font-size: 0.82rem;
    color: var(--fg-1);
  }
  .chip-remove {
    background: none;
    border: none;
    color: var(--fg-3);
    cursor: pointer;
    font-size: 0.95rem;
    line-height: 1;
    padding: 2px 6px;
  }
  .chip-remove:hover {
    color: var(--danger);
  }
  .add-row {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .add-row input {
    flex: 1;
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--fg-1);
    padding: 6px 10px;
    font-size: 0.9rem;
  }
</style>
