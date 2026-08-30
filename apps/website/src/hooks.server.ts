import { routeLocale } from '$lib/i18n/locale';
import type { Handle } from '@sveltejs/kit';

export const handle: Handle = async ({ event, resolve }) =>
  resolve(event, {
    transformPageChunk: ({ html }) => html.replace('%lang%', routeLocale(event.url.pathname)),
  });
