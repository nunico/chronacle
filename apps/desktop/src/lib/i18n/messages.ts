import type { DeepStringCatalog, MessageKeyFor } from './types';

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
    embedding: 'Embeddings',
    embeddingBackend: 'Embedding backend',
    embeddingModel: 'Embedding model',
    saveSuccess: 'Settings saved.',
  },
  progress: {
    source: 'Source {current}/{total}',
    sources: '{current} of {total} sources',
    extracting: 'Extracting text…',
    uploading: 'Uploading…',
    indexing: 'Indexing…',
    step: 'Step {current} of {total}',
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
