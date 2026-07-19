import type { DeepStringCatalog, MessageKeyFor, MessageParameters } from './types';

export const sourceCatalog = {
  common: {
    save: 'Save',
    cancel: 'Cancel',
    close: 'Close',
    delete: 'Delete',
    edit: 'Edit',
    add: 'Add',
    back: 'Back',
    continue: 'Continue',
    confirm: 'Confirm',
    loading: 'Loading…',
    search: 'Search',
    settings: 'Settings',
    language: 'Language',
    dismiss: 'Dismiss',
  },
  status: {
    ready: 'Ready',
    saving: 'Saving…',
    saved: 'Saved',
    processing: 'Processing…',
    complete: 'Complete',
    failed: 'Failed',
  },
  settings: {
    language: 'Display language',
    languageDescription: 'Choose the language used throughout Chronacle.',
    languageAutomatic: 'Automatic',
    languageEnglish: 'English',
    languageGerman: 'Deutsch',
    languageFrench: 'Français',
    languageSpanish: 'Español',
    embedding: 'Embeddings',
    embeddingBackend: 'Embedding backend',
    embeddingModel: 'Embedding model',
    saveSuccess: 'Settings saved.',
    saveSettings: 'Save settings',
    saveConnect: 'Save & connect',
  },
  progress: {
    uploadProgress: 'Upload progress',
    source: 'Source {current}/{total}',
    sources: '{current} of {total} sources',
    extracting: 'Extracting text…',
    uploading: 'Uploading…',
    indexing: 'Indexing…',
    step: 'Step {current} of {total}',
  },
  modelDownload: {
    title: 'AI model required',
    description:
      'Chronacle needs to download an AI embedding model before you can ask questions about your PDFs. This is a one-time download of approximately 250 MB from Hugging Face.',
    checking: 'Checking local cache…',
    start: 'Start download',
    connecting: 'Connecting to Hugging Face…',
    downloading: 'Downloading {name}…',
    ready: 'Model ready!',
    failed: 'Download failed',
    retry: 'Retry',
  },
  settingsPage: {
    loadFailed: 'Failed to load settings: {error}',
    saveFailed: 'Failed to save: {error}',
    languageSaveFailed: 'Failed to save language: {error}',
    embeddingSaveFailed: 'Failed to save embedding settings: {error}',
    embeddingSaved:
      'Embedding provider set to {model}. Re-index existing sources below to apply it.',
    connectionFailed: 'Connection failed: {error}',
    connected: 'Connected: {provider}',
    apiKeyRequired: 'An API key is required for this provider.',
    invalidBaseUrl: 'The base URL is not a valid URL (expected e.g. http://localhost:11434).',
    connectionStatus: 'Connection status',
    provider: 'Provider',
    model: 'Model',
    apiKey: 'API key',
    configured: 'Configured',
    notSet: 'Not set',
    defaultModel: '(default)',
    ollamaLocal: 'Ollama (local)',
    customProvider: 'Custom: {name}',
    createProviderFailed: 'Failed to create provider: {error}',
    deleteProviderFailed: 'Failed to delete provider: {error}',
    addModelFailed: 'Failed to add model: {error}',
    removeModelFailed: 'Failed to remove model: {error}',
    llmProvider: 'LLM provider',
    selectModel: 'Select a model…',
    baseUrl: 'Base URL',
    uploadHint:
      'Need to upload rulebook PDFs? Use the main chat view. Once PDFs are indexed, ask questions and Chronacle will cite the sources.',
    customProviders: 'Custom providers',
    customProvidersHint: 'Register API-compatible providers (OpenRouter, Groq, etc.)',
    noCustomProviders: 'No custom providers configured yet.',
    openAiCompatible: 'OpenAI-compatible',
    anthropicCompatible: 'Anthropic-compatible',
    models: 'Models',
    noModels: 'No models added',
    modelIdPlaceholder: 'Model ID (e.g. gpt-4o)',
    modelDisplayNamePlaceholder: 'Display name (e.g. GPT-4o)',
    addModel: 'Add model',
    providerName: 'Provider name',
    providerNamePlaceholder: 'e.g. OpenRouter',
    apiCompatibility: 'API compatibility',
    apiKeyOptional: 'API key (optional)',
    saveProvider: 'Save provider',
    addCustomProvider: 'Add custom provider',
    deleteProvider: 'Delete provider',
    removeModel: 'Remove model',
    embeddingProvider: 'Embedding provider',
    embeddingDescription:
      'How document and query text is turned into vectors for search. The local model runs offline; the cloud option uses an OpenAI-compatible API at 768 dimensions (matching the local index, so switching only requires re-indexing).',
    active: 'Active',
    dimension: 'Dimension',
    backend: 'Backend',
    localEmbedding: 'Local — nomic-embed-text-v1.5 (offline)',
    cloudEmbedding: 'Cloud — OpenAI-compatible API',
    optional: 'optional',
    saveEmbeddingProvider: 'Save embedding provider',
    localUnavailable:
      'The local embedding model is not available on this computer. Configure a cloud embedding provider below to enable search.',
    reindexSources: 'Re-index sources',
    reindexDescription:
      'Re-index every PDF source to apply recent improvements to text extraction, chunking, and embedding quality — or after changing the embedding provider above. Existing sources stay searchable during re-indexing; only their chunks get replaced.',
    reindexing: 'Re-indexing…',
    reindexAll: 'Re-index all sources',
    reindexProgress: 'Source {current}/{total}: {step} ({progress})',
    reindexFailed: 'Re-index failed: {error}',
    reindexed: 'Re-indexed {count} source(s).',
    relationshipGraph: 'Relationship graph',
    relationshipDescription:
      'Re-scan every note’s [[links]] and rebuild the relationship graph. Useful after importing notes or for entities created before linking existed.',
    rebuilding: 'Rebuilding…',
    rebuildLinks: 'Rebuild relationship links',
    rebuildFailed: 'Rebuild failed: {error}',
    rebuilt: 'Rebuilt links across {count} entities.',
    entityExtraction: 'Entity extraction',
    enrichEntities: 'Enrich related entities',
    enrichDescription:
      'After extracting an entity, run a second pass that re-searches the rulebook for each related entity and rewrites its summary to describe the entity itself rather than its link to the original. More accurate, but slower and uses more LLM calls. Capped at 20 related entities per extraction.',
  },
  dialog: {
    confirmDelete: 'Delete this item?',
    discardChanges: 'Discard unsaved changes?',
    cancel: 'Cancel',
    confirm: 'Confirm',
  },
  errors: {
    generic: 'Something went wrong.',
    network: 'Network request failed.',
    saveFailed: 'Could not save your changes.',
    loadFailed: 'Could not load this content.',
    validationRequired: '{field} is required.',
  },
} as const;

export type MessageCatalog = DeepStringCatalog<typeof sourceCatalog>;
export type MessageKey = MessageKeyFor<MessageCatalog>;

type MessageAtPath<T, Path extends string> = Path extends `${infer Head}.${infer Tail}`
  ? Head extends keyof T
    ? MessageAtPath<T[Head], Tail>
    : never
  : Path extends keyof T
    ? T[Path]
    : never;

type PlaceholderName<Message extends string> =
  Message extends `${string}{${infer Name}}${infer Rest}` ? Name | PlaceholderName<Rest> : never;

export type MessageParametersFor<Key extends MessageKey> = Record<
  PlaceholderName<Extract<MessageAtPath<typeof sourceCatalog, Key>, string>>,
  string | number
>;

type IsUnion<Type, Whole = Type> = Type extends unknown
  ? [Whole] extends [Type]
    ? false
    : true
  : never;

export type TranslationArguments<Key extends MessageKey> =
  IsUnion<Key> extends true
    ? [parameters?: MessageParameters]
    : keyof MessageParametersFor<Key> extends never
      ? [parameters?: never]
      : [parameters: MessageParametersFor<Key>];
