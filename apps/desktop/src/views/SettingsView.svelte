<script lang="ts">
  import { onMount } from 'svelte';
  import {
    getSettings,
    updateSetting,
    getLlmProviderStatus,
    reconfigureLlmProvider,
  } from '../lib/commands';
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
  import Button from '../components/ui/Button.svelte';
  import FormField from '../components/ui/FormField.svelte';
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
      showError(i18n.t('settingsPage.loadFailed', { error: String(e) }));
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
      showSuccess(i18n.t('settingsPage.embeddingSaved', { model: activeModel }));
    } catch (e) {
      showError(i18n.t('settingsPage.embeddingSaveFailed', { error: String(e) }));
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
      showError(i18n.t('settingsPage.languageSaveFailed', { error: String(e) }));
    }
  }

  function queueUiLocaleWrite(locale: UiLocalePreference): Promise<void> {
    const write = uiLocaleWriteQueue.then(() => updateSetting('ui_locale', locale));
    uiLocaleWriteQueue = write.catch(() => undefined);
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
    setTimeout(() => {
      statusMessage = '';
    }, 5000);
  }

  function showSuccess(msg: string) {
    statusMessage = msg;
    statusIsError = false;
    setTimeout(() => {
      statusMessage = '';
    }, 3000);
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
      showSuccess(i18n.t('settings.saveSuccess'));
    } catch (e) {
      showError(i18n.t('settingsPage.saveFailed', { error: String(e) }));
    } finally {
      isSaving = false;
    }
  }

  async function saveEnrichNeighbors() {
    try {
      await updateSetting('extraction_enrich_neighbors', enrichNeighbors ? 'true' : 'false');
      showSuccess(i18n.t('settings.saveSuccess'));
    } catch (e) {
      enrichNeighbors = !enrichNeighbors; // revert optimistic toggle
      showError(i18n.t('settingsPage.saveFailed', { error: String(e) }));
    }
  }

  /** Client-side checks before hitting the backend: cloud providers need a
   * key, and a non-empty base URL must at least parse. Returns an error
   * message, or null when the form is valid. */
  function validateConnection(): string | null {
    if (showApiKey && !apiKey.trim()) {
      return i18n.t('settingsPage.apiKeyRequired');
    }
    if (baseUrl.trim()) {
      try {
        new URL(baseUrl.trim());
      } catch {
        return i18n.t('settingsPage.invalidBaseUrl');
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
      showSuccess(i18n.t('settingsPage.connected', { provider: activeType }));
    } catch (e) {
      showError(i18n.t('settingsPage.connectionFailed', { error: String(e) }));
    } finally {
      isConnecting = false;
    }
  }

  // ── New: derived state ─────────────────────────────────────────────

  let showBaseUrl = $derived(
    providerType === 'ollama' ||
      (providerType === 'openai' && baseUrl !== '') ||
      providerType.startsWith('custom:'),
  );

  let showApiKey = $derived(
    providerType === 'openai' || providerType === 'anthropic' || providerType.startsWith('custom:'),
  );

  let modelPlaceholder = $derived.by(() => {
    switch (providerType) {
      case 'openai':
        return 'gpt-4o-mini';
      case 'anthropic':
        return 'claude-3-5-haiku-20241022';
      case 'ollama':
        return 'llama3.2';
      default:
        return '';
    }
  });

  let baseUrlPlaceholder = $derived.by(() => {
    switch (providerType) {
      case 'ollama':
        return 'http://localhost:11434';
      case 'openai':
        return 'https://api.openai.com/v1';
      default:
        return '';
    }
  });

  // Provider options: built-in + custom providers with a separator
  let providerOptions = $derived.by(() => {
    const builtin: Array<{ value: string; label: string; disabled?: boolean }> = [
      { value: 'openai', label: 'OpenAI' },
      { value: 'anthropic', label: 'Anthropic' },
      { value: 'ollama', label: 'Ollama (Local)' },
    ];
    const custom: Array<{ value: string; label: string; disabled?: boolean }> = customProviders.map(
      (cp) => ({
        value: `custom:${cp.name}`,
        label: `Custom: ${cp.name}`,
      }),
    );
    if (custom.length === 0) return builtin;
    return [...builtin, { value: '', label: '──────────', disabled: true }, ...custom];
  });

  // Find the current custom provider id when a custom provider is selected
  let selectedCustomProviderId = $derived.by(() => {
    if (!providerType.startsWith('custom:')) return null;
    const name = providerType.slice('custom:'.length);
    return customProviders.find((p) => p.name === name)?.id ?? null;
  });

  // Auto-populate API key and base URL when a custom provider is selected
  $effect(() => {
    if (providerType.startsWith('custom:')) {
      const name = providerType.slice('custom:'.length);
      const cp = customProviders.find((p) => p.name === name);
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
    <FormField label={i18n.t('settings.language')} controlId="ui-locale">
      <select id="ui-locale" bind:value={uiLocale} onchange={saveUiLocale}>
        <option value="auto">{i18n.t('settings.languageAutomatic')}</option>
        <option value="en">{i18n.t('settings.languageEnglish')}</option>
        <option value="de">{i18n.t('settings.languageGerman')}</option>
        <option value="fr">{i18n.t('settings.languageFrench')}</option>
        <option value="es">{i18n.t('settings.languageSpanish')}</option>
      </select>
    </FormField>
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
    <h3>{i18n.t('settingsPage.connectionStatus')}</h3>
    <div class="status-grid">
      <span class="label">{i18n.t('settingsPage.provider')}</span>
      <span class="value">{currentProviderType}</span>
      <span class="label">{i18n.t('settingsPage.model')}</span>
      <span class="value">{currentModel}</span>
      <span class="label">{i18n.t('settingsPage.apiKey')}</span>
      <span class="value"
        >{apiKeyConfigured
          ? i18n.t('settingsPage.configured')
          : i18n.t('settingsPage.notSet')}</span
      >
    </div>
  </section>

  <!-- Provider configuration -->
  <section class="config-section">
    <h3>{i18n.t('settingsPage.llmProvider')}</h3>

    <label for="provider">{i18n.t('settingsPage.provider')}</label>
    <select id="provider" bind:value={providerType}>
      {#each providerOptions as opt (opt.value)}
        <option value={opt.value} disabled={opt.disabled}>{opt.label}</option>
      {/each}
    </select>

    {#if showApiKey}
      <label for="api-key">{i18n.t('settingsPage.apiKey')}</label>
      <input
        id="api-key"
        type="password"
        bind:value={apiKey}
        placeholder="sk-..."
        autocomplete="off"
      />
    {/if}

    {#if providerType.startsWith('custom:')}
      <label for="model">{i18n.t('settingsPage.model')}</label>
      <select id="model" bind:value={model}>
        <option value="">{i18n.t('settingsPage.selectModel')}</option>
        {#each providerModelsMap.get(selectedCustomProviderId ?? '') ?? [] as cm (cm.id)}
          <option value={cm.model_id}>{cm.display_name}</option>
        {/each}
      </select>
    {:else}
      <label for="model">{i18n.t('settingsPage.model')}</label>
      <input id="model" type="text" bind:value={model} placeholder={modelPlaceholder} />
    {/if}

    {#if showBaseUrl}
      <label for="base-url">{i18n.t('settingsPage.baseUrl')}</label>
      <input id="base-url" type="text" bind:value={baseUrl} placeholder={baseUrlPlaceholder} />
    {/if}

    <div class="actions">
      <Button variant="secondary" onclick={saveSettings} disabled={isSaving} loading={isSaving}>
        {i18n.t('settings.saveSettings')}
      </Button>
      <Button onclick={connect} disabled={isConnecting || isSaving} loading={isConnecting}>
        {i18n.t('settings.saveConnect')}
      </Button>
    </div>
  </section>

  <p class="hint">{i18n.t('settingsPage.uploadHint')}</p>

  <hr />

  <section class="config-section custom-providers-section">
    <h3>{i18n.t('settingsPage.customProviders')}</h3>
    <p class="hint">{i18n.t('settingsPage.customProvidersHint')}</p>

    {#if customProviders.length === 0 && !showAddProvider}
      <p class="empty-state">{i18n.t('settingsPage.noCustomProviders')}</p>
    {/if}

    {#each customProviders as cp (cp.id)}
      <div class="custom-provider-card">
        <div class="provider-header">
          <strong>{cp.name}</strong>
          <span class="type-badge"
            >{cp.provider_type === 'openai'
              ? i18n.t('settingsPage.openAiCompatible')
              : i18n.t('settingsPage.anthropicCompatible')}</span
          >
          <Button variant="danger" onclick={() => handleDeleteProvider(cp.id)}
            >{i18n.t('settingsPage.deleteProvider')}</Button
          >
        </div>
        <div class="provider-detail">
          <span class="label">{i18n.t('settingsPage.baseUrl')}:</span>
          <code>{cp.base_url}</code>
        </div>
        <div class="provider-detail">
          <span class="label">{i18n.t('settingsPage.models')}:</span>
          {#if (providerModelsMap.get(cp.id)?.length ?? 0) === 0}
            <span class="text-muted">{i18n.t('settingsPage.noModels')}</span>
          {:else}
            <ul class="model-list">
              {#each providerModelsMap.get(cp.id) ?? [] as modelEntry (modelEntry.id)}
                <li>
                  <span class="model-display">{modelEntry.display_name}</span>
                  <code class="model-id">{modelEntry.model_id}</code>
                  <Button
                    variant="danger"
                    iconOnly
                    ariaLabel={i18n.t('settingsPage.removeModel')}
                    onclick={() => handleRemoveModel(modelEntry.id, cp.id)}>×</Button
                  >
                </li>
              {/each}
            </ul>
          {/if}
        </div>

        {#if editingProviderModels === cp.id}
          <div class="add-model-form">
            <input
              type="text"
              placeholder={i18n.t('settingsPage.modelIdPlaceholder')}
              bind:value={newModelId}
            />
            <input
              type="text"
              placeholder={i18n.t('settingsPage.modelDisplayNamePlaceholder')}
              bind:value={newModelDisplayName}
            />
            <Button onclick={() => handleAddModel(cp.id)}>{i18n.t('common.add')}</Button>
          </div>
        {/if}
        <Button
          variant="secondary"
          onclick={() => {
            editingProviderModels = editingProviderModels === cp.id ? null : cp.id;
            newModelId = '';
            newModelDisplayName = '';
          }}
        >
          {editingProviderModels === cp.id
            ? i18n.t('common.cancel')
            : i18n.t('settingsPage.addModel')}
        </Button>
      </div>
    {/each}

    {#if showAddProvider}
      <div class="add-provider-form">
        <label for="new-provider-name">{i18n.t('settingsPage.providerName')}</label>
        <input
          id="new-provider-name"
          type="text"
          bind:value={newProviderName}
          placeholder={i18n.t('settingsPage.providerNamePlaceholder')}
        />

        <label for="new-provider-type">{i18n.t('settingsPage.apiCompatibility')}</label>
        <select id="new-provider-type" bind:value={newProviderType}>
          <option value="openai">{i18n.t('settingsPage.openAiCompatible')}</option>
          <option value="anthropic">{i18n.t('settingsPage.anthropicCompatible')}</option>
        </select>

        <label for="new-provider-url">{i18n.t('settingsPage.baseUrl')}</label>
        <input
          id="new-provider-url"
          type="text"
          bind:value={newProviderBaseUrl}
          placeholder="https://openrouter.ai/api/v1"
        />

        <label for="new-provider-key">{i18n.t('settingsPage.apiKeyOptional')}</label>
        <input
          id="new-provider-key"
          type="password"
          bind:value={newProviderApiKey}
          autocomplete="off"
        />

        <div class="form-actions">
          <Button
            variant="secondary"
            onclick={() => {
              showAddProvider = false;
            }}>{i18n.t('common.cancel')}</Button
          >
          <Button onclick={handleAddProvider}>{i18n.t('settingsPage.saveProvider')}</Button>
        </div>
      </div>
    {:else}
      <Button
        onclick={() => {
          showAddProvider = true;
        }}>{i18n.t('settingsPage.addCustomProvider')}</Button
      >
    {/if}
  </section>

  <section class="config-section">
    <h3>{i18n.t('settingsPage.embeddingProvider')}</h3>
    <p class="muted">{i18n.t('settingsPage.embeddingDescription')}</p>

    {#if embeddingStatus}
      <div class="status-grid">
        <span class="label">{i18n.t('settingsPage.active')}</span>
        <span class="value"
          >{embeddingStatus.backend === 'openai'
            ? i18n.t('settingsPage.cloudEmbedding')
            : i18n.t('settingsPage.localEmbedding')}</span
        >
        <span class="label">{i18n.t('settingsPage.model')}</span>
        <span class="value">{embeddingStatus.model}</span>
        <span class="label">{i18n.t('settingsPage.dimension')}</span>
        <span class="value">{embeddingStatus.dimension}</span>
      </div>
    {/if}

    {#if embeddingStatus && !embeddingStatus.local_available}
      <p class="muted warn">{i18n.t('settingsPage.localUnavailable')}</p>
    {/if}

    <label for="embed-backend">{i18n.t('settingsPage.backend')}</label>
    <select id="embed-backend" bind:value={embeddingBackend}>
      <option value="local" disabled={embeddingStatus ? !embeddingStatus.local_available : false}>
        {i18n.t('settingsPage.localEmbedding')}
      </option>
      <option value="openai">{i18n.t('settingsPage.cloudEmbedding')}</option>
    </select>

    {#if embeddingBackend === 'openai'}
      <label for="embed-model">{i18n.t('settingsPage.model')}</label>
      <input
        id="embed-model"
        type="text"
        bind:value={embeddingModel}
        placeholder="text-embedding-3-small"
      />

      <label for="embed-api-key">{i18n.t('settingsPage.apiKey')}</label>
      <input
        id="embed-api-key"
        type="password"
        bind:value={embeddingApiKey}
        placeholder="sk-..."
        autocomplete="off"
      />

      <label for="embed-base-url"
        >{i18n.t('settingsPage.baseUrl')}
        <span class="muted">({i18n.t('settingsPage.optional')})</span></label
      >
      <input
        id="embed-base-url"
        type="text"
        bind:value={embeddingBaseUrl}
        placeholder="https://api.openai.com/v1"
      />
    {/if}

    <div class="actions">
      <Button
        onclick={saveEmbeddingSettings}
        disabled={isSavingEmbedding}
        loading={isSavingEmbedding}>{i18n.t('settingsPage.saveEmbeddingProvider')}</Button
      >
    </div>
  </section>

  <section class="config-section">
    <h3>{i18n.t('settingsPage.reindexSources')}</h3>
    <p class="muted">{i18n.t('settingsPage.reindexDescription')}</p>
    <Button disabled={reindexing} onclick={onReindexAll} loading={reindexing}
      >{i18n.t('settingsPage.reindexAll')}</Button
    >
    {#if reindexing && reindexProgress}
      <div class="reindex-progress">
        {i18n.t('settingsPage.reindexProgress', {
          current: reindexProgress.current,
          total: reindexProgress.total,
          step: reindexProgress.step,
          progress: `${Math.round(reindexProgress.progress * 100)}%`,
        })}
      </div>
    {/if}
    {#if reindexError}
      <div class="reindex-error">
        {i18n.t('settingsPage.reindexFailed', { error: reindexError })}
      </div>
    {/if}
    {#if reindexedCount !== null && !reindexing}
      <div class="reindex-success">
        {i18n.t('settingsPage.reindexed', { count: reindexedCount })}
      </div>
    {/if}
  </section>

  <section class="config-section">
    <h3>{i18n.t('settingsPage.relationshipGraph')}</h3>
    <p class="muted">{i18n.t('settingsPage.relationshipDescription')}</p>
    <Button disabled={resyncing} onclick={onResyncWikilinks} loading={resyncing}
      >{i18n.t('settingsPage.rebuildLinks')}</Button
    >
    {#if resyncError}
      <div class="reindex-error">
        {i18n.t('settingsPage.rebuildFailed', { error: resyncError })}
      </div>
    {/if}
    {#if resyncedCount !== null && !resyncing}
      <div class="reindex-success">{i18n.t('settingsPage.rebuilt', { count: resyncedCount })}</div>
    {/if}
  </section>

  <section class="config-section">
    <h3>{i18n.t('settingsPage.entityExtraction')}</h3>
    <label class="toggle-row">
      <input type="checkbox" bind:checked={enrichNeighbors} onchange={saveEnrichNeighbors} />
      <span>{i18n.t('settingsPage.enrichEntities')}</span>
    </label>
    <p class="muted">{i18n.t('settingsPage.enrichDescription')}</p>
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
