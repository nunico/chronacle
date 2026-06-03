<script>
  /* Hero + live demo ruling card. */
  import Icon from "./Icon.svelte";
  import EyeMark from "./EyeMark.svelte";
</script>

<header class="hero" id="top">
  <div class="wrap">
    <div class="halo"></div>
    <div class="eyebrow-pill"><Icon name="sparkles" size={14} />Local-first RAG oracle · open source</div>
    <h1>The rules are written. <span class="g">Now they answer.</span></h1>
    <p class="lede">Chronacle reads your rulebooks and campaign lore — fantasy or sci-fi — then settles questions mid-session in plain language, and shows you the exact passage it ruled from. It all runs on your own machine.</p>
    <div class="hero-cta">
      <a class="btn btn-primary btn-lg" href="#oss"><Icon name="download" size={18} />Download free</a>
      <a class="btn btn-outline btn-lg" href="#"><Icon name="git-fork" size={17} />View source</a>
    </div>
    <div class="trust">AGPL-3.0 · macOS, Windows &amp; Linux · no account · your books never leave your computer</div>

    <div class="demo">
      <div class="demo-frame">
        <div class="demo-bar"><span class="lt"></span><span class="lt"></span><span class="lt"></span></div>
        <div class="demo-q"><Icon className="star" name="sparkles" size={17} style="color:var(--violet-400)" />Can I cast a spell while I'm grappled?</div>
        <div class="demo-ruling">
          <div class="who"><span class="eye"><EyeMark size={22} /></span><b>Chronacle</b><span>· ruling</span></div>
          <p class="verdict">Yes — but at disadvantage.</p>
          <p class="why">Being grappled sets your speed to 0 but doesn't stop you casting. A somatic component still works; the condition imposes disadvantage on any attack roll the spell requires.</p>
          <span class="cite"><Icon name="quote" size={14} />Cited from SRD 5.2 · Grappling</span>
        </div>
      </div>
    </div>
  </div>
</header>
