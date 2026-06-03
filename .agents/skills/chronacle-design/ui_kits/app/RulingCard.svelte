<script>
  /* A single Chronacle ruling: verdict + reasoning + citation pills that unfurl the passage. */
  import Icon from "./Icon.svelte";
  import EyeMark from "./EyeMark.svelte";
  let { data, defaultOpen = false } = $props();
  let open = $state(defaultOpen ? 0 : -1);
</script>

<div class="msg">
  <div class="who-av eye-badge"><EyeMark size={28} /></div>
  <div class="ruling">
    <div class="who">
      <span>Chronacle</span>
      <span class="tag">· ruling</span>
      <div class="ruling-actions">
        <button class="r-act" title="Copy"><Icon name="copy" size={15} /></button>
        <button class="r-act" title="Add to session notes"><Icon name="bookmark-plus" size={15} /></button>
      </div>
    </div>
    <p class="verdict">{data.verdict}</p>
    <p class="why">{@html data.why}</p>
    <div class="cite-row">
      {#each data.cites as c, i}
        <button class="cite" onclick={() => (open = open === i ? -1 : i)}>
          <Icon name="quote" size={14} />
          {c.label}
          <Icon name={open === i ? "chevron-up" : "chevron-down"} size={13} />
        </button>
      {/each}
    </div>
    {#if open >= 0 && data.cites[open]}
      <div class="passage">
        <div class="src">{data.cites[open].src}</div>
        <div class="quote">{data.cites[open].quote}</div>
      </div>
    {/if}
  </div>
</div>
