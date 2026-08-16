import { getArticle, manualEntries } from '$lib/content/registry';
import { isManualRoutePair, manualBase } from '$lib/i18n/locale';
import type { Locale } from '$lib/i18n/types';
import { error } from '@sveltejs/kit';
import type { EntryGenerator, PageLoad } from './$types';

export const entries: EntryGenerator = () => manualEntries();

export const prerender = 'auto';
export const trailingSlash = 'always';

export const load: PageLoad = ({ params }) => {
  if (!isManualRoutePair(params.locale, params.manual)) {
    error(404, 'Manual not found');
  }

  const slug = Array.isArray(params.slug) ? params.slug.join('/') : params.slug;

  try {
    const article = getArticle(params.locale as Locale, slug);
    if (article.href !== `${manualBase(article.locale)}/${slug}`) {
      error(404, 'Manual article not found');
    }

    return {
      locale: article.locale,
      slug: article.slug,
      title: article.title,
      summary: article.summary,
    };
  } catch (cause) {
    if (cause && typeof cause === 'object' && 'status' in cause && cause.status === 404) {
      throw cause;
    }
    error(404, 'Manual article not found');
  }
};
