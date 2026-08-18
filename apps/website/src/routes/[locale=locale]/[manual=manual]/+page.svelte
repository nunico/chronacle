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
  <link rel="canonical" href={article.href} />
  <meta property="og:type" content="article" />
  <meta property="og:title" content={data.title} />
  <meta property="og:description" content={data.summary} />
  <meta property="og:image" content="/brand/chronacle-icon.png" />
  <meta property="og:image:alt" content="Chronacle" />
  <meta property="og:locale" content={data.locale === 'de' ? 'de_DE' : 'en_US'} />
  <meta name="twitter:card" content="summary" />
</svelte:head>

<ManualShell {article} />
