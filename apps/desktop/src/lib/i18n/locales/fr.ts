import type { MessageCatalog } from '../messages';

const fr = {
  common: {
    save: 'Enregistrer',
    cancel: 'Annuler',
    close: 'Fermer',
    delete: 'Supprimer',
    edit: 'Modifier',
    add: 'Ajouter',
    back: 'Retour',
    continue: 'Continuer',
    confirm: 'Confirmer',
    loading: 'Chargement…',
    search: 'Rechercher',
    settings: 'Paramètres',
    language: 'Langue',
  },
  status: {
    ready: 'Prêt',
    saving: 'Enregistrement…',
    saved: 'Enregistré',
    processing: 'Traitement…',
    complete: 'Terminé',
    failed: 'Échec',
  },
  settings: {
    language: 'Langue d’affichage',
    languageDescription: 'Choisissez la langue utilisée dans Chronacle.',
    embedding: 'Représentations vectorielles',
    embeddingBackend: 'Moteur de représentations vectorielles',
    embeddingModel: 'Modèle de représentations vectorielles',
    saveSuccess: 'Paramètres enregistrés.',
  },
  progress: {
    source: 'Source {current}/{total}',
    sources: '{current} sources sur {total}',
    extracting: 'Extraction du texte…',
    uploading: 'Téléversement…',
    indexing: 'Indexation…',
    step: 'Étape {current} sur {total}',
  },
  dialog: {
    confirmDelete: 'Supprimer cet élément ?',
    discardChanges: 'Ignorer les modifications non enregistrées ?',
    cancel: 'Annuler',
    confirm: 'Confirmer',
  },
  errors: {
    generic: 'Une erreur est survenue.',
    network: 'La requête réseau a échoué.',
    saveFailed: 'Impossible d’enregistrer vos modifications.',
    loadFailed: 'Impossible de charger ce contenu.',
    validationRequired: '{field} est obligatoire.',
  },
} satisfies MessageCatalog;

export default fr;
