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

  it('replaces [Entity] with an entity-badge span', () => {
    const html = renderContent(
      'Nazirdijan acts [Entity: "Nazirdijan", kind: "player_character"].',
    );
    expect(html).toContain('<span class="entity-badge"');
    expect(html).toContain('title="player_character"');
    expect(html).toContain('>Nazirdijan<');
  });

  it('escapes a malicious entity name in [Entity]', () => {
    const html = renderContent('[Entity: "<script>alert(1)</script>", kind: "npc"]');
    expect(html).not.toMatch(/<script>/);
    expect(html).toContain('&lt;script&gt;');
  });

  it('renders both Source and Entity markers in the same string', () => {
    const html = renderContent(
      'Rules apply [Source: "PHB", p.72]. Nazirdijan agrees [Entity: "Nazirdijan", kind: "player_character"].',
    );
    expect(html).toContain('class="citation-badge"');
    expect(html).toContain('class="entity-badge"');
  });

  // The LLM emits Markdown emphasis; without rendering it leaks as literal
  // asterisks in the ruling card (the original field bug).
  it('renders **bold** as <strong>', () => {
    expect(renderContent('names **Mandragor Ho** here')).toContain(
      '<strong>Mandragor Ho</strong>',
    );
  });

  it('renders *italic* as <em>', () => {
    expect(renderContent('working *for* the guild')).toContain('<em>for</em>');
  });

  it('does not leave a stray <em> when rendering bold', () => {
    expect(renderContent('**Mandragor Ho**')).not.toContain('<em>');
  });

  it('renders `inline code` as <code>', () => {
    expect(renderContent('roll `1d6` now')).toContain('<code>1d6</code>');
  });

  it('does not treat a lone asterisk as emphasis', () => {
    const html = renderContent('a * b for 5 * 5');
    expect(html).not.toContain('<em>');
  });

  it('HTML-escapes body text so raw markup cannot render', () => {
    const html = renderContent('beware <script>alert(1)</script>');
    expect(html).not.toContain('<script>');
    expect(html).toContain('&lt;script&gt;');
  });

  it('renders emphasis alongside a citation marker', () => {
    const html = renderContent('**Yes**. [Source: "X", p.1]');
    expect(html).toContain('<strong>Yes</strong>');
    expect(html).toContain('class="citation-badge"');
  });

  // Field regression: the LLM emitted plural `quotes:` with two excerpts joined
  // by "and". The strict `quote:` + `]` anchor failed to match, so the raw
  // marker leaked into the rendered answer instead of becoming a badge.
  it('renders a marker that uses plural "quotes:" with multiple excerpts', () => {
    const html = renderContent(
      '[Source: "Coriolis EN.pdf", p.214-215, quotes: "Secure dangerous artifacts for... the Draconites" and "Prevent the spread of dangerous bionics for... the Draconites"]',
    );
    expect(html).toContain('class="citation-badge"');
    expect(html).toContain('data-source="Coriolis EN.pdf"');
    expect(html).toContain('data-page="214"');
    expect(html).not.toContain('[Source:');
    // The first excerpt becomes the supporting quote shown in the popover.
    expect(html).toContain('data-quote="Secure dangerous artifacts for... the Draconites"');
  });

  it('tolerates trailing junk before the closing bracket of a marker', () => {
    const html = renderContent('See [Source: "X", p.3, note: whatever extra here].');
    expect(html).toContain('class="citation-badge"');
    expect(html).not.toContain('[Source:');
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

  // Regression: when the LLM's first sentence ends with a citation and the
  // filename contains a period (e.g. "Rulebook.pdf"), parseRuling used to split
  // verdict/body at the period INSIDE the filename, cutting the [Source: ...]
  // marker in half so neither verdict nor body could match SOURCE_RE — the
  // raw citation markup leaked into the rendered output. Mirrors the actual
  // shape observed in the field (a single citation-laden sentence ending in `.`).
  it('does not split a sentence inside a citation marker (filename contains a period)', () => {
    const text =
      'The lands of Vethara include Drystone [Source: "Rulebook.pdf", p.298, quote: "Drystone is a rising power."], Marrowen [Source: "Rulebook.pdf", p.36, quote: "Marrowen lies to the east."].';
    const r = parseRuling(text);
    // No raw citation markup must leak into either rendered half.
    expect(r.verdict).not.toContain('[Source:');
    expect(r.why).not.toContain('[Source:');
    // Verdict carries the citation-stripped sentence so the title makes sense.
    expect(r.verdict).toContain('Drystone');
    expect(r.verdict).toContain('Marrowen');
    expect(r.verdict).not.toContain('Rulebook.pdf');
    // Both citations are captured (chip footer + popover).
    expect(r.cites).toHaveLength(2);
    expect(r.cites[0].label).toBe('Rulebook.pdf p.298');
    expect(r.cites[1].label).toBe('Rulebook.pdf p.36');
  });

  // Defensive: same problem could trigger on a literal newline inside a
  // citation (if the LLM emitted one), since the verdict/body split regex
  // also matched \n.
  it('does not split inside a citation marker that spans a newline', () => {
    const text = 'Hello Drystone [Source: "Rulebook.pdf",\np.298, quote: "Drystone."] and more.';
    const r = parseRuling(text);
    expect(r.verdict).not.toContain('[Source:');
    expect(r.cites).toHaveLength(1);
    expect(r.cites[0].label).toBe('Rulebook.pdf p.298');
  });

  // Regression: same period-inside-marker bug as [Source:], but for [Entity:].
  // If the entity name contains a period (e.g. "Dr. Aldric"), findVerdictBoundary
  // used to treat that period as the sentence boundary, splitting the marker
  // across verdict and whyText so the entity badge never rendered correctly.
  it('renders bold emphasis in the verdict', () => {
    const r = parseRuling('**Mandragor Ho** is the only member. [Source: "C", p.205]');
    expect(r.verdict).toContain('<strong>Mandragor Ho</strong>');
  });

  it('captures a plural-"quotes:" marker instead of leaking it as raw text', () => {
    const text =
      'The reference names one member. No other individual is identified. [Source: "Coriolis EN.pdf", p.214-215, quotes: "Secure dangerous artifacts for... the Draconites" and "Prevent the spread of dangerous bionics for... the Draconites"]';
    const r = parseRuling(text);
    expect(r.why).not.toContain('[Source:');
    expect(r.cites).toHaveLength(1);
    expect(r.cites[0].label).toBe('Coriolis EN.pdf p.214');
    expect(r.cites[0].quote).toBe('Secure dangerous artifacts for... the Draconites');
  });

  it('renders italic emphasis in the why body', () => {
    const r = parseRuling(
      'One member named. They work *for* the guild. [Source: "C", p.205]',
    );
    expect(r.why).toContain('<em>for</em>');
  });

  it('entity name with period does not split the marker at the period', () => {
    const text =
      'Aldric is present [Entity: "Dr. Aldric", kind: "npc"]. More detail here [Entity: "Dr. Aldric", kind: "npc"].';
    const ruling = parseRuling(text);
    // The entity marker must not bleed into verdict as raw markup.
    expect(ruling.verdict).not.toContain('[Entity:');
    // The why (rendered HTML) must contain the entity badge derived from the full marker.
    expect(ruling.why).toContain('Dr. Aldric');
    expect(ruling.why).toContain('class="entity-badge"');
  });
});
