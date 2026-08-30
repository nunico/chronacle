import type { Locale } from '$lib/i18n/types';
import type { ParamMatcher } from '@sveltejs/kit';

export const match = ((param: string): param is Locale =>
  param === 'en' || param === 'de') satisfies ParamMatcher;
