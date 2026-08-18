import type { Component } from 'svelte';
import type { Pathname } from '$app/types';
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
  headings?: ManualHeading[];
}

export interface ManualArticle extends ManualFrontmatter {
  component: Component;
  href: Pathname;
  source: string;
  links: string[];
}

export interface ManualHeading {
  id: string;
  text: string;
  level: 2 | 3;
}
