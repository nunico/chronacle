<script lang="ts">
  import { onMount } from 'svelte';
  import { initLocale } from './lib/locale.svelte';
  import ModelDownload from './ModelDownload.svelte';
  import Shell from './shell/Shell.svelte';

  let modelReady = $state(false);

  function onModelReady() {
    modelReady = true;
  }

  onMount(() => {
    initLocale();
  });
</script>

{#if !modelReady}
  <ModelDownload {onModelReady} />
{:else}
  <Shell />
{/if}
