/** Display shape for a finished extraction rendered in the chat thread. */
export interface ExtractionCardData {
  status: 'running' | 'done' | 'empty' | 'cancelled' | 'error';
  title: string;
  detail: string;
  entitiesFound: number;
  relationsFound: number;
}

/**
 * Parse an `extraction`-role chat message body (persisted by the backend) into
 * card data, or `null` when the content is not a valid extraction summary.
 *
 * The backend stores extraction results as JSON so the result card survives
 * navigating away from and back to the chat, where it previously lived only in
 * transient component state.
 */
export function parseExtractionMessage(content: string): ExtractionCardData | null {
  let raw: unknown;
  try {
    raw = JSON.parse(content);
  } catch {
    return null;
  }
  if (typeof raw !== 'object' || raw === null) return null;
  const o = raw as Record<string, unknown>;
  if (
    typeof o.status !== 'string' ||
    typeof o.title !== 'string' ||
    typeof o.detail !== 'string' ||
    typeof o.entitiesFound !== 'number' ||
    typeof o.relationsFound !== 'number'
  ) {
    return null;
  }
  return {
    status: o.status as ExtractionCardData['status'],
    title: o.title,
    detail: o.detail,
    entitiesFound: o.entitiesFound,
    relationsFound: o.relationsFound,
  };
}
