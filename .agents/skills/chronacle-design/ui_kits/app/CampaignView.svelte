<script>
  /* Campaign management: overview + subscribed source collections with toggles. */
  import { CHRONACLE as D } from "./data.js";
  import Icon from "./Icon.svelte";

  let subs = $state(Object.fromEntries(D.collections.map((c) => [c.name, c.subscribed])));
  let open = $state(D.collections.filter((c) => c.subscribed).map((c) => c.name));

  const toggleOpen = (name) =>
    (open = open.includes(name) ? open.filter((n) => n !== name) : [...open, name]);
  const toggleSub = (name, e) => {
    e.stopPropagation();
    subs = { ...subs, [name]: !subs[name] };
  };

  let subCount = $derived(Object.values(subs).filter(Boolean).length);
  let bookCount = $derived(
    D.collections.filter((c) => subs[c.name]).reduce((n, c) => n + c.books.length, 0)
  );
  const noteCount = ["player_characters", "npcs", "locations", "factions", "creatures", "items", "events", "misc"].reduce(
    (n, k) => n + (D.notes[k] || []).length,
    0
  );
  const sessionCount = (D.notes.sessions || []).length;
</script>

<div class="scroll">
  <div class="campaign-view">
    <section class="cv-hero">
      <div class="cv-gem"></div>
      <div>
        <div class="cv-eyebrow">Campaign</div>
        <h1>{D.campaign.name}</h1>
        <p class="cv-meta">{D.campaign.system} · {D.campaign.session}</p>
      </div>
      <button class="cv-edit"><Icon name="pencil" size={15} />Edit details</button>
    </section>

    <div class="cv-stats">
      <div class="cv-stat"><span class="n">{subCount}</span><span class="l">collections</span></div>
      <div class="cv-stat"><span class="n">{bookCount}</span><span class="l">books indexed</span></div>
      <div class="cv-stat"><span class="n">{noteCount}</span><span class="l">notebook entries</span></div>
      <div class="cv-stat"><span class="n">{sessionCount}</span><span class="l">sessions logged</span></div>
    </div>

    <section class="cv-section">
      <div class="cv-section-head">
        <div>
          <h2>Source collections</h2>
          <p>Subscribe this campaign to the rulebooks and lore it should draw from. Collections are shared across campaigns; subscribing is per-campaign.</p>
        </div>
        <button class="cv-add"><Icon name="plus" size={16} />Add collection</button>
      </div>

      <div class="cv-collections">
        {#each D.collections as c (c.name)}
          {@const on = subs[c.name]}
          {@const isOpen = open.includes(c.name)}
          <div class="cv-coll{on ? ' on' : ''}">
            <div class="cv-coll-head" onclick={() => toggleOpen(c.name)} role="presentation">
              <div class="cv-coll-icon"><Icon name={c.icon} size={18} /></div>
              <div class="cv-coll-text">
                <div class="nm">{c.name}</div>
                <div class="ct">{c.books.length} {c.books.length === 1 ? "book" : "books"} · {on ? "subscribed" : "not subscribed"}</div>
              </div>
              <span
                class="sub-toggle{on ? ' on' : ''}"
                role="switch"
                aria-checked={on}
                tabindex="0"
                title={on ? "Subscribed — click to remove" : "Click to subscribe"}
                onclick={(e) => toggleSub(c.name, e)}
                onkeydown={(e) => (e.key === "Enter" || e.key === " ") && toggleSub(c.name, e)}
              >
                <span class="knob"></span>
              </span>
              <button class="cv-coll-chev" onclick={(e) => { e.stopPropagation(); toggleOpen(c.name); }}>
                <Icon name={isOpen ? "chevron-up" : "chevron-down"} size={16} />
              </button>
            </div>
            {#if isOpen}
              <div class="cv-books">
                {#each c.books as b, i (i)}
                  <div class="cv-book">
                    <Icon name="file-text" size={14} className="bic" />
                    <span class="bnm">{b.name}</span>
                    <span class="book-status {b.status === 'ok' ? 'ok' : b.status === 'idx' ? 'idx' : 'off'}">
                      {b.status === "ok" ? "Indexed" : b.status === "idx" ? "Indexing…" : "Inactive"}
                    </span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </section>
  </div>
</div>
