import type { ManualSegment } from '$lib/i18n/types';
import type { ParamMatcher } from '@sveltejs/kit';

export const match = ((param: string): param is ManualSegment =>
  param === 'manual' || param === 'handbuch') satisfies ParamMatcher;
