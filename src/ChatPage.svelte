<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    chatSend,
    getChatHistory,
    getChunkForCitation,
    type Campaign,
    type CitationChunk,
  } from './lib/commands';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

  let {
    campaigns,
    activeCampaignId,
    onCampaignChange,
  }: {
    campaigns: Campaign[];
    activeCampaignId: string | null;
    onCampaignChange?: (id: string | null) => void;
  } = $props();

  let messages = $state<Array<{ role: string; content: string }>>([]);
  let input = $state('');
  let isLoading = $state(false);
  let currentResponse = $state('');

  // Citation popover state: when the user clicks a citation badge we look
  // up the chunk that backed it and float a popover next to the badge.
  let citationPopover = $state<
    | {
        source: string;
        page: number | null;
        chunk: CitationChunk | null;
        loading: boolean;
        x: number;
        y: number;
      }
    | null
  >(null);

  let unlistenListener: UnlistenFn | null = null;

  onMount(async () => {
    try {
      const history = await getChatHistory(activeCampaignId);
      messages = history;
    } catch (e) {
      console.error('Failed to load chat history:', e);
    }

    // Register a persistent listener for streaming chat tokens.
    // Chat_send returns immediately (it spawns a background task), so the
    // listener must outlive the invoke call.
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

  /** HTML-attribute-escape a string. */
  function escapeAttr(s: string): string {
    return s
      .replace(/&/g, '&amp;')
      .replace(/"/g, '&quot;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  /** Render message content with clickable citation badges.
   *
   * Matches both `p.N` and `p.N-M` (range). `page` captures the start; the
   * optional `-M` tail is consumed but discarded — the chunk for page_start
   * also covers page_end. */
  function renderContent(text: string): string {
    return text.replace(
      /\[Source:\s*"([^"]+)"(?:,\s*p\.\s*(\d+)(?:-\d+)?)?\]/g,
      (_, name: string, page: string | undefined) => {
        const dataPage = page ? ` data-page="${escapeAttr(page)}"` : '';
        const label = `${escapeAttr(name)}${page ? ` p.${escapeAttr(page)}` : ''}`;
        return `<button type="button" class="citation-badge" data-source="${escapeAttr(name)}"${dataPage} title="Show source passage">${label}</button>`;
      },
    );
  }

  async function handleMessagesClick(event: MouseEvent) {
    const target = (event.target as HTMLElement | null)?.closest('.citation-badge');
    if (!(target instanceof HTMLElement)) return;
    event.stopPropagation();

    const source = target.dataset.source ?? '';
    const pageStr = target.dataset.page;
    const page = pageStr ? parseInt(pageStr, 10) : null;
    const rect = target.getBoundingClientRect();

    citationPopover = {
      source,
      page,
      chunk: null,
      loading: true,
      x: rect.left,
      y: rect.bottom + 6,
    };

    try {
      const chunk = await getChunkForCitation(source, page);
      // Only commit if the user hasn't clicked another / dismissed in the meantime
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
    const target = event.target as HTMLElement | null;
    if (target?.closest('.citation-popover') || target?.closest('.citation-badge')) return;
    citationPopover = null;
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') citationPopover = null;
  }

  async function sendMessage() {
    const text = input.trim();
    if (!text || isLoading) return;

    messages = [...messages, { role: 'user', content: text }];
    input = '';
    isLoading = true;
    currentResponse = '';

    try {
      await chatSend(text, activeCampaignId);
    } catch (e) {
      console.error('Chat send failed:', e);
      isLoading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  }

  function getActiveCampaignName(): string {
    if (!activeCampaignId) return 'Global';
    return campaigns.find((c) => c.id === activeCampaignId)?.name ?? 'Unknown';
  }

  async function handleCampaignChange(cid: string | null) {
    if (onCampaignChange) onCampaignChange(cid);
    const history = await getChatHistory(cid);
    messages = history;
  }
</script>

<div class="chat-container">
  {#if messages.length === 0 && !isLoading}
    <div class="welcome">
      <p>Welcome to Chronacle, your TTRPG Game Master's assistant.</p>
      <p class="hint">Upload a PDF rulebook using the Upload button above, then ask questions here.</p>
    </div>
  {/if}

  <div class="messages" onclick={handleMessagesClick} role="presentation">
    {#each messages as msg (msg.role + msg.content)}
      <div class="message {msg.role}">
        <div class="role-label">{msg.role === 'user' ? 'You' : 'Chronacle'}</div>
        <div class="content">{@html renderContent(msg.content)}</div>
      </div>
    {/each}

    {#if isLoading && currentResponse}
      <div class="message assistant">
        <div class="role-label">Chronacle</div>
        <div class="content streaming">{@html renderContent(currentResponse)}</div>
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
      {#if citationPopover.chunk}
        <span class="muted">
          p.{citationPopover.chunk.page_start}{citationPopover.chunk.page_end !==
          citationPopover.chunk.page_start
            ? `-${citationPopover.chunk.page_end}`
            : ''}
        </span>
      {:else if citationPopover.page !== null}
        <span class="muted">p.{citationPopover.page}</span>
      {/if}
      <button
        type="button"
        class="popover-close"
        aria-label="Close"
        onclick={() => (citationPopover = null)}>×</button>
    </div>
    {#if citationPopover.loading}
      <div class="popover-body muted">Loading…</div>
    {:else if citationPopover.chunk}
      {#if citationPopover.chunk.section_heading}
        <div class="popover-heading">{citationPopover.chunk.section_heading}</div>
      {/if}
      <div class="popover-body">{citationPopover.chunk.text}</div>
    {:else}
      <div class="popover-body muted">No matching passage found.</div>
    {/if}
  </div>
{/if}

<svelte:window onclick={handleWindowClick} onkeydown={handleWindowKeydown} />

<div class="input-area">
  <div class="input-header">
    <div class="campaign-context">
      Context: <strong>{getActiveCampaignName()}</strong>
      {#if campaigns.length > 0}
        <select
          value={activeCampaignId}
          onchange={(e) => handleCampaignChange((e.target as HTMLSelectElement).value || null)}
        >
          <option value={null}>Global</option>
          {#each campaigns as c}
            <option value={c.id}>{c.name}</option>
          {/each}
        </select>
      {/if}
    </div>
  </div>
  <textarea
        bind:value={input}
        onkeydown={handleKeydown}
        placeholder="Ask a question about your rulebooks…"
        rows="3"
        disabled={isLoading}
      ></textarea>
  <button onclick={sendMessage} disabled={isLoading || !input.trim()}>
    {isLoading ? 'Thinking…' : 'Send'}
  </button>
</div>

<style>
  .chat-container {
    flex: 1;
    overflow-y: auto;
    margin-bottom: 1rem;
  }

  .welcome {
    text-align: center;
    padding: 3rem 1rem;
    color: var(--text-muted);
  }

  .welcome .hint {
    font-size: 0.9rem;
    margin-top: 0.5rem;
  }

  .messages {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .message {
    padding: 0.75rem 1rem;
    border-radius: 8px;
    line-height: 1.6;
  }

  .message.user {
    background: var(--bg-user);
    align-self: flex-end;
    max-width: 80%;
  }

  .message.assistant {
    background: var(--bg-assistant);
    align-self: flex-start;
    max-width: 85%;
  }

  .role-label {
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.3rem;
    color: var(--text-muted);
  }

  .content {
    white-space: pre-wrap;
    word-wrap: break-word;
  }

  .streaming::after {
    content: '\258A';
    animation: blink 0.8s step-end infinite;
  }

  @keyframes blink {
    50% { opacity: 0; }
  }

  .input-area {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .input-header {
    display: flex;
    justify-content: flex-start;
  }

  .campaign-context {
    font-size: 0.8rem;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .campaign-context select {
    font-size: 0.8rem;
    padding: 0.15rem 0.3rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
  }

  .input-area textarea {
    flex: none;
    padding: 0.6rem 0.8rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.95rem;
    resize: none;
  }

  .input-area textarea:disabled {
    opacity: 0.6;
  }

  .input-area > button {
    align-self: flex-end;
    padding: 0.6rem 1.5rem;
    border: none;
    border-radius: 6px;
    background: var(--accent);
    color: #fff;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
    white-space: nowrap;
  }

  .input-area > button:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .input-area > button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .citation-badge {
    display: inline-block;
    background: var(--accent);
    color: #fff;
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    font-size: 0.8rem;
    cursor: pointer;
    margin: 0 0.15rem;
    line-height: 1.4;
    user-select: none;
    border: none;
    font-family: inherit;
  }

  .citation-badge:hover {
    filter: brightness(1.15);
  }

  .citation-popover {
    position: fixed;
    z-index: 100;
    max-width: min(440px, 90vw);
    background: var(--bg, #1f2030);
    color: var(--text, #e7e7ef);
    border: 1px solid var(--border, #3a3b50);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    overflow: hidden;
  }

  .popover-header {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border, #3a3b50);
    background: var(--surface, #2a2b40);
  }

  .popover-header .muted {
    color: var(--text-muted, #9999b3);
    font-size: 0.8rem;
  }

  .popover-close {
    margin-left: auto;
    background: transparent;
    color: inherit;
    border: none;
    font-size: 1.1rem;
    line-height: 1;
    cursor: pointer;
    padding: 0 0.25rem;
  }

  .popover-close:hover {
    color: var(--accent);
  }

  .popover-heading {
    padding: 0.4rem 0.75rem 0;
    font-size: 0.8rem;
    color: var(--text-muted, #9999b3);
    font-style: italic;
  }

  .popover-body {
    padding: 0.6rem 0.75rem 0.75rem;
    font-size: 0.9rem;
    line-height: 1.45;
    max-height: 320px;
    overflow-y: auto;
    white-space: pre-wrap;
  }

  .popover-body.muted {
    color: var(--text-muted, #9999b3);
    font-style: italic;
  }
</style>