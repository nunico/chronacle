<script lang="ts">
  import Icon from '$lib/components/Icon.svelte';
  import type { FeatureCopy, LandingCopy } from '$lib/i18n/landing-copy';
  import BookOpen from 'lucide-svelte/icons/book-open';
  import NotebookPen from 'lucide-svelte/icons/notebook-pen';
  import Quote from 'lucide-svelte/icons/quote';

  interface Props {
    copy: LandingCopy['features'];
  }

  const icons = {
    'book-open': BookOpen,
    notebook: NotebookPen,
    quote: Quote,
  } satisfies Record<FeatureCopy['icon'], typeof BookOpen>;

  let { copy }: Props = $props();
</script>

<section class="features" aria-labelledby="features-heading">
  <div class="features__inner">
    <header>
      <p>{copy.eyebrow}</p>
      <h2 id="features-heading">{copy.heading}</h2>
      <span>{copy.body}</span>
    </header>
    <div class="features__grid">
      {#each copy.items as feature (feature.title)}
        <article>
          <span class="feature__icon"><Icon icon={icons[feature.icon]} size={24} /></span>
          <h3>{feature.title}</h3>
          <p>{feature.body}</p>
        </article>
      {/each}
    </div>
  </div>
</section>

<style>
  .features {
    padding: var(--s-20) var(--s-4);
    border-block: 1px solid var(--line-faint);
    background: rgb(10 12 26 / 42%);
  }

  .features__inner {
    width: min(100%, var(--content-width));
    margin: 0 auto;
  }

  header {
    max-width: 42rem;
    margin: 0 auto var(--s-12);
    text-align: center;
  }

  header p {
    margin: 0 0 var(--s-3);
    color: var(--arcane-300);
    font-family: var(--font-sans);
    font-size: 0.75rem;
    font-weight: 650;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0 0 var(--s-4);
    font-size: clamp(2rem, 4vw, 2.75rem);
  }

  header span {
    color: var(--fg-2);
    font-size: 1.05rem;
  }

  .features__grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--s-4);
  }

  article {
    border: 1px solid var(--line);
    border-radius: var(--r-xl);
    background: var(--bg-panel);
    padding: clamp(1.25rem, 3vw, 1.75rem);
    box-shadow: var(--shadow-card);
  }

  .feature__icon {
    display: inline-flex;
    width: 3rem;
    height: 3rem;
    align-items: center;
    justify-content: center;
    margin-bottom: var(--s-5);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--info-bg);
    color: var(--violet-300);
  }

  h3 {
    margin: 0 0 var(--s-2);
    font-size: 1.15rem;
  }

  article p {
    margin: 0;
    color: var(--fg-2);
    font-family: var(--font-serif);
    line-height: 1.6;
  }

  @media (max-width: 48rem) {
    .features__grid {
      grid-template-columns: 1fr;
    }
  }
</style>
