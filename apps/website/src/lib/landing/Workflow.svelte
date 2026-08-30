<script lang="ts">
  import type { LandingCopy } from '$lib/i18n/landing-copy';

  interface Props {
    copy: LandingCopy['workflow'];
  }

  let { copy }: Props = $props();
</script>

<section class="workflow" aria-labelledby="workflow-heading">
  <div class="workflow__inner">
    <header>
      <p>{copy.eyebrow}</p>
      <h2 id="workflow-heading">{copy.heading}</h2>
    </header>
    <ol>
      {#each copy.items as step (step.number)}
        <li>
          <span>{step.number}</span>
          <h3>{step.title}</h3>
          <p>{step.body}</p>
        </li>
      {/each}
    </ol>
  </div>
</section>

<style>
  .workflow {
    padding: var(--s-20) var(--s-4);
  }

  .workflow__inner {
    width: min(100%, var(--content-width));
    margin: 0 auto;
  }

  header {
    margin-bottom: var(--s-12);
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
    margin: 0;
    font-size: clamp(2rem, 4vw, 2.75rem);
  }

  ol {
    position: relative;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--s-8);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  ol::before {
    position: absolute;
    height: 1px;
    top: 1.5rem;
    right: 15%;
    left: 15%;
    background: linear-gradient(90deg, transparent, var(--line-glow), transparent);
    content: '';
  }

  li {
    position: relative;
    text-align: center;
  }

  li > span {
    position: relative;
    z-index: 1;
    display: inline-flex;
    width: 3rem;
    height: 3rem;
    align-items: center;
    justify-content: center;
    margin-bottom: var(--s-5);
    border: 1px solid var(--line-strong);
    border-radius: 50%;
    background: var(--bg-panel);
    color: var(--gem);
    font-family: var(--font-mono);
  }

  h3 {
    margin: 0 0 var(--s-2);
    font-size: 1.12rem;
  }

  li p {
    margin: 0;
    color: var(--fg-2);
    font-family: var(--font-serif);
    line-height: 1.6;
  }

  @media (max-width: 44rem) {
    ol {
      grid-template-columns: 1fr;
    }

    ol::before {
      display: none;
    }
  }
</style>
