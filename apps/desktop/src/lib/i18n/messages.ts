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
