<script lang="ts">
  import { resolve } from '$app/paths';
  import EyeMark from '$lib/brand/EyeMark.svelte';
  import Icon from '$lib/components/Icon.svelte';
  import type { LandingCopy } from '$lib/i18n/landing-copy';
  import { OPEN_GAME_LICENSE_ROUTE } from '$lib/legal/open-game-content';
  import Sparkles from 'lucide-svelte/icons/sparkles';

  interface Props {
    copy: LandingCopy['productExample'];
  }

  let { copy }: Props = $props();
</script>

<section class="example" aria-labelledby="example-label">
  <div class="example__inner">
    <h2 class="example__label" id="example-label">{copy.label}</h2>
    <div class="window" role="group" aria-label={copy.windowLabel}>
      <div class="window__bar" aria-hidden="true">
        <span></span><span></span><span></span>
      </div>
      <div class="window__question">
        <Icon icon={Sparkles} size={17} />
        <div>
          <span>{copy.questionLabel}</span>
          <p>{copy.question}</p>
        </div>
      </div>
      <article class="answer">
        <header>
          <span class="answer__eye"><EyeMark size={24} glow={false} /></span>
          <strong>{copy.assistant}</strong>
          <span>{copy.answerLabel}</span>
        </header>
        <div class="answer__open-game" data-open-game-content>
          <a class="open-game-marker" href={resolve(OPEN_GAME_LICENSE_ROUTE)}>{copy.metadata}</a>
          <p class="answer__verdict">{copy.verdict}</p>
          <p class="answer__body">{copy.answer}</p>
          <div class="citation">
            <div class="citation__title">
              <span>{copy.citationLabel}</span>
              <strong>{copy.citation}</strong>
            </div>
            <blockquote>{copy.excerpt}</blockquote>
          </div>
        </div>
      </article>
    </div>
  </div>
</section>

<style>
  .example {
    padding: var(--s-4) var(--s-4) var(--s-20);
  }

  .example__inner {
    width: min(100%, 49rem);
    margin: 0 auto;
  }

  .example__label {
    margin: 0 0 var(--s-3);
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-align: center;
    text-transform: uppercase;
  }

  .window {
    position: relative;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-xl);
    background: linear-gradient(180deg, var(--bg-panel), rgb(10 12 26 / 82%));
    padding: var(--s-3);
    box-shadow: var(--shadow-3);
  }

  .window__bar {
    display: flex;
    gap: 0.45rem;
    padding: 0.3rem 0.45rem 2.675rem;
  }

  .open-game-marker {
    position: absolute;
    top: 2.45rem;
    left: 50%;
    width: fit-content;
    max-width: calc(100% - 2rem);
    color: var(--rune-gold);
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.04em;
    text-align: center;
    text-decoration-color: rgb(232 184 106 / 48%);
    transform: translateX(-50%);
    white-space: nowrap;
  }

  .window__bar span {
    width: 0.6rem;
    height: 0.6rem;
    border-radius: 50%;
    background: var(--line-strong);
  }

  .window__question {
    display: flex;
    align-items: flex-start;
    gap: var(--s-3);
    margin-bottom: var(--s-3);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--bg-inset);
    padding: var(--s-4);
    color: var(--violet-300);
  }

  .window__question span,
  .answer header span {
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.04em;
  }

  .window__question p {
    margin: 0.15rem 0 0;
    color: var(--fg-2);
    font-family: var(--font-sans);
    line-height: 1.45;
  }

  .answer {
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background:
      linear-gradient(90deg, transparent, var(--line-glow), transparent) top center / 70% 1px
        no-repeat,
      var(--bg-abyss);
    padding: clamp(1rem, 4vw, 1.5rem);
  }

  .answer header {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin-bottom: var(--s-3);
    font-family: var(--font-sans);
  }

  .answer__eye {
    display: inline-flex;
    width: 2rem;
    height: 2rem;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--line-strong);
    border-radius: 50%;
    background: var(--bg-inset);
  }

  .answer header strong {
    font-size: 0.82rem;
  }

  .answer__verdict {
    margin: 0 0 var(--s-2);
    color: var(--gem);
    font-family: var(--font-serif);
    font-size: 1.15rem;
    font-weight: 650;
  }

  .answer__body {
    margin: 0 0 var(--s-4);
    color: var(--fg-2);
    font-family: var(--font-serif);
    line-height: 1.6;
  }

  .citation {
    border: 1px solid var(--line-glow);
    border-radius: var(--r-md);
    background: var(--info-bg);
    padding: var(--s-3) var(--s-4);
    box-shadow: var(--glow-arcane);
  }

  .citation__title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-2);
    color: var(--arcane-300);
    font-family: var(--font-mono);
    font-size: 0.72rem;
  }

  .citation__title strong {
    color: var(--gem);
    font-weight: 550;
  }

  blockquote {
    margin: var(--s-2) 0 0;
    color: var(--fg-2);
    font-family: var(--font-serif);
    font-size: 0.9rem;
    line-height: 1.55;
  }

  @media (max-width: 31rem) {
    .window__bar {
      padding-bottom: 3.8rem;
    }

    .open-game-marker {
      white-space: normal;
    }
  }
</style>
