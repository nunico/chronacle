import type { MessageCatalog } from '../messages';

const de = {
  common: {
    save: 'Speichern',
    cancel: 'Abbrechen',
    close: 'Schließen',
    delete: 'Löschen',
    edit: 'Bearbeiten',
    add: 'Hinzufügen',
    back: 'Zurück',
    continue: 'Weiter',
    confirm: 'Bestätigen',
    loading: 'Wird geladen…',
    search: 'Suchen',
    settings: 'Einstellungen',
    language: 'Sprache',
  },
  status: {
    ready: 'Bereit',
    saving: 'Wird gespeichert…',
    saved: 'Gespeichert',
    processing: 'Wird verarbeitet…',
    complete: 'Abgeschlossen',
    failed: 'Fehlgeschlagen',
  },
  settings: {
    language: 'Anzeigesprache',
    languageDescription: 'Wähle die Sprache für Chronacle.',
    embedding: 'Einbettungen',
    embeddingBackend: 'Einbettungs-Backend',
    embeddingModel: 'Einbettungsmodell',
    saveSuccess: 'Einstellungen gespeichert.',
  },
  progress: {
    source: 'Quelle {current}/{total}',
    sources: '{current} von {total} Quellen',
    extracting: 'Text wird extrahiert…',
    uploading: 'Wird hochgeladen…',
    indexing: 'Wird indexiert…',
    step: 'Schritt {current} von {total}',
  },
  dialog: {
    confirmDelete: 'Dieses Element löschen?',
    discardChanges: 'Ungespeicherte Änderungen verwerfen?',
    cancel: 'Abbrechen',
    confirm: 'Bestätigen',
  },
  errors: {
    generic: 'Etwas ist schiefgelaufen.',
    network: 'Netzwerkanfrage fehlgeschlagen.',
    saveFailed: 'Deine Änderungen konnten nicht gespeichert werden.',
    loadFailed: 'Dieser Inhalt konnte nicht geladen werden.',
    validationRequired: '{field} ist erforderlich.',
  },
} satisfies MessageCatalog;

export default de;
