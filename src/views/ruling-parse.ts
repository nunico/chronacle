export interface Cite {
  label: string;
  src: string;
  quote: string;
}

export interface RulingData {
  verdict: string;
  why: string; // HTML — contains citation-badge buttons
  cites: Cite[];
}

/** HTML-attribute-escape a string. */
export function escapeAttr(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/"/g, '&quot;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

/** Split a leading ALL-CAPS section heading off the quote, if any.
 *
 * pdfium concatenates section headings onto the same line as body text
 * ("CORIOLIS AND KUA The center of the Third Horizon..."), and when the
 * LLM picks a verbatim sentence it grabs the heading too. We split at
 * the first word containing a lowercase letter.
 *
 * Conservative: requires 2+ leading ALL-CAPS words AND non-empty body
 * to avoid misreading "A 6 means success." or stray emphasis as a heading. */
export function splitHeading(quote: string): { heading: string | null; body: string } {
  const tokens = quote.split(/(\s+)/);
  let headingTokenEnd = 0;
  let headingWordCount = 0;
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (/^\s+$/.test(t)) continue;
    if (/^[A-Z][A-Z0-9'&:\-/]*$/.test(t)) {
      headingTokenEnd = i + 1;
      headingWordCount++;
    } else {
      break;
    }
  }
  if (headingWordCount < 2 || headingTokenEnd >= tokens.length) {
    return { heading: null, body: quote };
  }
  const heading = tokens.slice(0, headingTokenEnd).join('').trim();
  const body = tokens.slice(headingTokenEnd).join('').trim();
  if (!body) return { heading: null, body: quote };
  return { heading, body };
}

const SOURCE_RE =
  /\[Source:\s*"([^"]+)"(?:,\s*p\.\s*(\d+)(?:-\d+)?)?(?:,\s*quote:\s*"([\s\S]*?)")?\s*\]/g;

/** Render message content with clickable citation badges (HTML string). */
export function renderContent(text: string): string {
  return text.replace(SOURCE_RE, (_, name: string, page: string | undefined, quote: string | undefined) => {
    const dataPage = page ? ` data-page="${escapeAttr(page)}"` : '';
    const dataQuote = quote ? ` data-quote="${escapeAttr(quote)}"` : '';
    const label = `${escapeAttr(name)}${page ? ` p.${escapeAttr(page)}` : ''}`;
    return `<button type="button" class="citation-badge" data-source="${escapeAttr(name)}"${dataPage}${dataQuote} title="Show source passage">${label}</button>`;
  });
}

/** Parse an assistant message into a ruling structure for RulingCard. */
export function parseRuling(text: string): RulingData {
  const cites: Cite[] = [];
  // Reset regex state for repeated use.
  SOURCE_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = SOURCE_RE.exec(text)) !== null) {
    const name = m[1];
    const page = m[2];
    const quote = m[3] ?? '';
    const label = `${name}${page ? ` p.${page}` : ''}`;
    const src = `${name}${page ? ` · p.${page}` : ''}`;
    cites.push({ label, src, quote });
  }

  // Split verdict from why on the first sentence boundary (before stripping citations).
  const sentenceEnd = text.search(/[.!?\n]/);
  let verdict: string;
  let whyText: string;
  if (sentenceEnd === -1) {
    verdict = text;
    whyText = '';
  } else {
    verdict = text.slice(0, sentenceEnd).trim();
    whyText = text.slice(sentenceEnd + 1).trim();
  }

  // Strip the verdict of any source markers, then render why with badges.
  verdict = verdict.replace(SOURCE_RE, '').trim();
  const why = renderContent(whyText);

  return { verdict, why, cites };
}
