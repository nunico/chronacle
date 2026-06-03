<script>
  /* Open source / download panel. */
  import Icon from "./Icon.svelte";

  const platforms = [
    { ic: "apple", label: "macOS", note: "Universal · .dmg" },
    { ic: "monitor", label: "Windows", note: "10/11 · .exe" },
    { ic: "terminal", label: "Linux", note: ".AppImage · .deb" }
  ];
</script>

<section class="section" id="oss">
  <div class="wrap">
    <div class="oss-panel">
      <span class="eyebrow" style="display: block; margin-bottom: 14px">Free &amp; open source</span>
      <h2 class="oss-h2">Yours to run, read, and reshape</h2>
      <p class="oss-lede">Chronacle is released under the <b>AGPL-3.0</b> license. Download a build, or clone the repo and make it your own. It runs entirely on your machine — no account, no servers, no telemetry.</p>

      <div class="oss-actions">
        <a class="btn btn-primary btn-lg" href="#"><Icon name="download" size={18} />Download free</a>
        <a class="btn btn-outline btn-lg" href="#"><Icon name="star" size={17} />Star on GitHub</a>
      </div>

      <div class="platforms">
        {#each platforms as p (p.label)}
          <a class="plat" href="#">
            <Icon className="ic" name={p.ic} size={20} />
            <span><b>{p.label}</b><em>{p.note}</em></span>
          </a>
        {/each}
      </div>

      <div class="oss-meta">
        <span><Icon name="scale" size={14} />AGPL-3.0</span>
        <span><Icon name="wifi-off" size={14} />Works offline</span>
        <span><Icon name="lock" size={14} />No account, no cloud</span>
        <span><Icon name="git-fork" size={14} />Fork it freely</span>
      </div>
    </div>
  </div>
</section>
