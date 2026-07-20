import type { SupportedLocale } from './types';

const languageMarkers: Record<SupportedLocale, RegExp> = {
  en: /\b(?:the|and|what|how|does|can|is|are|with|rule|rules|cover|work|please)\b/giu,
  de: /\b(?:der|die|das|und|wie|was|ist|sind|funktioniert|regel|regeln|mit|für|kann)\b/giu,
  fr: /\b(?:le|la|les|de|des|et|est|quelle|quel|comment|règle|règles|avec|pour|peut)\b/giu,
  es: /\b(?:el|la|los|las|de|del|y|qué|como|cómo|es|son|regla|reglas|con|para|puede)\b/giu,
};

const distinctiveCharacters: Record<SupportedLocale, RegExp> = {
  en: /$^/u,
  de: /[äöüß]/giu,
  fr: /[àâæçéèêëîïôœùûüÿ]/giu,
  es: /[áéíóúüñ¿¡]/giu,
};

/**
 * Detect one of the languages Chronacle can guarantee a response for.
 *
 * This deliberately small, offline heuristic only returns a language when a
 * sentence contains enough language-specific evidence. Ambiguous short input
 * falls back to the configured UI locale rather than guessing.
 */
export function detectSupportedLanguage(message: string): SupportedLocale | null {
  const normalized = message.trim().toLocaleLowerCase();
  const wordCount = normalized.match(/[\p{L}]+/gu)?.length ?? 0;
  if (wordCount < 3) return null;

  const scores = supportedScores(normalized);
  const ranked = (Object.entries(scores) as Array<[SupportedLocale, number]>).sort(
    ([, left], [, right]) => right - left,
  );
  const [language, score] = ranked[0];
  const runnerUp = ranked[1][1];

  return score >= 2 && score > runnerUp ? language : null;
}

/** Return the detected message language, or the caller's locale fallback. */
export function resolveResponseLanguage(
  message: string,
  fallback: SupportedLocale,
): SupportedLocale {
  return detectSupportedLanguage(message) ?? fallback;
}

function supportedScores(message: string): Record<SupportedLocale, number> {
  return (Object.keys(languageMarkers) as SupportedLocale[]).reduce(
    (scores, language) => {
      const markers = message.match(languageMarkers[language])?.length ?? 0;
      const distinctive = message.match(distinctiveCharacters[language])?.length ?? 0;
      scores[language] = markers + Math.min(distinctive, 2);
      return scores;
    },
    { en: 0, de: 0, fr: 0, es: 0 } satisfies Record<SupportedLocale, number>,
  );
}
