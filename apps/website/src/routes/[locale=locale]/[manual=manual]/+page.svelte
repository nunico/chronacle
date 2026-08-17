<script lang="ts">
  import { getArticle } from '$lib/content/registry';
  import type { PageProps } from './$types';

  let { data }: PageProps = $props();
  const Article = $derived(getArticle(data.locale, data.slug).component);
</script>

<svelte:head>
  <title>{data.title}</title>
  <meta name="description" content={data.summary} />
</svelte:head>

<Article />
