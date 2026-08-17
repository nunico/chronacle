<script lang="ts">
  import type { Locale } from '$lib/i18n/types';

  interface Props {
    value: Locale;
    label: string;
    englishLabel: string;
    germanLabel: string;
    onchange: (locale: Locale) => void;
  }

  let { value, label, englishLabel, germanLabel, onchange }: Props = $props();
</script>

<div class="language-switch" role="group" aria-label={label}>
  <button
    type="button"
    class={{ active: value === 'en' }}
    aria-label={englishLabel}
    aria-pressed={value === 'en'}
    lang="en"
    onclick={() => onchange('en')}
  >
    EN
  </button>
  <span aria-hidden="true">/</span>
  <button
    type="button"
    class={{ active: value === 'de' }}
    aria-label={germanLabel}
    aria-pressed={value === 'de'}
    lang="de"
    onclick={() => onchange('de')}
  >
    DE
  </button>
</div>

<style>
  .language-switch {
    display: inline-flex;
    align-items: center;
    gap: 0.15rem;
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    background: rgb(7 9 18 / 62%);
    padding: 0.2rem;
    color: var(--fg-4);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    line-height: 1;
  }

  button {
    min-width: 2rem;
    min-height: 1.9rem;
    border: 0;
    border-radius: var(--r-xs);
    background: transparent;
    color: var(--fg-3);
    cursor: pointer;
    font: inherit;
    font-weight: 650;
    letter-spacing: 0.04em;
    transition:
      background-color var(--dur-fast),
      color var(--dur-fast),
      box-shadow var(--dur-fast);
  }

  button:hover {
    color: var(--fg-1);
  }

  button.active {
    background: var(--info-bg);
    box-shadow: inset 0 0 0 1px var(--line-strong);
    color: var(--gem);
  }
</style>
