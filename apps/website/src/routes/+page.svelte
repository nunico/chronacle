<script lang="ts">
  import type { Pathname } from '$app/types';
  import { onMount } from 'svelte';
  import SiteFooter from '$lib/components/SiteFooter.svelte';
  import SiteHeader from '$lib/components/SiteHeader.svelte';
  import { browserPreferredLocale } from '$lib/i18n/locale';
  import { landingCopy } from '$lib/i18n/landing-copy';
  import type { Locale } from '$lib/i18n/types';
  import DownloadPanel from '$lib/landing/DownloadPanel.svelte';
  import FeatureGrid from '$lib/landing/FeatureGrid.svelte';
  import Hero from '$lib/landing/Hero.svelte';
  import ProductExample from '$lib/landing/ProductExample.svelte';
  import ProviderPrivacy from '$lib/landing/ProviderPrivacy.svelte';
  import Workflow from '$lib/landing/Workflow.svelte';

  const sourceUrl = 'https://github.com/nunico/chronacle';
  const downloadUrl = 'https://github.com/nunico/chronacle/releases/latest';
  const licenseUrl = 'https://github.com/nunico/chronacle/blob/main/LICENSE';

  let locale = $state<Locale>('en');
  let copy = $derived(landingCopy[locale]);
  let manualPath: Pathname = $derived(locale === 'de' ? '/de/handbuch' : '/en/manual');

  onMount(() => {
    locale = browserPreferredLocale(navigator.languages);
  });

  $effect(() => {
    document.documentElement.lang = locale;
    document.title = copy.metadata.title;
  });
</script>

<svelte:head>
  <title>{copy.metadata.title}</title>
  <meta name="description" content={copy.metadata.description} />
</svelte:head>

<SiteHeader
  {locale}
  labels={copy.header}
  links={{ home: '/', manual: manualPath, source: sourceUrl, download: downloadUrl }}
  onlocalechange={(nextLocale) => (locale = nextLocale)}
/>
<Hero copy={copy.hero} {downloadUrl} {manualPath} />
<ProductExample copy={copy.productExample} />
<FeatureGrid copy={copy.features} />
<Workflow copy={copy.workflow} />
<ProviderPrivacy copy={copy.provider} />
<DownloadPanel copy={copy.download} {downloadUrl} {manualPath} />
<SiteFooter
  labels={copy.footer}
  links={{ home: '/', manual: manualPath, source: sourceUrl, license: licenseUrl }}
/>
