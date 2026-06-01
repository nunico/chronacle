<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    getCollections,
    createCollection,
    getMruCollectionId,
    setMruCollectionId,
    uploadSource,
    type Collection,
  } from './lib/commands';

  let messages = $state<Array<{ role: string; content: string }>>([]);
  let input = $state('');
  let isLoading = $state(false);
  let currentResponse = $state('');

  // Upload state
  let isUploading = $state(false);
  let uploadStatus = $state('');
  let uploadedSourceName = $state('');

  // Collection picker state
  let collections = $state<Collection[]>([]);
  let pendingUploadPath = $state<string | null>(null);
  let pendingUploadName = $state<string | null>(null);
  let showCollectionPicker = $state(false);
  let pickerCollectionId = $state('');
  let pickerNewName = $state('');
  let showNewCollectionInput = $state(false);
  let pickerError = $state('');

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

  async function openFilePicker() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;

    const path = typeof selected === 'string' ? selected : selected[0];
    const name = path.split('/').pop()?.split('\\').pop() ?? 'document.pdf';

    // Load collections and show picker
    try {
      collections = await getCollections();
    } catch (e) {
      console.error('Failed to load collections:', e);
      collections = [];
    }

    // Pre-select MRU collection if it still exists
    const mru = getMruCollectionId();
    pickerCollectionId =
      mru && collections.some((c) => c.id === mru) ? mru : (collections[0]?.id ?? '');
    pendingUploadPath = path;
    pendingUploadName = name;
    showCollectionPicker = true;
    pickerError = '';
    showNewCollectionInput = false;
    pickerNewName = '';
  }

  async function handlePickerCreateNew() {
    if (!pickerNewName.trim()) return;
    pickerError = '';
    try {
      const newCol = await createCollection(pickerNewName.trim());
      collections = [...collections, newCol];
      pickerCollectionId = newCol.id;
      pickerNewName = '';
      showNewCollectionInput = false;
    } catch (e) {
      pickerError = String(e);
    }
  }

  async function confirmUpload() {
    if (!pickerCollectionId || !pendingUploadPath || !pendingUploadName) return;
    pickerError = '';

    const path = pendingUploadPath;
    const name = pendingUploadName;
    const collectionId = pickerCollectionId;

    // Close picker immediately
    showCollectionPicker = false;
    pendingUploadPath = null;
    pendingUploadName = null;

    setMruCollectionId(collectionId);

    isUploading = true;
    uploadStatus = 'Uploading…';
    uploadedSourceName = name;

    let unlistenProgress: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;

    try {
      unlistenProgress = await listen<{ source_id: string; status: string; step: string }>(
        'ingestion-progress',
        (event) => {
          uploadStatus = event.payload.step ?? 'Processing…';
          if (event.payload.status === 'done') {
            isUploading = false;
            setTimeout(() => {
              uploadStatus = '';
            }, 2000);
          }
        },
      );

      unlistenError = await listen<{ source_id: string; error: string }>(
        'ingestion-error',
        (event) => {
          uploadStatus = `Error: ${event.payload.error}`;
          isUploading = false;
        },
      );

      await uploadSource(path, name, 'rules', collectionId);
    } catch (e) {
      uploadStatus = `Upload failed: ${String(e)}`;
      isUploading = false;
    } finally {
      if (unlistenProgress) unlistenProgress();
      if (unlistenError) unlistenError();
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
          <p class="hint">Upload rulebook PDFs using the button below, then ask questions here.</p>
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

    {#if isUploading || uploadStatus}
      <div class="upload-status" class:uploading={isUploading}>
        <span class="upload-name">{uploadedSourceName}</span>
        <span class="upload-msg">{uploadStatus}</span>
      </div>
    {/if}

    <!-- Collection picker overlay -->
    {#if showCollectionPicker}
      <div class="picker-overlay">
        <div class="picker-dialog" role="dialog" aria-modal="true" aria-labelledby="picker-title">
          <h3 id="picker-title">Add "{pendingUploadName}" to collection</h3>
          {#if pickerError}
            <div class="picker-error">{pickerError}</div>
          {/if}

          {#if collections.length > 0}
            <select bind:value={pickerCollectionId} class="picker-select">
              {#each collections as col}
                <option value={col.id}>{col.name}</option>
              {/each}
            </select>
          {:else}
            <p class="picker-hint">No collections yet.</p>
          {/if}

          {#if showNewCollectionInput}
            <div class="picker-new">
              <input
                bind:value={pickerNewName}
                placeholder="New collection name"
                onkeydown={(e) => e.key === 'Enter' && handlePickerCreateNew()}
              />
              <button class="picker-create-btn" onclick={handlePickerCreateNew}>Create</button>
              <button class="picker-cancel-btn" onclick={() => (showNewCollectionInput = false)}>Cancel</button>
            </div>
          {:else}
            <button class="picker-new-btn" onclick={() => (showNewCollectionInput = true)}>
              + Create new collection
            </button>
          {/if}

          <div class="picker-actions">
            <button
              class="picker-cancel-btn"
              data-testid="picker-cancel"
              onclick={() => { showCollectionPicker = false; pendingUploadPath = null; pendingUploadName = null; }}
            >Cancel</button>
            <button
              class="picker-confirm-btn"
              disabled={!pickerCollectionId}
              onclick={confirmUpload}
            >Upload</button>
          </div>
        </div>
      </div>
    {/if}
  </main>

  <footer>
    <button onclick={openFilePicker} disabled={isUploading}>
      {isUploading ? 'Uploading…' : 'Upload PDF'}
    </button>
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

  .upload-status {
    margin-top: 0.5rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.82rem;
    color: var(--text-muted);
    padding: 0.3rem 0.5rem;
    border-radius: 4px;
    background: var(--bg-assistant);
  }

  .upload-status.uploading {
    color: var(--accent);
  }

  .upload-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }

  .upload-msg {
    flex: 1;
  }

  footer {
    padding: 0.75rem 0;
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: center;
    gap: 0.5rem;
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

  footer button:hover:not(:disabled) {
    background: var(--bg-assistant);
  }

  footer button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* ── Collection picker ──────────────────────────────────────────── */

  .picker-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .picker-dialog {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1.25rem;
    width: 320px;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .picker-dialog h3 {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }

  .picker-error {
    color: #e74c3c;
    font-size: 0.8rem;
    padding: 0.3rem 0.5rem;
    background: rgba(231, 76, 60, 0.1);
    border-radius: 4px;
  }

  .picker-select {
    width: 100%;
    padding: 0.4rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .picker-hint { font-size: 0.85rem; color: var(--text-muted); margin: 0; }

  .picker-new {
    display: flex;
    gap: 0.35rem;
  }

  .picker-new input {
    flex: 1;
    padding: 0.35rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .picker-new-btn {
    background: none;
    border: 1px dashed var(--border);
    border-radius: 4px;
    padding: 0.3rem 0.6rem;
    font-size: 0.8rem;
    cursor: pointer;
    color: var(--text-muted);
  }

  .picker-new-btn:hover { border-color: var(--accent); color: var(--accent); }

  .picker-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }

  .picker-cancel-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0.35rem 0.7rem;
    font-size: 0.85rem;
    cursor: pointer;
    color: var(--text);
  }

  .picker-create-btn, .picker-confirm-btn {
    border: none;
    border-radius: 4px;
    padding: 0.35rem 0.7rem;
    font-size: 0.85rem;
    cursor: pointer;
    background: var(--accent);
    color: #fff;
    font-weight: 600;
  }

  .picker-confirm-btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
