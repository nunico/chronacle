<script>
  /* Features grid. */
  import Icon from "./Icon.svelte";

  const FEATURES = [
    { icon: "gavel", title: "Cited rulings", body: "Every answer leads with a verdict, then the reasoning, then the exact rule it drew from. No bluffing — if it isn't in your sources, it says so." },
    { icon: "scroll-text", title: "Import your tomes", body: "Add official rulebooks, third-party supplements, and your campaign codex from your own files. Indexed on your machine — nothing is uploaded." },
    { icon: "orbit", title: "Fantasy or sci-fi", body: "Spell components or starship hull breaches, psionics or polymorph — Chronacle is genre-agnostic. Any system, any setting, your homebrew included." },
    { icon: "scale", title: "House rules win", body: "Tell Chronacle how your table actually plays. When your house rule contradicts the book, it rules your way and flags it." },
    { icon: "dices", title: "Dice at hand", body: "Roll inline without leaving the conversation. Build a pool, roll, and keep the result attached to the ruling that called for it." },
    { icon: "laptop", title: "Runs on your machine", body: "A desktop app, fully local. No account, no cloud, no telemetry — your sources and your questions never leave your computer." }
  ];
</script>

<section class="section" id="features">
  <div class="wrap">
    <div class="section-head">
      <span class="eyebrow">What it does</span>
      <h2>A learned oracle, bound to your books</h2>
      <p>Chronacle isn't a search box bolted onto a PDF. It reads like a rules lawyer and answers like a generous GM — for any world you run.</p>
    </div>
    <div class="feat-grid">
      {#each FEATURES as f (f.title)}
        <div class="feat">
          <div class="ic-wrap"><Icon name={f.icon} size={24} /></div>
          <h3>{f.title}</h3>
          <p>{f.body}</p>
        </div>
      {/each}
    </div>
  </div>
</section>
