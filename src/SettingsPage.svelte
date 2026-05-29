<script lang="ts">
  import { onMount } from 'svelte';
  import { getSettings, updateSetting, getLlmProviderStatus, reconfigureLlmProvider } from './lib/commands';

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

  onMount(async () => {
    await loadSettings();
    await loadStatus();
  });

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
    } catch (e) {
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
      // Save first, then reconfigure
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

  // Show/hide base URL for Ollama or custom endpoints
  let showBaseUrl = $derived(
    providerType === 'ollama' || (providerType === 'openai' && baseUrl !== '')
  );

  let showApiKey = $derived(providerType === 'openai' || providerType === 'anthropic');
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
      <option value="openai">OpenAI</option>
      <option value="anthropic">Anthropic</option>
      <option value="ollama">Ollama (Local)</option>
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

    <label for="model">Model</label>
    <input
      id="model"
      type="text"
      bind:value={model}
      placeholder={modelPlaceholder}
    />

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
</style>