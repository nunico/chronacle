<script lang="ts">
  import { onMount } from 'svelte';
  import { getSettings, updateSetting, getLlmProviderStatus, reconfigureLlmProvider } from './lib/commands';
  import {
    getCustomProviders,
    createCustomProvider,
    deleteCustomProvider,
    getProviderModels,
    addProviderModel,
    removeProviderModel,
    reindexAllSources,
    type CustomProvider,
    type CustomProviderModel,
    type ReindexProgress,
  } from './lib/commands';
  import { listen } from '@tauri-apps/api/event';
  import { SvelteMap } from 'svelte/reactivity';

  let providerType = $state('openai');
  let apiKey = $state('');
  let model = $state('');
  let baseUrl = $state('');

  let isSaving = $state(false);
  let isConnecting = $state(false);
  let statusMessage = $state('');
  let statusIsError = $state(false);

  let currentProviderType = $state('—');
  let currentModel = $state('—');
  let apiKeyConfigured = $state(false);

  // Custom providers state
  let customProviders = $state<CustomProvider[]>([]);
  let providerModelsMap = $state<Map<string, CustomProviderModel[]>>(new Map());
  let showAddProvider = $state(false);
  let newProviderName = $state('');
  let newProviderType = $state('openai');
  let newProviderBaseUrl = $state('');
  let newProviderApiKey = $state('');
  let editingProviderModels = $state<string | null>(null);
  let newModelId = $state('');
  let newModelDisplayName = $state('');

  // Re-index state
  let reindexing = $state(false);
  let reindexProgress = $state<ReindexProgress | null>(null);
  let reindexError = $state<string | null>(null);
  let reindexedCount = $state<number | null>(null);

  async function onReindexAll() {
    reindexing = true;
    reindexError = null;
    reindexedCount = null;
    reindexProgress = null;
    const unlisten = await listen<ReindexProgress>('reindex-progress', (e) => {
      reindexProgress = e.payload;
    });
    try {
      const count = await reindexAllSources();
      reindexedCount = count;
    } catch (e) {
      reindexError = String(e);
    } finally {
      reindexing = false;
      reindexProgress = null;
      unlisten();
    }
  }

  onMount(async () => {
    await loadSettings();
    await loadStatus();
    await loadCustomProviders();
  });

  // ── Existing functions (unchanged) ──────────────────────────────────

  async function loadSettings() {
    try {
      const settings = await getSettings();
      providerType = settings['llm_provider'] ?? 'openai';
      apiKey = settings['llm_api_key'] ?? '';
      model = settings['llm_model'] ?? '';
      baseUrl = settings['llm_base_url'] ?? '';
    } catch (e) {
      showError(`Failed to load settings: ${e}`);
    }
  }

  async function loadStatus() {
    try {
      const status = await getLlmProviderStatus();
      currentProviderType = status.provider_type;
      currentModel = status.model || '(default)';
      apiKeyConfigured = status.api_key_configured;
    } catch {
      // Status is unavailable on first load; that's fine
    }
  }

  function showError(msg: string) {
    statusMessage = msg;
    statusIsError = true;
    setTimeout(() => { statusMessage = ''; }, 5000);
  }

  function showSuccess(msg: string) {
    statusMessage = msg;
    statusIsError = false;
    setTimeout(() => { statusMessage = ''; }, 3000);
  }

  async function saveSettings() {
    isSaving = true;
    statusMessage = '';
    try {
      await Promise.all([
        updateSetting('llm_provider', providerType),
        updateSetting('llm_api_key', apiKey),
        updateSetting('llm_model', model),
        updateSetting('llm_base_url', baseUrl),
      ]);
      showSuccess('Settings saved.');
    } catch (e) {
      showError(`Failed to save: ${e}`);
    } finally {
      isSaving = false;
    }
  }

  async function connect() {
    isConnecting = true;
    statusMessage = '';
    try {
      await saveSettings();
      const activeType = await reconfigureLlmProvider();
      await loadStatus();
      showSuccess(`Connected: ${activeType}`);
    } catch (e) {
      showError(`Connection failed: ${e}`);
    } finally {
      isConnecting = false;
    }
  }

  // ── New: derived state ─────────────────────────────────────────────

  let showBaseUrl = $derived(
    providerType === 'ollama' || (providerType === 'openai' && baseUrl !== '') || providerType.startsWith('custom:')
  );

  let showApiKey = $derived(
    providerType === 'openai' || providerType === 'anthropic' || providerType.startsWith('custom:')
  );

  let modelPlaceholder = $derived.by(() => {
    switch (providerType) {
      case 'openai': return 'gpt-4o-mini';
      case 'anthropic': return 'claude-3-5-haiku-20241022';
      case 'ollama': return 'llama3.2';
      default: return '';
    }
  });

  let baseUrlPlaceholder = $derived.by(() => {
    switch (providerType) {
      case 'ollama': return 'http://localhost:11434';
      case 'openai': return 'https://api.openai.com/v1';
      default: return '';
    }
  });

  // Provider options: built-in + custom providers with a separator
  let providerOptions = $derived.by(() => {
    const builtin = [
      { value: 'openai', label: 'OpenAI' },
      { value: 'anthropic', label: 'Anthropic' },
      { value: 'ollama', label: 'Ollama (Local)' },
    ];
    const custom = customProviders.map(cp => ({
      value: `custom:${cp.name}`,
      label: `Custom: ${cp.name}`,
    }));
    if (custom.length === 0) return builtin;
    return [...builtin, { value: '', label: '──────────', disabled: true }, ...custom];
  });

  // Find the current custom provider id when a custom provider is selected
  let selectedCustomProviderId = $derived.by(() => {
    if (!providerType.startsWith('custom:')) return null;
    const name = providerType.slice('custom:'.length);
    return customProviders.find(p => p.name === name)?.id ?? null;
  });

  // Auto-populate API key and base URL when a custom provider is selected
  $effect(() => {
    if (providerType.startsWith('custom:')) {
      const name = providerType.slice('custom:'.length);
      const cp = customProviders.find(p => p.name === name);
      if (cp) {
        apiKey = cp.api_key;
        baseUrl = cp.base_url;
      }
    }
  });

  // ── New: custom provider functions ─────────────────────────────────

  async function loadCustomProviders() {
    try {
      const providers = await getCustomProviders();
      customProviders = providers;
      const modelsMap = new SvelteMap<string, CustomProviderModel[]>();
      for (const p of providers) {
        const models = await getProviderModels(p.id);
        modelsMap.set(p.id, models);
      }
      providerModelsMap = modelsMap;
    } catch (e) {
      console.error('Failed to load custom providers:', e);
    }
  }

  async function handleAddProvider() {
    if (!newProviderName.trim() || !newProviderBaseUrl.trim()) return;
    try {
      await createCustomProvider(
        newProviderName.trim(),
        newProviderType,
        newProviderBaseUrl.trim(),
        newProviderApiKey,
      );
      newProviderName = '';
      newProviderType = 'openai';
      newProviderBaseUrl = '';
      newProviderApiKey = '';
      showAddProvider = false;
      await loadCustomProviders();
    } catch (e) {
      showError(`Failed to create provider: ${e}`);
    }
  }

  async function handleDeleteProvider(id: string) {
    try {
      await deleteCustomProvider(id);
      await loadCustomProviders();
    } catch (e) {
      showError(`Failed to delete provider: ${e}`);
    }
  }

  async function handleAddModel(providerId: string) {
    if (!newModelId.trim() || !newModelDisplayName.trim()) return;
    try {
      await addProviderModel(providerId, newModelId.trim(), newModelDisplayName.trim());
      newModelId = '';
      newModelDisplayName = '';
      const models = await getProviderModels(providerId);
      providerModelsMap.set(providerId, models);
      providerModelsMap = new SvelteMap(providerModelsMap);
    } catch (e) {
      showError(`Failed to add model: ${e}`);
    }
  }

  async function handleRemoveModel(id: string, providerId: string) {
    try {
      await removeProviderModel(id);
      const models = await getProviderModels(providerId);
      providerModelsMap.set(providerId, models);
      providerModelsMap = new SvelteMap(providerModelsMap);
    } catch (e) {
      showError(`Failed to remove model: ${e}`);
    }
  }
</script>

<div class="settings-page">
  <h2>Settings</h2>

  <!-- Status banner -->
  {#if statusMessage}
    <div class="status-banner" class:error={statusIsError} class:success={!statusIsError}>
      {statusMessage}
    </div>
  {/if}

  <!-- Current connection status -->
  <section class="status-section">
    <h3>Connection Status</h3>
    <div class="status-grid">
      <span class="label">Provider</span>
      <span class="value">{currentProviderType}</span>
      <span class="label">Model</span>
      <span class="value">{currentModel}</span>
      <span class="label">API Key</span>
      <span class="value">{apiKeyConfigured ? '✅ Configured' : '❌ Not set'}</span>
    </div>
  </section>

  <!-- Provider configuration -->
  <section class="config-section">
    <h3>LLM Provider</h3>

    <label for="provider">Provider</label>
    <select id="provider" bind:value={providerType}>
      {#each providerOptions as opt (opt.value)}
        <option value={opt.value} disabled={opt.disabled}>{opt.label}</option>
      {/each}
    </select>

    {#if showApiKey}
      <label for="api-key">API Key</label>
      <input
        id="api-key"
        type="password"
        bind:value={apiKey}
        placeholder="sk-..."
        autocomplete="off"
      />
    {/if}

    {#if providerType.startsWith('custom:')}
      <label for="model">Model</label>
      <select id="model" bind:value={model}>
        <option value="">Select a model…</option>
        {#each providerModelsMap.get(selectedCustomProviderId ?? '') ?? [] as cm (cm.id)}
          <option value={cm.model_id}>{cm.display_name}</option>
        {/each}
      </select>
    {:else}
      <label for="model">Model</label>
      <input
        id="model"
        type="text"
        bind:value={model}
        placeholder={modelPlaceholder}
      />
    {/if}

    {#if showBaseUrl}
      <label for="base-url">Base URL</label>
      <input
        id="base-url"
        type="text"
        bind:value={baseUrl}
        placeholder={baseUrlPlaceholder}
      />
    {/if}

    <div class="actions">
      <button onclick={saveSettings} disabled={isSaving}>
        {isSaving ? 'Saving…' : 'Save Settings'}
      </button>
      <button
        class="primary"
        onclick={connect}
        disabled={isConnecting || isSaving}
      >
        {isConnecting ? 'Connecting…' : 'Save &amp; Connect'}
      </button>
    </div>
  </section>

  <p class="hint">
    Need to upload rulebook PDFs? Use the main chat view.
    Once PDFs are indexed, ask questions and Chronacle will cite the sources.
  </p>

  <hr />

  <section class="config-section custom-providers-section">
    <h3>Custom Providers</h3>
    <p class="hint">Register API-compatible providers (OpenRouter, Groq, etc.)</p>

    {#if customProviders.length === 0 && !showAddProvider}
      <p class="empty-state">No custom providers configured yet.</p>
    {/if}

    {#each customProviders as cp (cp.id)}
      <div class="custom-provider-card">
        <div class="provider-header">
          <strong>{cp.name}</strong>
          <span class="type-badge">{cp.provider_type === 'openai' ? 'OpenAI-compatible' : 'Anthropic-compatible'}</span>
          <button class="small-btn" onclick={() => handleDeleteProvider(cp.id)}>Delete</button>
        </div>
        <div class="provider-detail">
          <span class="label">Base URL:</span>
          <code>{cp.base_url}</code>
        </div>
        <div class="provider-detail">
          <span class="label">Models:</span>
          {#if (providerModelsMap.get(cp.id)?.length ?? 0) === 0}
            <span class="text-muted">No models added</span>
          {:else}
            <ul class="model-list">
              {#each providerModelsMap.get(cp.id) ?? [] as modelEntry (modelEntry.id)}
                <li>
                  <span class="model-display">{modelEntry.display_name}</span>
                  <code class="model-id">{modelEntry.model_id}</code>
                  <button class="small-btn danger" onclick={() => handleRemoveModel(modelEntry.id, cp.id)}>×</button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>

        {#if editingProviderModels === cp.id}
          <div class="add-model-form">
            <input type="text" placeholder="Model ID (e.g. gpt-4o)" bind:value={newModelId} />
            <input type="text" placeholder="Display name (e.g. GPT-4o)" bind:value={newModelDisplayName} />
            <button class="small-btn primary" onclick={() => handleAddModel(cp.id)}>Add</button>
          </div>
        {/if}
        <button
          class="small-btn"
          onclick={() => {
            editingProviderModels = editingProviderModels === cp.id ? null : cp.id;
            newModelId = '';
            newModelDisplayName = '';
          }}
        >
          {editingProviderModels === cp.id ? 'Cancel' : '+ Add Model'}
        </button>
      </div>
    {/each}

    {#if showAddProvider}
      <div class="add-provider-form">
        <label for="new-provider-name">Provider Name</label>
        <input id="new-provider-name" type="text" bind:value={newProviderName} placeholder="e.g. OpenRouter" />

        <label for="new-provider-type">API Compatibility</label>
        <select id="new-provider-type" bind:value={newProviderType}>
          <option value="openai">OpenAI-compatible</option>
          <option value="anthropic">Anthropic-compatible</option>
        </select>

        <label for="new-provider-url">Base URL</label>
        <input id="new-provider-url" type="text" bind:value={newProviderBaseUrl} placeholder="https://openrouter.ai/api/v1" />

        <label for="new-provider-key">API Key (optional)</label>
        <input id="new-provider-key" type="password" bind:value={newProviderApiKey} autocomplete="off" />

        <div class="form-actions">
          <button onclick={() => { showAddProvider = false; }}>Cancel</button>
          <button class="primary" onclick={handleAddProvider}>Save Provider</button>
        </div>
      </div>
    {:else}
      <button class="small-btn primary" onclick={() => { showAddProvider = true; }}>+ Add Custom Provider</button>
    {/if}
  </section>

  <section class="config-section">
    <h3>Embedding model</h3>
    <p class="muted">
      Re-index every PDF source to apply recent improvements to text extraction,
      chunking, and embedding quality. Existing sources stay searchable during
      re-indexing; only their chunks get replaced.
    </p>
    <button class="small-btn primary" disabled={reindexing} onclick={onReindexAll}>
      {reindexing ? 'Re-indexing…' : 'Re-index all sources'}
    </button>
    {#if reindexing && reindexProgress}
      <div class="reindex-progress">
        Source {reindexProgress.current}/{reindexProgress.total}: {reindexProgress.step}
        ({Math.round(reindexProgress.progress * 100)}%)
      </div>
    {/if}
    {#if reindexError}
      <div class="reindex-error">Re-index failed: {reindexError}</div>
    {/if}
    {#if reindexedCount !== null && !reindexing}
      <div class="reindex-success">Re-indexed {reindexedCount} source(s).</div>
    {/if}
  </section>
</div>

<style>
  .settings-page {
    max-width: 540px;
    margin: 0 auto;
    padding: 1.5rem 1rem;
  }

  h2 {
    margin: 0 0 1.5rem;
    font-size: 1.25rem;
    font-weight: 700;
  }

  h3 {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 0.75rem;
    color: var(--text-muted);
  }

  .status-banner {
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    margin-bottom: 1rem;
    font-size: 0.9rem;
  }

  .status-banner.success {
    background: #14532d;
    color: #86efac;
  }

  .status-banner.error {
    background: #7f1d1d;
    color: #fca5a5;
  }

  .status-section {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1.5rem;
  }

  .status-grid {
    display: grid;
    grid-template-columns: 100px 1fr;
    gap: 0.4rem 1rem;
    font-size: 0.9rem;
  }

  .status-grid .label {
    color: var(--text-muted);
  }

  .status-grid .value {
    color: var(--text);
  }

  .config-section {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
  }

  label {
    display: block;
    font-size: 0.85rem;
    font-weight: 500;
    margin: 0.75rem 0 0.25rem;
    color: var(--text-muted);
  }

  select,
  input {
    width: 100%;
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.95rem;
    box-sizing: border-box;
  }

  select:focus,
  input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 1.25rem;
  }

  .actions button {
    flex: 1;
    padding: 0.55rem 1rem;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-assistant);
    color: var(--text);
    font-family: inherit;
    font-size: 0.9rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
  }

  .actions button:hover:not(:disabled) {
    background: var(--bg-user);
  }

  .actions button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .actions .primary {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .actions .primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .hint {
    font-size: 0.8rem;
    color: var(--text-muted);
    text-align: center;
    margin-top: 2rem;
  }

  .custom-providers-section {
    margin-top: 1.5rem;
  }

  .custom-provider-card {
    background: var(--bg-assistant);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .provider-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
  }

  .provider-header strong {
    font-size: 0.95rem;
  }

  .type-badge {
    font-size: 0.7rem;
    background: var(--bg-user);
    color: var(--text-muted);
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
  }

  .provider-detail {
    font-size: 0.85rem;
    margin-bottom: 0.3rem;
  }

  .provider-detail .label {
    color: var(--text-muted);
    margin-right: 0.25rem;
  }

  .provider-detail code {
    font-size: 0.8rem;
    color: var(--text-muted);
    background: var(--bg-input);
    padding: 0.1rem 0.3rem;
    border-radius: 3px;
  }

  .model-list {
    list-style: none;
    padding: 0;
    margin: 0.3rem 0;
  }

  .model-list li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.2rem 0;
    font-size: 0.85rem;
  }

  .model-display {
    font-weight: 500;
  }

  .model-id {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .small-btn {
    background: none;
    border: 1px solid var(--border);
    border-radius: 3px;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0.2rem 0.5rem;
    font-family: inherit;
  }

  .small-btn:hover {
    background: var(--bg-user);
    color: var(--text);
  }

  .small-btn.danger {
    color: #fca5a5;
    border-color: #7f1d1d;
  }

  .small-btn.danger:hover {
    background: #7f1d1d;
  }

  .small-btn.primary {
    background: var(--accent);
    color: #fff;
    border-color: var(--accent);
  }

  .add-provider-form {
    background: var(--bg-surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 1rem;
    margin-bottom: 0.75rem;
  }

  .add-model-form {
    display: flex;
    gap: 0.3rem;
    margin: 0.5rem 0;
    align-items: center;
  }

  .add-model-form input {
    flex: 1;
    padding: 0.3rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: 3px;
    background: var(--bg-input);
    color: var(--text);
    font-family: inherit;
    font-size: 0.85rem;
  }

  .form-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }

  .empty-state {
    color: var(--text-muted);
    font-size: 0.85rem;
    text-align: center;
    padding: 1rem;
  }

  .text-muted {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 1.5rem 0;
  }

  .muted {
    color: var(--text-muted);
    font-size: 0.85rem;
    margin: 0 0 0.75rem;
  }

  .reindex-progress {
    margin-top: 0.75rem;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .reindex-error {
    margin-top: 0.75rem;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background: #5a1e1e;
    color: #fca5a5;
    font-size: 0.85rem;
  }

  .reindex-success {
    margin-top: 0.75rem;
    padding: 0.5rem 0.75rem;
    border-radius: 6px;
    background: #14532d;
    color: #86efac;
    font-size: 0.85rem;
  }
</style>