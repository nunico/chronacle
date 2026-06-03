<script>
  /* App shell: rail + top bar + active view. */
  import { CHRONACLE as D } from "./data.js";
  import CampaignRail from "./CampaignRail.svelte";
  import Icon from "./Icon.svelte";
  import OracleView from "./OracleView.svelte";
  import NotesView from "./NotesView.svelte";
  import CampaignView from "./CampaignView.svelte";

  let view = $state("oracle");

  const catTitles = Object.fromEntries(D.categories.map((c) => [c.id, { t: c.label, s: c.sub }]));
  const titles = {
    oracle: { t: "Oracle", s: "Ask in plain language — answers come cited" },
    campaign: { t: "Campaign", s: "Manage details & subscribed source collections" },
    ...catTitles
  };
  const NOTE_CATS = D.categories.map((c) => c.id);

  let head = $derived(titles[view]);
</script>

<div class="app">
  <CampaignRail {view} setView={(v) => (view = v)} />
  <main class="main">
    <header class="topbar">
      <div>
        <div class="title">{head.t}</div>
        <div class="sub">{head.s}</div>
      </div>
      <div class="spacer"></div>
      <button class="icon-btn" title="Search"><Icon name="search" size={18} /></button>
      <button class="icon-btn" title="New thread"><Icon name="square-pen" size={18} /></button>
    </header>

    {#if view === "oracle"}
      <OracleView />
    {:else if NOTE_CATS.includes(view)}
      <NotesView category={view} />
    {:else if view === "campaign"}
      <CampaignView />
    {/if}
  </main>
</div>
