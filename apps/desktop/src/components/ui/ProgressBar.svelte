<script lang="ts">
  interface Props {
    value: number;
    label?: string;
    locale?: string;
    showValue?: boolean;
  }

  let { value, label, locale, showValue = true }: Props = $props();

  let clampedValue = $derived(Math.min(100, Math.max(0, value)));
  let percentage = $derived(
    new Intl.NumberFormat(locale, { style: 'percent', maximumFractionDigits: 0 }).format(
      clampedValue / 100,
    ),
  );
</script>

<div class="progress-wrap">
  {#if label || showValue}
    <div class="progress-meta">
      {#if label}<span>{label}</span>{/if}
      {#if showValue}<span class="value" aria-hidden="true">{percentage}</span>{/if}
    </div>
  {/if}
  <div
    class="progress-bar"
    role="progressbar"
    aria-label={label ?? 'Progress'}
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={clampedValue}
  >
    <div class="fill" style:width={`${clampedValue}%`}></div>
  </div>
</div>

<style>
  .progress-wrap {
    display: grid;
    gap: var(--s-1);
  }

  .progress-meta {
    display: flex;
    justify-content: space-between;
    gap: var(--s-2);
    color: var(--fg-2);
    font-size: 0.8125rem;
  }

  .value {
    color: var(--fg-3);
    font-variant-numeric: tabular-nums;
  }

  .progress-bar {
    height: 6px;
    overflow: hidden;
    border-radius: var(--r-full);
    background: var(--line);
  }

  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--grad-arcane);
    transition: width var(--dur) var(--ease-arcane);
  }
</style>
