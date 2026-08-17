import type { Component } from 'svelte';
import { manualBase } from '$lib/i18n/locale';
import type { Locale, ManualSegment } from '$lib/i18n/types';
import { manualSections } from './sections';
import type { ManualArticle, ManualFrontmatter, ManualSectionId } from './types';

interface MarkdownModule {
  default: unknown;
  metadata?: unknown;
}

const markdownModules = import.meta.glob<MarkdownModule>('/src/content/manual/**/*.md', {
  eager: true,
});

const locales: readonly Locale[] = ['en', 'de'];
const slugPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*(?:\/[a-z0-9]+(?:-[a-z0-9]+)*)*$/;
const sectionOrder = new Map<ManualSectionId, number>(
  manualSections.map((section, index) => [section, index]),
);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isLocale(value: unknown): value is Locale {
  return value === 'en' || value === 'de';
}

function isSection(value: unknown): value is ManualSectionId {
  return typeof value === 'string' && manualSections.some((section) => section === value);
}

function isComponent(value: unknown): value is Component {
  return typeof value === 'function';
}

function requireString(
  metadata: Record<string, unknown>,
  field: keyof ManualFrontmatter,
  source: string,
): string {
  const value = metadata[field];
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`Manual article ${source} has invalid ${field}`);
  }
  return value;
}

function parseFrontmatter(value: unknown, source: string): ManualFrontmatter {
  if (!isRecord(value)) {
    throw new Error(`Manual article ${source} is missing frontmatter`);
  }

  const locale = value.locale;
  if (!isLocale(locale)) {
    throw new Error(`Manual article ${source} has unknown locale`);
  }

  const section = value.section;
  if (!isSection(section)) {
    throw new Error(`Manual article ${source} has unknown section`);
  }

  if (typeof value.order !== 'number' || !Number.isFinite(value.order)) {
    throw new Error(`Manual article ${source} has invalid order`);
  }
  if (
    value.navTitle !== undefined &&
    (typeof value.navTitle !== 'string' || value.navTitle === '')
  ) {
    throw new Error(`Manual article ${source} has invalid navTitle`);
  }
  if (value.search !== undefined && typeof value.search !== 'boolean') {
    throw new Error(`Manual article ${source} has invalid search`);
  }
  if (
    value.headings !== undefined &&
    (!Array.isArray(value.headings) ||
      value.headings.some(
        (heading) =>
          !isRecord(heading) ||
          typeof heading.id !== 'string' ||
          heading.id === '' ||
          typeof heading.text !== 'string' ||
          heading.text === '' ||
          (heading.level !== 2 && heading.level !== 3),
      ))
  ) {
    throw new Error(`Manual article ${source} has invalid headings`);
  }

  return {
    translationKey: requireString(value, 'translationKey', source),
    locale,
    slug: requireString(value, 'slug', source),
    title: requireString(value, 'title', source),
    summary: requireString(value, 'summary', source),
    section,
    order: value.order,
    ...(value.navTitle === undefined ? {} : { navTitle: value.navTitle }),
    ...(value.search === undefined ? {} : { search: value.search }),
    ...(value.headings === undefined
      ? {}
      : {
          headings: value.headings.map((heading) => ({
            id: String((heading as Record<string, unknown>).id),
            text: String((heading as Record<string, unknown>).text),
            level: (heading as Record<string, unknown>).level as 2 | 3,
          })),
        }),
  };
}

function toArticle(source: string, module: MarkdownModule): ManualArticle {
  const frontmatter = parseFrontmatter(module.metadata, source);
  if (!isComponent(module.default)) {
    throw new Error(`Manual article ${source} has invalid component`);
  }

  return {
    ...frontmatter,
    component: module.default,
    href:
      frontmatter.section === 'overview'
        ? manualBase(frontmatter.locale)
        : `${manualBase(frontmatter.locale)}/${frontmatter.slug}`,
  };
}

function compareCodePoints(left: string, right: string): number {
  if (left < right) {
    return -1;
  }
  if (left > right) {
    return 1;
  }
  return 0;
}

function compareArticles(left: ManualArticle, right: ManualArticle): number {
  return (
    (sectionOrder.get(left.section) ?? manualSections.length) -
      (sectionOrder.get(right.section) ?? manualSections.length) ||
    left.order - right.order ||
    compareCodePoints(left.locale, right.locale) ||
    compareCodePoints(left.slug, right.slug)
  );
}

const articles = Object.entries(markdownModules)
  .map(([source, module]) => toArticle(source, module))
  .sort(compareArticles);

validateArticles(articles);

export function articlesFor(locale: Locale): ManualArticle[] {
  return articles.filter((article) => article.locale === locale);
}

export function getArticle(locale: Locale, slug: string): ManualArticle {
  const article = articles.find(
    (candidate) => candidate.locale === locale && candidate.slug === slug,
  );
  if (!article) {
    throw new Error(`Manual article not found: ${locale}/${slug}`);
  }
  return article;
}

export function getTranslation(locale: Locale, slug: string): ManualArticle {
  const article = getArticle(locale, slug);
  const targetLocale: Locale = locale === 'en' ? 'de' : 'en';
  const translation = articles.find(
    (candidate) =>
      candidate.locale === targetLocale && candidate.translationKey === article.translationKey,
  );
  if (!translation) {
    throw new Error(`Missing ${targetLocale} translation for ${article.translationKey}`);
  }
  return translation;
}

export function manualEntries(): {
  locale: Locale;
  manual: ManualSegment;
  slug: string;
}[] {
  return articles
    .filter((article) => article.section !== 'overview')
    .map((article) => ({
      locale: article.locale,
      manual: article.locale === 'en' ? 'manual' : 'handbuch',
      slug: article.slug,
    }));
}

export function validateArticles(candidateArticles: ManualArticle[]): void {
  const routes = new Set<string>();
  const slugs = new Set<string>();
  const translations = new Map<string, Set<Locale>>();

  for (const article of candidateArticles) {
    if (!isLocale(article.locale)) {
      throw new Error(`Unknown locale for ${article.translationKey}`);
    }
    if (!isSection(article.section)) {
      throw new Error(`Unknown section for ${article.translationKey}`);
    }

    for (const [field, value] of [
      ['translationKey', article.translationKey],
      ['slug', article.slug],
      ['title', article.title],
      ['summary', article.summary],
      ['href', article.href],
    ] as const) {
      if (value.trim() === '') {
        throw new Error(`Manual article has invalid ${field}`);
      }
    }
    if (!Number.isFinite(article.order)) {
      throw new Error(`Manual article ${article.translationKey} has invalid order`);
    }
    if (!slugPattern.test(article.slug)) {
      throw new Error(`Manual article ${article.translationKey} has invalid slug`);
    }
    if (article.navTitle !== undefined && article.navTitle.trim() === '') {
      throw new Error(`Manual article ${article.translationKey} has invalid navTitle`);
    }
    if (article.search !== undefined && typeof article.search !== 'boolean') {
      throw new Error(`Manual article ${article.translationKey} has invalid search`);
    }
    if (!isComponent(article.component)) {
      throw new Error(`Manual article ${article.translationKey} has invalid component`);
    }

    const route = `${article.locale}:${article.href}`;
    if (routes.has(route)) {
      throw new Error(`Duplicate route: ${article.href}`);
    }
    routes.add(route);

    const slug = `${article.locale}:${article.slug}`;
    if (slugs.has(slug)) {
      throw new Error(`Duplicate slug: ${article.locale}/${article.slug}`);
    }
    slugs.add(slug);

    const translationLocales = translations.get(article.translationKey) ?? new Set<Locale>();
    if (translationLocales.has(article.locale)) {
      throw new Error(
        `Duplicate locale ${article.locale} for translation ${article.translationKey}`,
      );
    }
    translationLocales.add(article.locale);
    translations.set(article.translationKey, translationLocales);
  }

  for (const [translationKey, translationLocales] of translations) {
    for (const locale of locales) {
      if (!translationLocales.has(locale)) {
        throw new Error(`Missing ${locale} translation for ${translationKey}`);
      }
    }
  }
}
