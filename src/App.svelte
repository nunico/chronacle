<script lang="ts">
  import ModelDownload from './ModelDownload.svelte';
  import Shell from './shell/Shell.svelte';

  let modelReady = $state(false);

  function onModelReady() {
    modelReady = true;
  }
</script>

{#if !modelReady}
  <ModelDownload {onModelReady} />
{:else}
  <Shell />
{/if}
