import { describe, it, expect } from 'vitest';
import { parseExtractionMessage } from './extraction-message';

describe('parseExtractionMessage', () => {
  it('parses a persisted done summary', () => {
    const card = parseExtractionMessage(
      JSON.stringify({
        status: 'done',
        title: 'Extraction complete',
        detail: 'Created 5 entities, 3 relations',
        entitiesFound: 5,
        relationsFound: 3,
      }),
    );
    expect(card).toEqual({
      status: 'done',
      title: 'Extraction complete',
      detail: 'Created 5 entities, 3 relations',
      entitiesFound: 5,
      relationsFound: 3,
    });
  });

  it('parses a persisted empty summary', () => {
    const card = parseExtractionMessage(
      JSON.stringify({
        status: 'empty',
        title: 'Nothing found',
        detail: 'No passages found for "Varn"',
        entitiesFound: 0,
        relationsFound: 0,
      }),
    );
    expect(card?.status).toBe('empty');
    expect(card?.entitiesFound).toBe(0);
  });

  it('returns null for malformed JSON', () => {
    expect(parseExtractionMessage('not json')).toBeNull();
  });

  it('returns null when required fields are missing', () => {
    expect(parseExtractionMessage(JSON.stringify({ title: 'x' }))).toBeNull();
  });
});
