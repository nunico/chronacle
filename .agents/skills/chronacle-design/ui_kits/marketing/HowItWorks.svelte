<script>
  /* How it works — three steps. */
  const steps = [
    { n: "1", t: "Bind your sources", p: "Drop in rulebooks, supplements, and campaign notes from your own files. Chronacle indexes every page, locally." },
    { n: "2", t: "Ask in plain words", p: "Mid-session, type the question the way you'd say it aloud. No syntax, no page-flipping." },
    { n: "3", t: "Rule and move on", p: "Get the verdict with its citation, drop it in your session log, and get back to the table." }
  ];
</script>

<section class="section" id="how" style="background: rgba(10,12,26,0.4); border-top: 1px solid var(--line-faint); border-bottom: 1px solid var(--line-faint)">
  <div class="wrap">
    <div class="section-head">
      <span class="eyebrow">How it works</span>
      <h2>From shelf to ruling in three steps</h2>
    </div>
    <div class="steps">
      <div class="connector"></div>
      {#each steps as s (s.n)}
        <div class="step">
          <div class="num">{s.n}</div>
          <h3>{s.t}</h3>
          <p>{s.p}</p>
        </div>
      {/each}
    </div>
  </div>
</section>
