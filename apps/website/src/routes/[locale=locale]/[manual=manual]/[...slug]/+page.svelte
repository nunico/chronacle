<script lang="ts">
  import { getArticle } from '$lib/content/registry';
  import ManualShell from '$lib/manual/ManualShell.svelte';
  import type { PageProps } from './$types';

  let { data }: PageProps = $props();
  const article = $derived(getArticle(data.locale, data.slug));
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta name="description" content={data.summary} />
</svelte:head>

<ManualShell {article} />
