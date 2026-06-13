<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    chatSend,
    chatCancel,
    getChatHistory,
    getChunkForCitation,
    getSources,
    extractEntityByName,
    extractAllFromCampaign,
    cancelExtraction,
    type CitationChunk,
    type ExtractionProgress,
  } from '../lib/commands';
  import Icon from '../components/Icon.svelte';
  import EyeMark from '../components/EyeMark.svelte';
  import RulingCard from '../components/RulingCard.svelte';
  import ExtractionCard from '../components/ExtractionCard.svelte';
  import { parseCommand } from '../lib/chat-commands';
  import { renderContent, parseRuling, splitHeading } from './ruling-parse';
  import { parseExtractionMessage } from './extraction-message';
  import { isNearBottom } from '../lib/scroll';
  import { clampPopoverPosition } from './popover-position';

  let {
    activeCampaignId,
    onOpenUpload,
  }: {
    activeCampaignId: string | null;
    onOpenUpload: () => void;
  } = $props();

  let messages = $state<Array<{ role: string; content: string; gmOnly?: boolean }>>([]);
  let input = $state('');
  let isLoading = $state(false);
  let currentResponse = $state('');
  let unlistenListener: UnlistenFn | null = null;
  let scrollEl = $state<HTMLDivElement | undefined>(undefined);
  let inputEl = $state<HTMLTextAreaElement | undefined>(undefined);
  let popoverEl = $state<HTMLDivElement | undefined>(undefined);
  let atBottom = $state(true);
  let hasSources = $state(true);

  type ExtractionStatus = 'running' | 'done' | 'empty' | 'cancelled' | 'error';
  let extraction = $state<{
    status: ExtractionStatus;
    title: string;
    detail: string;
    entitiesFound: number;
    relationsFound: number;
  } | null>(null);
  let unlistenExtract: UnlistenFn | null = null;

  let citationPopover = $state<{
    source: string;
    page: number | null;
    quote: string | null;
    chunk: CitationChunk | null;
    loading: boolean;
    x: number;
    y: number;
    anchor: { top: number; bottom: number };
  } | null>(null);

  const suggestions = [
    { icon: 'swords', text: 'Can I cast a spell while grappled?' },
    { icon: 'shield', text: 'How does cover affect spell attacks?' },
    { icon: 'scale', text: 'What are the rules for opportunity attacks?' },
    { icon: 'book-open', text: "What's in the rulebook I just uploaded?" },
  ];

  async function loadHistory(campaignId: string | null) {
    try {
      const history = await getChatHistory(campaignId);
      messages = history.map((m) => ({ role: m.role, content: m.content, gmOnly: m.is_gm_only }));
    } catch (e) {
      console.error('Failed to load chat history:', e);
    }
  }

  // Refetch when the active campaign changes.
  $effect(() => {
    loadHistory(activeCampaignId);
  });

  // Auto-follow the thread only while the user is already near the bottom;
  // never yank the scroll position while they are reading older messages.
  $effect(() => {
    void messages;
    void currentResponse;
    void isLoading;
    if (scrollEl && atBottom) scrollEl.scrollTop = scrollEl.scrollHeight;
  });

  function handleScroll() {
    if (scrollEl) atBottom = isNearBottom(scrollEl);
  }

  function jumpToLatest() {
    if (!scrollEl) return;
    scrollEl.scrollTop = scrollEl.scrollHeight;
    atBottom = true;
  }

  // Re-clamp the citation popover once it has rendered and we know its size.
  $effect(() => {
    if (!citationPopover || !popoverEl) return;
    const rect = popoverEl.getBoundingClientRect();
    const clamped = clampPopoverPosition({
      x: citationPopover.x,
      y: citationPopover.y,
      anchor: citationPopover.anchor,
      popover: { width: rect.width, height: rect.height },
      viewport: { width: window.innerWidth, height: window.innerHeight },
    });
    if (clamped.x !== citationPopover.x || clamped.y !== citationPopover.y) {
      citationPopover = { ...citationPopover, ...clamped };
    }
  });

  onMount(async () => {
    try {
      hasSources = (await getSources(null)).length > 0;
    } catch {
      // If the check fails, keep the optimistic default and show suggestions.
      hasSources = true;
    }
    unlistenListener = await listen<{ token: string; done: boolean; gm_only?: boolean }>(
      'chat-token',
      (event) => {
        // The backend emits the terminal error as `{ token: "[Error: ...]", done: true }`
        // in a single event. Append the token BEFORE flushing so the error reaches
        // the thread instead of being silently swallowed when no prior tokens streamed.
        if (event.payload.token) {
          currentResponse += event.payload.token;
        }
        if (event.payload.done) {
          if (currentResponse) {
            messages = [
              ...messages,
              { role: 'assistant', content: currentResponse, gmOnly: event.payload.gm_only },
            ];
          }
          currentResponse = '';
          isLoading = false;
        }
      },
    );
    unlistenExtract = await listen<ExtractionProgress>('extract-progress', (event) => {
      const p = event.payload;
      if (!extraction) return;
      extraction = {
        ...extraction,
        detail: p.detail,
        entitiesFound: p.entities_found,
        relationsFound: p.relations_found,
        status: p.phase === 'done' ? 'done' : p.phase === 'empty' ? 'empty' : 'running',
      };
    });
  });

  onDestroy(() => {
    if (unlistenListener) unlistenListener();
    if (unlistenExtract) unlistenExtract();
  });

  async function sendMessage(text?: string) {
    const t = (text ?? input).trim();
    if (!t || isLoading || extraction?.status === 'running') return;

    const cmd = parseCommand(t);
    if (cmd.kind !== 'chat') {
      input = '';
      if (inputEl) {
        inputEl.style.height = 'auto';
        inputEl.focus();
      }
      handleCommand(cmd);
      return;
    }

    messages = [...messages, { role: 'user', content: t }];
    input = '';
    if (inputEl) {
      inputEl.style.height = 'auto';
      inputEl.focus();
    }
    atBottom = true;
    isLoading = true;
    currentResponse = '';
    try {
      await chatSend(t, activeCampaignId);
    } catch (e) {
      messages = [...messages, { role: 'error', content: String(e) }];
      isLoading = false;
    }
  }

  function handleCommand(cmd: ReturnType<typeof parseCommand>) {
    switch (cmd.kind) {
      case 'extract-usage':
        messages = [
          ...messages,
          {
            role: 'system',
            content:
              'Usage: /extract <entity name>. To extract everything from all books, use /extract-all (this can take a while).',
          },
        ];
        return;
      case 'help':
        messages = [
          ...messages,
          {
            role: 'system',
            content:
              'Commands: /extract <name> — build one entity; /extract-all — extract everything (slow); /help — this list.',
          },
        ];
        return;
      case 'extract':
        runExtraction(
          () => extractEntityByName(activeCampaignId ?? '', cmd.name),
          `Extracting "${cmd.name}"`,
        );
        return;
      case 'extract-all':
        runExtraction(
          () => extractAllFromCampaign(activeCampaignId ?? ''),
          'Extracting all entities',
        );
        return;
    }
  }

  async function runExtraction(
    start: () => Promise<{ entities_created: number; relations_created: number }>,
    title: string,
  ) {
    if (!activeCampaignId) {
      messages = [...messages, { role: 'error', content: 'Select a campaign first.' }];
      return;
    }
    extraction = {
      status: 'running',
      title,
      detail: 'Starting…',
      entitiesFound: 0,
      relationsFound: 0,
    };
    try {
      const summary = await start();
      const wasEmpty = extraction?.status === 'empty';
      extraction = {
        status: wasEmpty ? 'empty' : 'done',
        title: wasEmpty ? 'Nothing found' : 'Extraction complete',
        detail: wasEmpty
          ? (extraction?.detail ?? 'No passages found')
          : `Created ${summary.entities_created} entities, ${summary.relations_created} relations`,
        entitiesFound: summary.entities_created,
        relationsFound: summary.relations_created,
      };
    } catch (e) {
      const cancelled = String(e).includes('cancelled');
      extraction = {
        status: cancelled ? 'cancelled' : 'error',
        title: cancelled ? 'Cancelled' : 'Extraction failed',
        detail: cancelled
          ? `Cancelled — kept ${extraction?.entitiesFound ?? 0} entities / ${extraction?.relationsFound ?? 0} relations created so far`
          : String(e),
        entitiesFound: extraction?.entitiesFound ?? 0,
        relationsFound: extraction?.relationsFound ?? 0,
      };
    }
  }

  async function cancelActiveExtraction() {
    try {
      await cancelExtraction();
    } catch (e) {
      console.error('Failed to cancel extraction:', e);
    }
  }

  async function stopGeneration() {
    try {
      await chatCancel();
    } catch (e) {
      console.error('Failed to cancel chat:', e);
    }
  }

  /** Backend pipeline failures stream in as `[Error: ...]` tokens; local
   * `chatSend` rejections are stored with role `error`. Both render as an
   * error bubble with a retry affordance. */
  function errorText(msg: { role: string; content: string }): string | null {
    if (msg.role === 'error') return msg.content;
    const m = msg.content.trim().match(/^\[Error:\s*([\s\S]*?)\]$/);
    return m ? m[1] : null;
  }

  /** Re-send the user message that preceded the error at `index`. */
  function retryFrom(index: number) {
    for (let i = index - 1; i >= 0; i--) {
      if (messages[i].role === 'user') {
        sendMessage(messages[i].content);
        return;
      }
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  function autoGrow() {
    if (!inputEl) return;
    inputEl.style.height = 'auto';
    inputEl.style.height = `${Math.min(inputEl.scrollHeight, 160)}px`;
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

    const anchor = { top: rect.top, bottom: rect.bottom };
    if (inlineQuote) {
      citationPopover = {
        source,
        page,
        quote: inlineQuote,
        chunk: null,
        loading: false,
        x: rect.left,
        y: rect.bottom + 6,
        anchor,
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
      anchor,
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

<div class="scroll" bind:this={scrollEl} onscroll={handleScroll}>
  <div class="thread" onclick={handleThreadClick} role="presentation">
    {#each messages as msg, i (msg)}
      {#if msg.role === 'user'}
        <div class="msg user">
          <div class="bubble">{msg.content}</div>
          <div class="who-av">GM</div>
        </div>
      {:else if msg.role === 'extraction'}
        {@const card = parseExtractionMessage(msg.content)}
        {#if card}
          <ExtractionCard
            status={card.status}
            title={card.title}
            detail={card.detail}
            entitiesFound={card.entitiesFound}
            relationsFound={card.relationsFound}
          />
        {/if}
      {:else if errorText(msg) !== null}
        <div class="msg">
          <div class="who-av eye-badge"><EyeMark size={28} /></div>
          <div class="error-bubble" role="alert">
            <div class="error-title">The oracle could not answer.</div>
            <div class="error-detail">{errorText(msg)}</div>
            <button type="button" class="retry-btn" onclick={() => retryFrom(i)}>
              <Icon name="rotate-ccw" size={13} />
              Retry
            </button>
          </div>
        </div>
      {:else if hasCitation(msg.content)}
        {#if msg.gmOnly}
          <div class="gm-badge" title="This answer drew on GM-secret material">
            <Icon name="eye-off" size={12} /> GM only
          </div>
        {/if}
        <RulingCard data={parseRuling(msg.content)} />
      {:else if msg.role === 'system'}
        <div class="msg">
          <div class="who-av eye-badge"><EyeMark size={28} /></div>
          <p class="system-note">{msg.content}</p>
        </div>
      {:else}
        <div class="msg">
          <div class="who-av eye-badge"><EyeMark size={28} /></div>
          <div class="plain">
            {#if msg.gmOnly}
              <div class="gm-badge" title="This answer drew on GM-secret material">
                <Icon name="eye-off" size={12} /> GM only
              </div>
            {/if}
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

    {#if extraction}
      <ExtractionCard
        status={extraction.status}
        title={extraction.title}
        detail={extraction.detail}
        entitiesFound={extraction.entitiesFound}
        relationsFound={extraction.relationsFound}
        onCancel={cancelActiveExtraction}
      />
    {/if}

    {#if messages.length === 0 && !isLoading}
      {#if hasSources}
        <div class="suggest">
          {#each suggestions as s (s.text)}
            <button class="sug" onclick={() => sendMessage(s.text)}>
              <Icon name={s.icon} size={15} />
              {s.text}
            </button>
          {/each}
        </div>
      {:else}
        <div class="empty-library">
          <p>The oracle has no tomes to consult yet.</p>
          <button class="sug nudge" onclick={onOpenUpload}>
            <Icon name="paperclip" size={15} />
            Upload a rulebook to get started
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

{#if !atBottom}
  <div class="jump-wrap">
    <button type="button" class="jump-btn" onclick={jumpToLatest}>
      <Icon name="arrow-down" size={14} />
      Jump to latest
    </button>
  </div>
{/if}

{#if citationPopover}
  <div
    class="citation-popover"
    bind:this={popoverEl}
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
        onclick={() => (citationPopover = null)}>×</button
      >
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
    <textarea
      bind:this={inputEl}
      bind:value={input}
      onkeydown={handleKeydown}
      oninput={autoGrow}
      rows="1"
      placeholder="Ask a rule, a name, a place…"
    ></textarea>
    <button
      class="tool"
      onclick={onOpenUpload}
      title="Attach a rulebook"
      aria-label="Attach a rulebook"
    >
      <Icon name="paperclip" size={18} />
    </button>
    <button class="tool" title="Roll — coming soon" aria-label="Roll dice" disabled>
      <Icon name="dices" size={18} />
    </button>
    {#if isLoading}
      <button
        class="send-btn"
        onclick={stopGeneration}
        aria-label="Stop generating"
        title="Stop generating"
      >
        <Icon name="square" size={16} />
      </button>
    {:else}
      <button
        class="send-btn"
        disabled={!input.trim()}
        onclick={() => sendMessage()}
        aria-label="Send"
      >
        <Icon name="arrow-up" size={18} />
      </button>
    {/if}
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
    0%,
    60%,
    100% {
      opacity: 0.35;
      transform: translateY(0);
    }
    30% {
      opacity: 1;
      transform: translateY(-2px);
    }
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
  .empty-library {
    margin-top: 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--fg-3);
    font-family: var(--font-serif);
    font-size: 15px;
    font-style: italic;
  }
  .empty-library p {
    margin: 0;
  }
  .nudge {
    color: var(--arcane-300);
    border-color: var(--line-glow);
    font-style: normal;
  }
  .system-note {
    color: var(--fg-3);
    font-size: 0.85rem;
    font-style: italic;
    margin: 0;
  }
  .gm-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    width: fit-content;
    margin-bottom: 6px;
    padding: 2px 8px;
    border: 1px solid var(--violet-300, #a78bfa);
    border-radius: 999px;
    color: var(--violet-300, #a78bfa);
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .error-bubble {
    flex: 1;
    background: var(--danger-bg, rgba(242, 103, 75, 0.08));
    border: 1px solid var(--line);
    border-left: 3px solid var(--danger);
    border-radius: var(--r-lg);
    padding: 12px 14px;
    font-family: var(--font-sans);
    font-size: 13.5px;
    color: var(--fg-2);
  }
  .error-title {
    font-weight: 600;
    color: var(--fg-1);
    margin-bottom: 4px;
  }
  .error-detail {
    color: var(--fg-2);
    word-break: break-word;
    margin-bottom: 10px;
  }
  .retry-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    background: none;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-full);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 12.5px;
    cursor: pointer;
  }
  .retry-btn:hover {
    border-color: var(--line-glow);
    color: var(--arcane-300);
  }
  .jump-wrap {
    position: relative;
    height: 0;
    display: flex;
    justify-content: center;
  }
  .jump-btn {
    position: absolute;
    bottom: 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    background: var(--bg-panel-2);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-full);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 12.5px;
    box-shadow: var(--shadow-3);
    cursor: pointer;
    z-index: 10;
  }
  .jump-btn:hover {
    border-color: var(--line-glow);
    color: var(--arcane-300);
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
  .composer textarea {
    flex: 1;
    border: 0;
    background: transparent;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 15px;
    padding: 8px 0;
    resize: none;
    max-height: 160px;
    line-height: 1.4;
  }
  .composer textarea:focus {
    outline: none;
  }
  .composer textarea::placeholder {
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
    line-height: 1;
    margin: 0 2px;
    cursor: pointer;
  }
  :global(.citation-badge:hover) {
    border-color: var(--line-strong);
    color: var(--gem);
  }
  /* Inline `code` injected via {@html} into ruling/plain message bodies. */
  :global(.plain code),
  :global(.ruling code) {
    font-family: var(--font-mono);
    font-size: 0.86em;
    padding: 1px 5px;
    border-radius: var(--r-sm, 4px);
    background: var(--bg-inset);
    color: var(--fg-1);
  }
  :global(.entity-badge) {
    display: inline-flex;
    align-items: baseline;
    padding: 1px 8px;
    border-radius: var(--r-full);
    border: 1px solid var(--line);
    color: var(--violet-300);
    background: rgba(184, 166, 255, 0.08);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1;
    margin: 0 2px;
  }
</style>
