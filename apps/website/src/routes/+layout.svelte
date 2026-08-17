<script lang="ts">
  import type { Snippet } from 'svelte';
  import '../app.css';

  let { children }: { children: Snippet } = $props();
</script>

<div class="site-background" aria-hidden="true"></div>
<main id="main-content">{@render children()}</main>
