import type { ManualSectionId } from './types';

export const manualSections = [
  'overview',
  'getting-started',
  'ai-providers',
  'source-library',
  'campaigns',
  'codex',
  'notes-and-sessions',
  'vault',
  'settings',
  'troubleshooting',
  'glossary',
] as const satisfies readonly ManualSectionId[];
