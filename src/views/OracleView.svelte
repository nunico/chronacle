<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    chatSend,
    getChatHistory,
    getChunkForCitation,
    type CitationChunk,
  } from '../lib/commands';
  import Icon from '../components/Icon.svelte';
  import EyeMark from '../components/EyeMark.svelte';
  import RulingCard from '../components/RulingCard.svelte';
  import { renderContent, parseRuling, splitHeading } from './ruling-parse';

  let {
    activeCampaignId,
    onOpenUpload,
  }: {
    activeCampaignId: string | null;
    onOpenUpload: () => void;
  } = $props();

  let messages = $state<Array<{ role: string; content: string }>>([]);
  let input = $state('');
  let isLoading = $state(false);
  let currentResponse = $state('');
  let unlistenListener: UnlistenFn | null = null;
  let scrollEl = $state<HTMLDivElement | undefined>(undefined);

  let citationPopover = $state<
    | {
        source: string;
        page: number | null;
        quote: string | null;
        chunk: CitationChunk | null;
        loading: boolean;
        x: number;
        y: number;
      }
    | null
  >(null);

  const suggestions = [
    { icon: 'swords', text: 'Can I cast a spell while grappled?' },
    { icon: 'shield', text: 'How does cover affect spell attacks?' },
    { icon: 'dices', text: 'Roll initiative for the party' },
    { icon: 'book-open', text: "What's in the rulebook I just uploaded?" },
  ];

  async function loadHistory(campaignId: string | null) {
    try {
      const history = await getChatHistory(campaignId);
      messages = history;
    } catch (e) {
      console.error('Failed to load chat history:', e);
    }
  }

  // Refetch when the active campaign changes.
  $effect(() => {
    loadHistory(activeCampaignId);
  });

  // Auto-scroll thread on new messages or while streaming.
  $effect(() => {
    void messages;
    void currentResponse;
    void isLoading;
    if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
  });

  onMount(async () => {
    unlistenListener = await listen<{ token: string; done: boolean }>('chat-token', (event) => {
      if (event.payload.done) {
        if (currentResponse) {
          messages = [...messages, { role: 'assistant', content: currentResponse }];
        }
        currentResponse = '';
        isLoading = false;
      } else {
        currentResponse += event.payload.token;
      }
    });
  });

  onDestroy(() => {
    if (unlistenListener) unlistenListener();
  });

  async function sendMessage(text?: string) {
    const t = (text ?? input).trim();
    if (!t || isLoading) return;
    messages = [...messages, { role: 'user', content: t }];
    input = '';
    isLoading = true;
    currentResponse = '';
    try {
      await chatSend(t, activeCampaignId);
    } catch (e) {
      console.error('Chat send failed:', e);
      isLoading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  async function handleThreadClick(event: MouseEvent) {
    const target = (event.target as HTMLElement | null)?.closest('.citation-badge');
    if (!(target instanceof HTMLElement)) return;
    event.stopPropagation();
    const source = target.dataset.source ?? '';
    const pageStr = target.dataset.page;
    const page = pageStr ? parseInt(pageStr, 10) : null;
    const inlineQuote = target.dataset.quote ?? null;
    const rect = target.getBoundingClientRect();

    if (inlineQuote) {
      citationPopover = {
        source,
        page,
        quote: inlineQuote,
        chunk: null,
        loading: false,
        x: rect.left,
        y: rect.bottom + 6,
      };
      return;
    }
    citationPopover = {
      source,
      page,
      quote: null,
      chunk: null,
      loading: true,
      x: rect.left,
      y: rect.bottom + 6,
    };
    try {
      const chunk = await getChunkForCitation(source, page);
      if (citationPopover && citationPopover.source === source && citationPopover.page === page) {
        citationPopover = { ...citationPopover, chunk, loading: false };
      }
    } catch (e) {
      console.error('Failed to load citation chunk:', e);
      if (citationPopover && citationPopover.source === source && citationPopover.page === page) {
        citationPopover = { ...citationPopover, chunk: null, loading: false };
      }
    }
  }

  function handleWindowClick(event: MouseEvent) {
    if (!citationPopover) return;
    const t = event.target as HTMLElement | null;
    if (t?.closest('.citation-popover') || t?.closest('.citation-badge')) return;
    citationPopover = null;
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') citationPopover = null;
  }

  function hasCitation(text: string): boolean {
    return /\[Source:\s*"/.test(text);
  }

  function plainHtml(text: string): string {
    return renderContent(text);
  }
</script>

<svelte:window onclick={handleWindowClick} onkeydown={handleWindowKeydown} />

<div class="scroll" bind:this={scrollEl}>
  <div class="thread" onclick={handleThreadClick} role="presentation">
    {#each messages as msg, i (i)}
      {#if msg.role === 'user'}
        <div class="msg user">
          <div class="bubble">{msg.content}</div>
          <div class="who-av">GM</div>
        </div>
      {:else if hasCitation(msg.content)}
        <RulingCard data={parseRuling(msg.content)} />
      {:else}
        <div class="msg">
          <div class="who-av eye-badge"><EyeMark size={28} /></div>
          <div class="plain">
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            {@html plainHtml(msg.content)}
          </div>
        </div>
      {/if}
    {/each}

    {#if isLoading && currentResponse}
      <div class="msg">
        <div class="who-av eye-badge"><EyeMark size={28} /></div>
        <div class="plain streaming">
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          {@html plainHtml(currentResponse)}
        </div>
      </div>
    {/if}

    {#if isLoading && !currentResponse}
      <div class="msg">
        <div class="who-av eye-badge"><EyeMark size={28} /></div>
        <div class="thinking">
          <span class="tdot"></span><span class="tdot"></span><span class="tdot"></span>
          <span class="label">consulting your tomes…</span>
        </div>
      </div>
    {/if}

    {#if messages.length === 0 && !isLoading}
      <div class="suggest">
        {#each suggestions as s (s.text)}
          <button class="sug" onclick={() => sendMessage(s.text)}>
            <Icon name={s.icon} size={15} />
            {s.text}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

{#if citationPopover}
  <div
    class="citation-popover"
    style="left: {citationPopover.x}px; top: {citationPopover.y}px"
    role="dialog"
    aria-label="Source passage"
  >
    <div class="popover-header">
      <strong>{citationPopover.source}</strong>
      {#if citationPopover.page !== null}
        <span class="muted">p.{citationPopover.page}</span>
      {/if}
      <button
        type="button"
        class="popover-close"
        aria-label="Close"
        onclick={() => (citationPopover = null)}>×</button>
    </div>
    {#if citationPopover.quote}
      {@const split = splitHeading(citationPopover.quote)}
      {#if split.heading}
        <div class="popover-heading">{split.heading}</div>
      {/if}
      <div class="popover-body popover-quote">"{split.body}"</div>
    {:else if citationPopover.loading}
      <div class="popover-body muted">Loading…</div>
    {:else if citationPopover.chunk}
      {#if citationPopover.chunk.section_heading}
        <div class="popover-heading">{citationPopover.chunk.section_heading}</div>
      {/if}
      <div class="popover-body">{citationPopover.chunk.text}</div>
    {:else}
      <div class="popover-body muted">No supporting quote available.</div>
    {/if}
  </div>
{/if}

<div class="composer-wrap">
  <div class="composer">
    <Icon name="sparkles" size={20} />
    <input
      bind:value={input}
      onkeydown={handleKeydown}
      placeholder="Ask a rule, a name, a place…"
      disabled={isLoading}
    />
    <button class="tool" onclick={onOpenUpload} title="Attach a rulebook" aria-label="Attach a rulebook">
      <Icon name="paperclip" size={18} />
    </button>
    <button class="tool" title="Roll — coming soon" aria-label="Roll dice" disabled>
      <Icon name="dices" size={18} />
    </button>
    <button
      class="send-btn"
      disabled={!input.trim() || isLoading}
      onclick={() => sendMessage()}
      aria-label="Send"
    >
      <Icon name="arrow-up" size={18} />
    </button>
  </div>
</div>

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 18px 26px 8px;
  }
  .thread {
    max-width: 760px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
  }
  .msg {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    margin: 14px 0;
  }
  .msg.user {
    justify-content: flex-end;
  }
  .msg.user .bubble {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 10px 14px;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 14.5px;
    max-width: 70%;
  }
  .who-av {
    flex: none;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 13px;
    color: var(--fg-2);
  }
  .who-av.eye-badge {
    background: var(--bg-inset);
  }
  .plain {
    flex: 1;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 12px 14px;
    font-family: var(--font-serif);
    font-size: 16px;
    color: var(--fg-2);
    line-height: 1.65;
    white-space: pre-wrap;
    word-wrap: break-word;
    box-shadow: var(--shadow-card);
  }
  .streaming::after {
    content: '▊';
    color: var(--arcane-300);
    animation: blink 0.8s step-end infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .thinking {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 12px 14px;
    color: var(--fg-3);
    font-family: var(--font-sans);
    font-size: 13.5px;
  }
  .tdot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--arcane-300);
    box-shadow: var(--glow-arcane);
    animation: tdot 1s var(--ease-arcane) infinite;
  }
  .tdot:nth-child(2) {
    animation-delay: 0.15s;
  }
  .tdot:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes tdot {
    0%, 60%, 100% { opacity: 0.35; transform: translateY(0); }
    30% { opacity: 1; transform: translateY(-2px); }
  }
  .suggest {
    margin-top: 24px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: center;
  }
  .sug {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-full);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .sug:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .composer-wrap {
    padding: 12px 26px 20px;
  }
  .composer {
    max-width: 760px;
    margin: 0 auto;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 8px 14px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-full);
    box-shadow: var(--shadow-card);
  }
  .composer:focus-within {
    border-color: var(--line-glow);
    box-shadow: var(--glow-arcane);
  }
  .composer input {
    flex: 1;
    border: 0;
    background: transparent;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 15px;
    padding: 8px 0;
  }
  .composer input:focus {
    outline: none;
  }
  .composer input::placeholder {
    color: var(--fg-3);
  }
  .tool {
    padding: 8px;
    border-radius: var(--r-md);
    border: 0;
    background: none;
    color: var(--fg-3);
  }
  .tool:hover:not(:disabled) {
    color: var(--fg-1);
  }
  .tool:disabled {
    opacity: 0.45;
  }
  .send-btn {
    padding: 8px 12px;
    border-radius: var(--r-full);
    border: 0;
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
  }
  .send-btn:disabled {
    opacity: 0.5;
    box-shadow: none;
    background: var(--bg-panel-2);
    color: var(--fg-3);
  }
  /* Citation popover (lifted) */
  .citation-popover {
    position: fixed;
    z-index: 100;
    max-width: min(440px, 90vw);
    background: rgba(16, 19, 42, 0.85);
    color: var(--fg-1);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    backdrop-filter: blur(14px);
    box-shadow: var(--shadow-3);
    overflow: hidden;
  }
  .popover-header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-panel);
    font-family: var(--font-mono);
    font-size: 12.5px;
  }
  .popover-header .muted {
    color: var(--fg-3);
    font-size: 12px;
  }
  .popover-close {
    margin-left: auto;
    background: transparent;
    color: var(--fg-3);
    border: 0;
    font-size: 16px;
    line-height: 1;
  }
  .popover-close:hover {
    color: var(--fg-1);
  }
  .popover-heading {
    padding: 6px 12px 0;
    font-size: 12px;
    color: var(--fg-3);
    font-style: italic;
  }
  .popover-body {
    padding: 10px 12px 12px;
    font-family: var(--font-serif);
    font-size: 14px;
    line-height: 1.5;
    max-height: 320px;
    overflow-y: auto;
    white-space: pre-wrap;
  }
  .popover-body.muted {
    color: var(--fg-3);
    font-style: italic;
  }
  .popover-quote {
    font-style: italic;
    color: var(--fg-2);
  }
  /* Citation badges injected via {@html} need un-scoped styling.
     Defined here scoped to .plain / .why containers — the regex output
     uses class="citation-badge", so a global :global() rule binds it. */
  :global(.citation-badge) {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    padding: 1px 8px;
    border-radius: var(--r-full);
    border: 1px solid var(--line);
    color: var(--arcane-300);
    background: rgba(91, 120, 255, 0.08);
    font-family: var(--font-mono);
    font-size: 12px;
    margin: 0 2px;
    cursor: pointer;
  }
  :global(.citation-badge:hover) {
    border-color: var(--line-strong);
    color: var(--gem);
  }
</style>
