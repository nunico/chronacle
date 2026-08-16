import type { Component } from 'svelte';
import type { Locale } from '$lib/i18n/types';

export type ManualSectionId =
  | 'overview'
  | 'getting-started'
  | 'ai-providers'
  | 'source-library'
  | 'campaigns'
  | 'codex'
  | 'notes-and-sessions'
  | 'vault'
  | 'settings'
  | 'troubleshooting'
  | 'glossary';

export interface ManualFrontmatter {
  translationKey: string;
  locale: Locale;
  slug: string;
  title: string;
  summary: string;
  section: ManualSectionId;
  order: number;
  navTitle?: string;
  search?: boolean;
}

export interface ManualArticle extends ManualFrontmatter {
  component: Component;
  href: string;
}
