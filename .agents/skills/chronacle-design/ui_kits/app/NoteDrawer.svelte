<script>
  /* Detail drawer for a single notebook entry. */
  import Icon from "./Icon.svelte";
  import { noteFile } from "./notes-util.js";
  let { item, cat, onClose } = $props();
  let file = $derived(noteFile(item, cat));
</script>

<div class="drawer-scrim" onclick={onClose} role="presentation"></div>
<div class="drawer">
  <button class="close icon-btn" onclick={onClose}><Icon name="x" size={18} /></button>
  <div class="kind"><Icon name={cat.icon} size={13} />{cat.label}</div>
  <h2>{item.title}</h2>
  {#if item.lead}<div class="drawer-lead">{item.lead}</div>{/if}
  <div class="drawer-file"><Icon name="file-text" size={12} />{file.path}</div>
  <div class="prose">
    {#each item.body as p}<p>{p}</p>{/each}
  </div>
  <div class="meta-card">
    {#each Object.entries(item.meta) as [k, v]}
      <div class="row"><span class="k">{k}</span><span class="v">{v}</span></div>
    {/each}
  </div>
  {#if item.tags && item.tags.length > 0}
    <div class="drawer-tags">
      {#each item.tags as t}<span><Icon name="link" size={11} />{t}</span>{/each}
    </div>
  {/if}
  <button class="ask-btn"><Icon name="sparkles" size={16} />Ask Chronacle about this</button>
</div>
