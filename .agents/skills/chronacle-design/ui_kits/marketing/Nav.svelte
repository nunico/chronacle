<script>
  /* Top navigation bar. */
  import Icon from "./Icon.svelte";
</script>

<nav class="nav">
  <div class="wrap nav-in">
    <a class="brand" href="#top">
      <span class="mark"></span>
      <span class="word">Chron<b>a</b>cle</span>
    </a>
    <div class="nav-links">
      <a href="#features">Features</a>
      <a href="#how">How it works</a>
      <a href="#oss">Open source</a>
      <a href="#">Docs</a>
    </div>
    <div class="nav-cta">
      <a class="btn btn-ghost" href="#"><Icon name="git-fork" size={16} />Source</a>
      <a class="btn btn-primary" href="#oss"><Icon name="download" size={16} />Download</a>
    </div>
  </div>
</nav>
