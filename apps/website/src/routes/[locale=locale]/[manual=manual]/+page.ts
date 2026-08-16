import { articlesFor } from '$lib/content/registry';
import { isManualRoutePair } from '$lib/i18n/locale';
import { error } from '@sveltejs/kit';
import type { EntryGenerator, PageLoad } from './$types';

export const entries: EntryGenerator = () => [
  { locale: 'en', manual: 'manual' },
  { locale: 'de', manual: 'handbuch' },
];

export const prerender = true;
export const trailingSlash = 'always';

export const load: PageLoad = ({ params }) => {
  if (!isManualRoutePair(params.locale, params.manual)) {
    error(404, 'Manual not found');
  }

  const article = articlesFor(params.locale).find((candidate) => candidate.section === 'overview');
  if (!article) {
    error(404, 'Manual overview not found');
  }

  return {
    locale: article.locale,
    slug: article.slug,
    title: article.title,
    summary: article.summary,
  };
};
