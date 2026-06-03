<script>
  /* One notebook entry — session layout or generic entity card. */
  import Icon from "./Icon.svelte";
  import { noteFile } from "./notes-util.js";
  let { item, cat, onOpen } = $props();
  let file = $derived(noteFile(item, cat));
  let num = $derived((item.lead.match(/Session\s+(\d+)/) || [])[1]);
</script>

{#if cat.id === "sessions"}
  <button class="note-card session" onclick={onOpen}>
    <div class="sess-num"><span>{num || "·"}</span></div>
    <div class="sess-body">
      <h3>{item.title}</h3>
      <div class="lead">{item.lead}</div>
      <p>{item.blurb}</p>
      <div class="note-file"><Icon name="file-text" size={11} />{file.name}</div>
    </div>
    <Icon className="go" name="chevron-right" size={18} />
  </button>
{:else}
  <button class="note-card" onclick={onOpen}>
    <div class="nc-top">
      <div class="nc-icon"><Icon name={cat.icon} size={18} /></div>
      <Icon className="go" name="arrow-up-right" size={16} />
    </div>
    <h3>{item.title}</h3>
    <div class="lead">{item.lead}</div>
    <p>{item.blurb}</p>
    <div class="note-file"><Icon name="file-text" size={11} />{file.name}</div>
  </button>
{/if}
