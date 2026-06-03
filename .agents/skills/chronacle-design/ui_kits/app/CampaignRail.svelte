<script>
  /* Left campaign rail: brand, campaign switcher (opens management), nav. */
  import { CHRONACLE as D } from "./data.js";
  import Icon from "./Icon.svelte";
  let { view, setView } = $props();
  const groups = ["Notebook", "Entities"];
</script>

<aside class="rail">
  <div class="rail-head">
    <div class="rail-mark"></div>
    <div class="rail-word">Chron<b>a</b>cle</div>
  </div>

  <button
    class="campaign{view === 'campaign' ? ' active' : ''}"
    title="Manage campaign & collections"
    onclick={() => setView("campaign")}
  >
    <div class="gem"></div>
    <div class="campaign-text">
      <div class="nm">{D.campaign.name}</div>
      <div class="mt">{D.campaign.system}</div>
    </div>
    <Icon className="chev" name="chevrons-up-down" size={15} />
  </button>

  <nav class="nav">
    {#each D.nav as n (n.id)}
      <button class="nav-item{view === n.id ? ' active' : ''}" onclick={() => setView(n.id)}>
        <Icon className="ic" name={n.icon} size={18} />
        {n.label}
        {#if n.count}<span class="ct">{n.count}</span>{/if}
      </button>
    {/each}
  </nav>

  <div class="rail-scroll">
    {#each groups as group (group)}
      <div class="rail-section">{group}</div>
      <nav class="nav">
        {#each D.categories.filter((c) => c.group === group) as n (n.id)}
          <button class="nav-item{view === n.id ? ' active' : ''}" onclick={() => setView(n.id)}>
            <Icon className="ic" name={n.icon} size={18} />
            {n.label}
            <span class="ct">{(D.notes[n.id] || []).length}</span>
          </button>
        {/each}
      </nav>
    {/each}
  </div>

  <div class="rail-foot">
    <button class="manage-btn{view === 'campaign' ? ' active' : ''}" onclick={() => setView("campaign")}>
      <Icon name="library" size={16} />
      Campaign &amp; sources
    </button>
  </div>
</aside>
