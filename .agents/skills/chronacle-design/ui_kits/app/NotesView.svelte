<script>
  /* Notebook: a category of file-backed notes (sessions/ + entities/<type>/*.md) + detail drawer. */
  import { CHRONACLE as D } from "./data.js";
  import Icon from "./Icon.svelte";
  import NoteCard from "./NoteCard.svelte";
  import NoteDrawer from "./NoteDrawer.svelte";

  let { category } = $props();

  let cat = $derived(D.categories.find((c) => c.id === category) || {});
  let items = $derived(D.notes[category] || []);
  let open = $state(null);
  let query = $state("");

  let filtered = $derived(
    items.filter(
      (it) =>
        !query ||
        it.title.toLowerCase().includes(query.toLowerCase()) ||
        it.blurb.toLowerCase().includes(query.toLowerCase())
    )
  );

  // Reset the search + drawer whenever we switch category.
  $effect(() => {
    category; // track
    open = null;
    query = "";
  });
</script>

<div class="scroll" style="position: relative">
  <div class="notes">
    <div class="notes-head">
      <div>
        <h1>{cat.label}</h1>
        <p>{cat.sub}</p>
        <div class="notes-path"><Icon name="folder" size={12} />{cat.folder}/</div>
      </div>
      <div class="notes-tools">
        <div class="notes-search">
          <Icon name="search" size={16} />
          <input bind:value={query} placeholder={"Search " + (cat.label || "").toLowerCase()} />
        </div>
        <button class="notes-add"><Icon name="plus" size={16} />New</button>
      </div>
    </div>

    <div class="notes-grid{category === 'sessions' ? ' sessions' : ''}">
      {#each filtered as it (it.title)}
        <NoteCard item={it} {cat} onOpen={() => (open = it)} />
      {/each}
      {#if filtered.length === 0 && items.length > 0}
        <div class="notes-empty"><Icon name="search-x" size={22} /><span>Nothing matches "{query}".</span></div>
      {/if}
      {#if items.length === 0}
        <div class="notes-empty"><Icon name="folder-open" size={22} /><span>No notes in {cat.folder}/ yet.</span></div>
      {/if}
    </div>
  </div>

  {#if open}
    <NoteDrawer item={open} {cat} onClose={() => (open = null)} />
  {/if}
</div>
