<script lang="ts">
  import { onMount } from 'svelte';
  import { chatSend, getChatHistory, uploadSource } from './lib/commands';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import SettingsPage from './SettingsPage.svelte';

  let currentPage = $state<'chat' | 'settings'>('chat');
  let messages = $state<Array<{ role: string; content: string }>>([]);
  let input = $state('');
  let isLoading = $state(false);
  let currentResponse = $state('');
  let isUploading = $state(false);
  let uploadProgress = $state(0);
  let uploadStatus = $state('');
  let uploadedSourceName = $state('');

  onMount(async () => {
    try {
      const history = await getChatHistory(null);
      messages = history;
    } catch (e) {
      console.error('Failed to load chat history:', e);
    }
  });

  async function selectAndUploadPdf() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;

    const path = selected as string;
    const name = path.split('/').pop()?.split('\\').pop() || 'document.pdf';
    isUploading = true;
    uploadProgress = 0;
    uploadStatus = 'Uploading…';
    uploadedSourceName = name;

    let unlistenProgress: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;

    try {
      unlistenProgress = await listen<{ source_id: string; status: string; progress: number }>(
        'ingestion-progress',
        (event) => {
          uploadProgress = Math.round(event.payload.progress * 100);
          switch (event.payload.status) {
            case 'indexing':
              uploadStatus = 'Indexing PDF…';
              break;
            case 'done':
              uploadStatus = 'Ready!';
              uploadProgress = 100;
              break;
          }
        },
      );

      unlistenError = await listen<{ source_id: string; error: string }>(
        'ingestion-error',
        (event) => {
          uploadStatus = `Error: ${event.payload.error}`;
          console.error('Ingestion error:', event.payload.error);
        },
      );

      await uploadSource(path, name, 'rules');
    } catch (e) {
      uploadStatus = `Failed: ${e}`;
      console.error('Upload failed:', e);
    } finally {
      if (unlistenProgress) unlistenProgress();
      if (unlistenError) unlistenError();
      isUploading = false;
    }
  }

  /** Render message content with citation badges */
  function renderContent(text: string): string {
    // Replace [Source: "name", p.N] or [Source: "name"] with styled HTML
    return text.replace(
      /\[Source:\s*"([^"]+)"(?:,\s*p\.\s*(\d+))?\]/g,
      (_, name: string, page: string | undefined) => {
        return `<span class="citation-badge" title="Source: ${name}${page ? `, p.${page}` : ''}">${name}${page ? ` p.${page}` : ''}</span>`;
      },
    );
  }

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

      await chatSend(text, null);
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

<header>
  <h1>Chronacle</h1>
  <span class="tagline">TTRPG GM Assistant</span>
  <nav>
    <button class="nav-btn" class:active={currentPage === 'chat'} onclick={() => (currentPage = 'chat')}>
      Chat
    </button>
    <button class="nav-btn" class:active={currentPage === 'settings'} onclick={() => (currentPage = 'settings')}>
      Settings
    </button>
    <button class="upload-btn" onclick={selectAndUploadPdf} disabled={isUploading}>
      {isUploading ? 'Uploading…' : 'Upload PDF'}
    </button>
  </nav>
  {#if isUploading || uploadStatus}
    <div class="upload-status">
      <span class="upload-filename">{uploadedSourceName}</span>
      <span class="upload-progress-text">{uploadStatus}</span>
      {#if isUploading}
        <div class="progress-bar">
          <div class="progress-fill" style="width: {uploadProgress}%"></div>
        </div>
      {/if}
    </div>
  {/if}
</header>

<main>
  {#if currentPage === 'chat'}
    <div class="chat-container">
      {#if messages.length === 0 && !isLoading}
        <div class="welcome">
          <p>Welcome to Chronacle, your TTRPG Game Master's assistant.</p>
          <p class="hint">Upload a PDF rulebook using the Upload button above, then ask questions here.</p>
        </div>
      {/if}

      <div class="messages">
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
  {:else}
    <SettingsPage />
  {/if}
</main>

<style>
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

  header nav {
    display: flex;
    justify-content: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }

  .nav-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.35rem 1rem;
    color: var(--text-muted);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.85rem;
    transition: background 0.15s, color 0.15s;
  }

  .nav-btn:hover {
    background: var(--bg-assistant);
    color: var(--text);
  }

  .nav-btn.active {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
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
    content: '\258A';
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

  .upload-btn {
    background: var(--accent);
    color: #fff;
    border: none;
    border-radius: 4px;
    padding: 0.35rem 1rem;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.85rem;
    transition: background 0.15s;
  }

  .upload-btn:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .upload-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .upload-status {
    margin-top: 0.5rem;
    font-size: 0.8rem;
    color: var(--text-muted);
    text-align: center;
  }

  .upload-filename {
    font-weight: 600;
    margin-right: 0.5rem;
  }

  .upload-progress-text {
    color: var(--accent);
  }

  .progress-bar {
    margin: 0.3rem auto 0;
    width: 200px;
    height: 4px;
    background: var(--border);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 0.3s ease;
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
  }

  .citation-badge:hover {
    filter: brightness(1.15);
  }
</style>