<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';

  let messages = $state<Array<{ role: string; content: string }>>([]);
  let input = $state('');
  let isLoading = $state(false);
  let currentResponse = $state('');

  onMount(async () => {
    // Load chat history from backend on page load
    try {
      const history = await invoke<Array<{ role: string; content: string }>>('get_chat_history', {
        campaignId: null as string | null,
      });
      messages = history;
    } catch (e) {
      console.error('Failed to load chat history:', e);
    }
  });

  async function sendMessage() {
    const text = input.trim();
    if (!text || isLoading) return;

    messages = [...messages, { role: 'user', content: text }];
    input = '';
    isLoading = true;
    currentResponse = '';

    let unlisten: UnlistenFn | null = null;

    try {
      unlisten = await listen<{ token: string; done: boolean }>('chat-token', (event) => {
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

      await invoke('chat_send', {
        request: {
          message: text,
          campaignId: null,
        },
      });
    } catch (e) {
      console.error('Chat send failed:', e);
      isLoading = false;
    } finally {
      if (unlisten) unlisten();
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      sendMessage();
    }
  }
</script>

<div class="app-container">
  <header>
    <h1>Chronacle</h1>
    <span class="tagline">TTRPG GM Assistant</span>
  </header>

  <main>
    <div class="chat-container">
      {#if messages.length === 0 && !isLoading}
        <div class="welcome">
          <p>Welcome to Chronacle, your TTRPG Game Master's assistant.</p>
          <p class="hint">Upload rulebook PDFs on the Settings page, then ask questions here.</p>
        </div>
      {/if}

      <div class="messages">
        {#each messages as msg (msg.role + msg.content)}
          <div class="message {msg.role}">
            <div class="role-label">{msg.role === 'user' ? 'You' : 'Chronacle'}</div>
            <div class="content">{msg.content}</div>
          </div>
        {/each}

        {#if isLoading && currentResponse}
          <div class="message assistant">
            <div class="role-label">Chronacle</div>
            <div class="content streaming">{currentResponse}</div>
          </div>
        {/if}
      </div>
    </div>

    <div class="input-area">
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
  </main>

  <footer>
    <button onclick={() => alert('Settings page — coming in Phase 1')}>
      Settings
    </button>
  </footer>
</div>

<style>
  .app-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    max-width: 900px;
    margin: 0 auto;
    padding: 0 1rem;
  }

  header {
    text-align: center;
    padding: 1rem 0;
    border-bottom: 1px solid var(--border);
  }

  header h1 {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0;
    letter-spacing: 0.05em;
  }

  .tagline {
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 1rem 0;
  }

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
    content: '▊';
    animation: blink 0.8s step-end infinite;
  }

  @keyframes blink {
    50% { opacity: 0; }
  }

  .input-area {
    display: flex;
    gap: 0.5rem;
    align-items: flex-end;
  }

  .input-area textarea {
    flex: 1;
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

  .input-area button {
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

  .input-area button:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .input-area button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  footer {
    padding: 0.75rem 0;
    border-top: 1px solid var(--border);
    text-align: center;
  }

  footer button {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.4rem 1rem;
    color: var(--text);
    cursor: pointer;
    font-size: 0.85rem;
  }

  footer button:hover {
    background: var(--bg-assistant);
  }
</style>
