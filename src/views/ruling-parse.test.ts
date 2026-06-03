import { describe, it, expect } from 'vitest';
import { escapeAttr, splitHeading, renderContent, parseRuling } from './ruling-parse';

describe('escapeAttr', () => {
  it('escapes &, ", <, >', () => {
    expect(escapeAttr('a & "b" <c>')).toBe('a &amp; &quot;b&quot; &lt;c&gt;');
  });
});

describe('splitHeading', () => {
  it('splits a leading ALL-CAPS heading off the body', () => {
    const r = splitHeading('CORIOLIS AND KUA The center of the Third Horizon is here.');
    expect(r.heading).toBe('CORIOLIS AND KUA');
    expect(r.body).toBe('The center of the Third Horizon is here.');
  });

  it('returns no heading when the body has no lowercase tail', () => {
    const r = splitHeading('A 6 means success.');
    expect(r.heading).toBeNull();
    expect(r.body).toBe('A 6 means success.');
  });

  it('returns no heading for a single ALL-CAPS word', () => {
    const r = splitHeading('GRAPPLED reduces speed to 0.');
    expect(r.heading).toBeNull();
  });
});

describe('renderContent', () => {
  it('replaces a [Source] marker with a citation badge button', () => {
    const html = renderContent('See [Source: "Codex", p.9] for context.');
    expect(html).toContain('<button');
    expect(html).toContain('class="citation-badge"');
    expect(html).toContain('data-source="Codex"');
    expect(html).toContain('data-page="9"');
    expect(html).toContain('Codex p.9');
  });

  it('stashes an inline quote in data-quote', () => {
    const html = renderContent('[Source: "SRD", p.45, quote: "A grappled creature\'s speed is 0."]');
    // Apostrophe is not escaped by escapeAttr
    expect(html).toContain('data-quote="A grappled creature\'s speed is 0."');
  });

  it('escapes a malicious source name (no raw <script>)', () => {
    const html = renderContent('[Source: "<script>alert(1)</script>", p.1]');
    expect(html).not.toMatch(/<script>/);
    expect(html).toContain('&lt;script&gt;');
  });
});

describe('parseRuling', () => {
  it('splits an assistant message with one citation into verdict + why + cites', () => {
    const text =
      'Yes, but at disadvantage. You can cast a spell while grappled. [Source: "SRD 5.2", p.190, quote: "A grappled creature\'s speed becomes 0."]';
    const r = parseRuling(text);
    expect(r.verdict).toBe('Yes, but at disadvantage');
    expect(r.why).toContain('You can cast a spell while grappled');
    expect(r.why).toContain('class="citation-badge"');
    expect(r.cites).toHaveLength(1);
    expect(r.cites[0].label).toBe('SRD 5.2 p.190');
    expect(r.cites[0].src).toBe('SRD 5.2 · p.190');
    expect(r.cites[0].quote).toBe("A grappled creature's speed becomes 0.");
  });

  it('returns one cite per [Source] marker', () => {
    const text =
      'Half cover gives +2 AC. [Source: "SRD", p.10] [Source: "House Rules", p.2]';
    const r = parseRuling(text);
    expect(r.cites).toHaveLength(2);
    expect(r.cites[0].label).toBe('SRD p.10');
    expect(r.cites[1].label).toBe('House Rules p.2');
  });

  it('handles a message with no citations (cites is empty)', () => {
    const r = parseRuling('Just a plain answer with no source.');
    expect(r.verdict).toBe('Just a plain answer with no source');
    expect(r.cites).toEqual([]);
  });

  it('handles a marker without a page (label omits p.)', () => {
    const r = parseRuling('Foo. [Source: "Lore"]');
    expect(r.cites[0].label).toBe('Lore');
    expect(r.cites[0].src).toBe('Lore');
  });
});
