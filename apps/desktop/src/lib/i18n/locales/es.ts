import type { MessageCatalog } from '../messages';

const es = {
  common: {
    save: 'Guardar',
    cancel: 'Cancelar',
    close: 'Cerrar',
    delete: 'Eliminar',
    edit: 'Editar',
    add: 'Añadir',
    back: 'Volver',
    continue: 'Continuar',
    confirm: 'Confirmar',
    loading: 'Cargando…',
    search: 'Buscar',
    settings: 'Configuración',
    language: 'Idioma',
  },
  status: {
    ready: 'Listo',
    saving: 'Guardando…',
    saved: 'Guardado',
    processing: 'Procesando…',
    complete: 'Completado',
    failed: 'Error',
  },
  settings: {
    language: 'Idioma de visualización',
    languageDescription: 'Elige el idioma que se usa en Chronacle.',
    languageAutomatic: 'Automático',
    languageEnglish: 'English',
    languageGerman: 'Deutsch',
    languageFrench: 'Français',
    languageSpanish: 'Español',
    embedding: 'Representaciones vectoriales',
    embeddingBackend: 'Backend de representaciones vectoriales',
    embeddingModel: 'Modelo de representaciones vectoriales',
    saveSuccess: 'Configuración guardada.',
  },
  progress: {
    source: 'Fuente {current}/{total}',
    sources: '{current} de {total} fuentes',
    extracting: 'Extrayendo texto…',
    uploading: 'Subiendo…',
    indexing: 'Indexando…',
    step: 'Paso {current} de {total}',
  },
  dialog: {
    confirmDelete: '¿Eliminar este elemento?',
    discardChanges: '¿Descartar los cambios sin guardar?',
    cancel: 'Cancelar',
    confirm: 'Confirmar',
  },
  errors: {
    generic: 'Algo salió mal.',
    network: 'La solicitud de red falló.',
    saveFailed: 'No se pudieron guardar los cambios.',
    loadFailed: 'No se pudo cargar este contenido.',
    validationRequired: 'Se requiere {field}.',
  },
} satisfies MessageCatalog;

export default es;
