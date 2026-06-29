<script lang="ts">
  let { title, sub = '' }: { title: string; sub?: string } = $props();
</script>

<header class="topbar">
  <div>
    <div class="title">{title}</div>
    {#if sub}
      <div class="sub">{sub}</div>
    {/if}
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 26px 14px;
    border-bottom: 1px solid var(--line-faint);
  }
  .title {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 700;
    letter-spacing: 0.01em;
    color: var(--fg-1);
  }
  .sub {
    font-family: var(--font-sans);
    font-size: 12.5px;
    color: var(--fg-3);
    margin-top: 2px;
  }
</style>
