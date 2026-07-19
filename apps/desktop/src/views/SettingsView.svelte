<script lang="ts">
  import { onMount } from 'svelte';
  import { getSettings, updateSetting, getLlmProviderStatus, reconfigureLlmProvider } from '../lib/commands';
  import {
    getEmbeddingProviderStatus,
    reconfigureEmbeddingProvider,
    type EmbeddingProviderStatus,
    getCustomProviders,
    createCustomProvider,
    deleteCustomProvider,
    getProviderModels,
    addProviderModel,
    removeProviderModel,
    reindexAllSources,
    resyncWikilinks,
    type CustomProvider,
    type CustomProviderModel,
    type ReindexProgress,
  } from '../lib/commands';
  import { listen } from '@tauri-apps/api/event';
  import { SvelteMap } from 'svelte/reactivity';
  import VaultSyncSettings from '../components/VaultSyncSettings.svelte';
  import {
    i18n,
    setUiLocalePreference,
    uiLocalePreference,
    type UiLocalePreference,
  } from '../lib/locale.svelte';

  let providerType = $state('openai');
  let apiKey = $state('');
  let model = $state('');
  let baseUrl = $state('');
  let enrichNeighbors = $state(false);
  let uiLocale = $state<UiLocalePreference>(uiLocalePreference());
  let persistedUiLocale = uiLocalePreference();
  let uiLocaleSaveVersion = 0;
  let uiLocaleWriteQueue: Promise<void> = Promise.resolve();

  let isSaving = $state(false);
  let isConnecting = $state(false);
  let statusMessage = $state('');
  let statusIsError = $state(false);

  let currentProviderType = $state('—');
  let currentModel = $state('—');
  let apiKeyConfigured = $state(false);

  // Embedding provider state
  let embeddingBackend = $state('local'); // 'local' | 'openai'
  let embeddingModel = $state('');
  let embeddingApiKey = $state('');
  let embeddingBaseUrl = $state('');
  let embeddingStatus = $state<EmbeddingProviderStatus | null>(null);
  let isSavingEmbedding = $state(false);

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

  // Resync wikilinks state
  let resyncing = $state(false);
  let resyncError = $state<string | null>(null);
  let resyncedCount = $state<number | null>(null);

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

  async function onResyncWikilinks() {
    resyncing = true;
    resyncError = null;
    resyncedCount = null;
    try {
      const count = await resyncWikilinks();
      resyncedCount = count;
    } catch (e) {
      resyncError = String(e);
    } finally {
      resyncing = false;
    }
  }

  onMount(async () => {
    await loadSettings();
    await loadStatus();
    await loadEmbeddingStatus();
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
      enrichNeighbors = settings['extraction_enrich_neighbors'] === 'true';
      setUiLocalePreference(settings['ui_locale']);
      uiLocale = uiLocalePreference();
      persistedUiLocale = uiLocale;
      embeddingModel = settings['embedding_model'] ?? '';
      embeddingApiKey = settings['embedding_api_key'] ?? '';
      embeddingBaseUrl = settings['embedding_base_url'] ?? '';
      // Default backend follows platform capability when unset (resolved by the
      // backend); seed the control from the live status in loadEmbeddingStatus.
      embeddingBackend = settings['embedding_backend'] ?? embeddingBackend;
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

  async function loadEmbeddingStatus() {
    try {
      const status = await getEmbeddingProviderStatus();
      embeddingStatus = status;
      // If the user has never explicitly chosen a backend, reflect the active one.
      const settings = await getSettings();
      if (settings['embedding_backend'] === undefined) {
        embeddingBackend = status.backend;
      }
    } catch {
      // Status unavailable on first load; that's fine.
    }
  }

  async function saveEmbeddingSettings() {
    isSavingEmbedding = true;
    statusMessage = '';
    try {
      await Promise.all([
        updateSetting('embedding_backend', embeddingBackend),
        updateSetting('embedding_model', embeddingModel),
        updateSetting('embedding_api_key', embeddingApiKey),
        updateSetting('embedding_base_url', embeddingBaseUrl),
      ]);
      const activeModel = await reconfigureEmbeddingProvider();
      await loadEmbeddingStatus();
      showSuccess(
        `Embedding provider set to ${activeModel}. Re-index existing sources below to apply it.`,
      );
    } catch (e) {
      showError(`Failed to save embedding settings: ${e}`);
    } finally {
      isSavingEmbedding = false;
    }
  }

  async function saveUiLocale(): Promise<void> {
    const saveVersion = ++uiLocaleSaveVersion;
    const selectedLocale = uiLocale;
    setUiLocalePreference(selectedLocale);
    try {
      await queueUiLocaleWrite(selectedLocale);
      if (saveVersion === uiLocaleSaveVersion) persistedUiLocale = selectedLocale;
    } catch (e) {
      if (saveVersion === uiLocaleSaveVersion) {
        uiLocale = persistedUiLocale;
        setUiLocalePreference(persistedUiLocale);
        try {
          await queueUiLocaleWrite(persistedUiLocale);
        } catch {
          await reconcileUiLocale();
        }
      }
      showError(`Failed to save language: ${e}`);
    }
  }

  function queueUiLocaleWrite(locale: UiLocalePreference): Promise<void> {
    const write = uiLocaleWriteQueue.then(() => updateSetting('ui_locale', locale));
    uiLocaleWriteQueue = write.catch(() => {});
    return write;
  }

  async function reconcileUiLocale(): Promise<void> {
    try {
      const settings = await getSettings();
      setUiLocalePreference(settings['ui_locale']);
      uiLocale = uiLocalePreference();
      persistedUiLocale = uiLocale;
    } catch {
      // Keep the best known preference when settings cannot be reloaded.
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

  async function saveEnrichNeighbors() {
    try {
      await updateSetting('extraction_enrich_neighbors', enrichNeighbors ? 'true' : 'false');
      showSuccess('Settings saved.');
    } catch (e) {
      enrichNeighbors = !enrichNeighbors; // revert optimistic toggle
      showError(`Failed to save: ${e}`);
    }
  }

  /** Client-side checks before hitting the backend: cloud providers need a
   * key, and a non-empty base URL must at least parse. Returns an error
   * message, or null when the form is valid. */
  function validateConnection(): string | null {
    if (showApiKey && !apiKey.trim()) {
      return 'An API key is required for this provider.';
    }
    if (baseUrl.trim()) {
      try {
        new URL(baseUrl.trim());
      } catch {
        return 'The base URL is not a valid URL (expected e.g. http://localhost:11434).';
      }
    }
    return null;
  }

  async function connect() {
    const validationError = validateConnection();
    if (validationError) {
      showError(validationError);
      return;
    }
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
  <h2>{i18n.t('common.settings')}</h2>

  <section class="config-section">
    <h3>{i18n.t('common.language')}</h3>
    <label for="ui-locale">{i18n.t('settings.language')}</label>
    <select id="ui-locale" bind:value={uiLocale} onchange={saveUiLocale}>
      <option value="auto">{i18n.t('settings.languageAutomatic')}</option>
      <option value="en">{i18n.t('settings.languageEnglish')}</option>
      <option value="de">{i18n.t('settings.languageGerman')}</option>
      <option value="fr">{i18n.t('settings.languageFrench')}</option>
      <option value="es">{i18n.t('settings.languageSpanish')}</option>
    </select>
    <p class="muted">{i18n.t('settings.languageDescription')}</p>
  </section>

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
      <span class="value">{apiKeyConfigured ? 'Configured' : 'Not set'}</span>
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
    <h3>Embedding Provider</h3>
    <p class="muted">
      How document and query text is turned into vectors for search. The local
      model runs offline; the cloud option uses an OpenAI-compatible API at 768
      dimensions (matching the local index, so switching only requires re-indexing).
    </p>

    {#if embeddingStatus}
      <div class="status-grid">
        <span class="label">Active</span>
        <span class="value">{embeddingStatus.backend === 'openai' ? 'Cloud (OpenAI)' : 'Local (fastembed)'}</span>
        <span class="label">Model</span>
        <span class="value">{embeddingStatus.model}</span>
        <span class="label">Dimension</span>
        <span class="value">{embeddingStatus.dimension}</span>
      </div>
    {/if}

    {#if embeddingStatus && !embeddingStatus.local_available}
      <p class="muted warn">
        The local embedding model is not available on this computer (no ONNX
        Runtime build is published for Intel Macs). Configure a cloud embedding
        provider below to enable search.
      </p>
    {/if}

    <label for="embed-backend">Backend</label>
    <select id="embed-backend" bind:value={embeddingBackend}>
      <option value="local" disabled={embeddingStatus ? !embeddingStatus.local_available : false}>
        Local — nomic-embed-text-v1.5 (offline)
      </option>
      <option value="openai">Cloud — OpenAI-compatible API</option>
    </select>

    {#if embeddingBackend === 'openai'}
      <label for="embed-model">Model</label>
      <input
        id="embed-model"
        type="text"
        bind:value={embeddingModel}
        placeholder="text-embedding-3-small"
      />

      <label for="embed-api-key">API Key</label>
      <input
        id="embed-api-key"
        type="password"
        bind:value={embeddingApiKey}
        placeholder="sk-..."
        autocomplete="off"
      />

      <label for="embed-base-url">Base URL <span class="muted">(optional)</span></label>
      <input
        id="embed-base-url"
        type="text"
        bind:value={embeddingBaseUrl}
        placeholder="https://api.openai.com/v1"
      />
    {/if}

    <div class="actions">
      <button class="primary" onclick={saveEmbeddingSettings} disabled={isSavingEmbedding}>
        {isSavingEmbedding ? 'Saving…' : 'Save Embedding Provider'}
      </button>
    </div>
  </section>

  <section class="config-section">
    <h3>Re-index sources</h3>
    <p class="muted">
      Re-index every PDF source to apply recent improvements to text extraction,
      chunking, and embedding quality — or after changing the embedding provider
      above. Existing sources stay searchable during re-indexing; only their
      chunks get replaced.
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

  <section class="config-section">
    <h3>Relationship Graph</h3>
    <p class="muted">
      Re-scan every note's [[links]] and rebuild the relationship graph. Useful
      after importing notes or for entities created before linking existed.
    </p>
    <button class="small-btn primary" disabled={resyncing} onclick={onResyncWikilinks}>
      {resyncing ? 'Rebuilding…' : 'Rebuild relationship links'}
    </button>
    {#if resyncError}
      <div class="reindex-error">Rebuild failed: {resyncError}</div>
    {/if}
    {#if resyncedCount !== null && !resyncing}
      <div class="reindex-success">Rebuilt links across {resyncedCount} entities.</div>
    {/if}
  </section>

  <section class="config-section">
    <h3>Entity Extraction</h3>
    <label class="toggle-row">
      <input
        type="checkbox"
        bind:checked={enrichNeighbors}
        onchange={saveEnrichNeighbors}
      />
      <span>Enrich related entities</span>
    </label>
    <p class="muted">
      After extracting an entity, run a second pass that re-searches the rulebook
      for each related entity and rewrites its summary to describe the entity
      itself rather than its link to the original. More accurate, but slower and
      uses more LLM calls. Capped at 20 related entities per extraction.
    </p>
  </section>

  <VaultSyncSettings />
</div>

<style>
.settings-page {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 28px 26px 40px;
  font-family: var(--font-sans);
}
.settings-page > * {
  max-width: 720px;
  margin-left: auto;
  margin-right: auto;
}
h2 {
  font-family: var(--font-display);
  font-size: 28px;
  margin: 0 0 22px;
  color: var(--fg-1);
}
h3 {
  font-family: var(--font-sans);
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--arcane-300);
  margin: 0 0 12px;
}
.status-banner {
  padding: 10px 14px;
  border-radius: var(--r-md);
  margin-bottom: 16px;
  font-size: 13.5px;
  border: 1px solid var(--line);
}
.status-banner.success {
  background: var(--success-bg);
  color: var(--success);
  border-color: rgba(79, 209, 160, 0.4);
}
.status-banner.error {
  background: var(--danger-bg);
  color: var(--danger);
  border-color: rgba(242, 103, 75, 0.4);
}
.status-section,
.config-section {
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--r-lg);
  padding: 18px 18px 16px;
  margin-bottom: 16px;
  box-shadow: var(--shadow-card);
}
.status-grid {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 6px 14px;
  font-size: 14px;
  color: var(--fg-2);
}
.status-grid .label {
  color: var(--fg-3);
}
label {
  display: block;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--fg-3);
  margin: 14px 0 6px;
  letter-spacing: 0.02em;
}
select,
input {
  width: 100%;
  padding: 9px 12px;
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  background: var(--bg-inset);
  color: var(--fg-1);
  font-family: var(--font-sans);
  font-size: 14px;
  box-sizing: border-box;
}
select:focus,
input:focus {
  outline: none;
  border-color: var(--line-glow);
  box-shadow: var(--glow-focus);
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 18px;
}
.actions button {
  flex: 1;
  padding: 10px 14px;
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  background: var(--bg-panel-2);
  color: var(--fg-1);
  font-family: var(--font-sans);
  font-weight: 600;
  font-size: 13.5px;
}
.actions button:hover:not(:disabled) {
  border-color: var(--line-strong);
}
.actions .primary {
  background: var(--grad-arcane);
  border-color: transparent;
  color: var(--fg-on-accent);
  box-shadow: var(--glow-arcane);
}
.actions .primary:hover:not(:disabled) {
  filter: brightness(1.08);
}
.actions button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.hint {
  font-size: 12.5px;
  color: var(--fg-3);
  text-align: center;
  margin: 24px 0 16px;
}
.muted {
  font-size: 13px;
  color: var(--fg-3);
  margin: 0 0 10px;
}
.muted.warn {
  color: #c2410c;
}
.toggle-row {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  margin-bottom: 6px;
}
.toggle-row input {
  width: auto;
  margin: 0;
}
hr {
  border: none;
  border-top: 1px solid var(--line-faint);
  margin: 24px 0;
}
.custom-provider-card {
  background: var(--bg-panel-2);
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  padding: 12px 14px;
  margin-bottom: 12px;
}
.provider-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.type-badge {
  font-size: 11px;
  background: rgba(91, 120, 255, 0.12);
  color: var(--arcane-300);
  padding: 2px 6px;
  border-radius: var(--r-sm);
  font-family: var(--font-mono);
}
.provider-detail {
  font-size: 13px;
  color: var(--fg-2);
  margin-bottom: 6px;
}
.provider-detail .label {
  color: var(--fg-3);
  margin-right: 4px;
}
.provider-detail code {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--arcane-300);
  background: var(--bg-inset);
  padding: 2px 6px;
  border-radius: 4px;
}
.model-list {
  list-style: none;
  padding: 0;
  margin: 4px 0;
}
.model-list li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 0;
  font-size: 13px;
}
.model-id {
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--fg-3);
}
.small-btn {
  background: none;
  border: 1px solid var(--line);
  border-radius: var(--r-sm);
  color: var(--fg-2);
  cursor: pointer;
  font-size: 12px;
  padding: 4px 9px;
  font-family: var(--font-sans);
}
.small-btn:hover {
  border-color: var(--line-strong);
  color: var(--fg-1);
}
.small-btn.danger {
  color: var(--danger);
  border-color: rgba(242, 103, 75, 0.4);
}
.small-btn.danger:hover {
  background: var(--danger-bg);
}
.small-btn.primary {
  background: var(--grad-arcane);
  border-color: transparent;
  color: var(--fg-on-accent);
}
.add-provider-form,
.add-model-form {
  background: var(--bg-inset);
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  padding: 12px 14px;
  margin-bottom: 12px;
}
.add-model-form {
  display: flex;
  gap: 6px;
  align-items: center;
}
.add-model-form input {
  flex: 1;
  padding: 6px 10px;
  font-size: 13px;
}
.form-actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}
.empty-state {
  color: var(--fg-3);
  font-size: 13px;
  text-align: center;
  padding: 12px;
}
.text-muted {
  color: var(--fg-3);
  font-size: 13px;
}
.reindex-progress {
  margin-top: 10px;
  font-size: 13px;
  color: var(--fg-3);
  font-family: var(--font-mono);
}
.reindex-error {
  margin-top: 10px;
  padding: 8px 12px;
  border-radius: var(--r-md);
  background: var(--danger-bg);
  color: var(--danger);
  font-size: 13px;
}
.reindex-success {
  margin-top: 10px;
  padding: 8px 12px;
  border-radius: var(--r-md);
  background: var(--success-bg);
  color: var(--success);
  font-size: 13px;
}
</style>
