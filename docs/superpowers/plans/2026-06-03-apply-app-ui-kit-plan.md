# Apply the Chronacle App UI Kit — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current three-page frontend (`ChatPage`/`CampaignsPage`/`SettingsPage`) with a single shell-plus-views structure styled to the "Arcane Terminal" design system, wired against the existing `commands.ts` API.

**Architecture:** Hybrid — copy the design kit's pure-presentation pieces (`EyeMark`, `RulingCard`, tokens, brand assets), rewrite its wired containers as TS-typed Svelte components. New layout: left `CampaignRail` (brand + campaign-switcher popover + nav + footer actions) plus `main` (`Topbar` + active view). Views: `OracleView` (chat with ruling cards), `CampaignView` (active-campaign hero + collection management with per-collection upload), `NotesView` (Phase 2 placeholder), `SettingsView` (restyled).

**Tech Stack:** Svelte 5 (runes), TypeScript, Vite, Vitest + `@testing-library/svelte`, `lucide` (ESM, tree-shaken), `@fontsource-variable/{cinzel,spectral,hanken-grotesk,jetbrains-mono}`, Tauri 2 (unchanged).

**Spec:** `docs/superpowers/specs/2026-06-03-apply-app-ui-kit-design.md`

---

## Order & build-state invariant

Tasks are ordered so that **the app builds and the existing UI still works after every commit until Task 20** (the App-shell swap). The new `tokens.css` is imported alongside the old palette; new components are added in parallel files; nothing is rewired until the final integration task.

Files mentioned in this plan map onto the spec's Section 4 file layout. The kit's source files live at `.claude/skills/chronacle-design/ui_kits/app/`.

---

## Task 1: Install dependencies

**Files:**
- Modify: `package.json` (devDependencies / dependencies)
- Modify: `pnpm-lock.yaml`

- [ ] **Step 1: Install Lucide and the four variable fonts**

```bash
cd /Users/admin/Code/github.com/nunico/chronacle
pnpm add lucide @fontsource-variable/cinzel @fontsource-variable/spectral @fontsource-variable/hanken-grotesk @fontsource-variable/jetbrains-mono
```

- [ ] **Step 2: Verify install**

Run: `pnpm typecheck`
Expected: PASS (no new TS errors — the packages exist on disk but aren't yet imported).

Run: `node -e "console.log(require.resolve('lucide'))"`
Expected: prints a path inside `node_modules/lucide/`.

- [ ] **Step 3: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore: add lucide and @fontsource-variable packages for UI kit"
```

---

## Task 2: Copy brand assets

**Files:**
- Create: `src/lib/assets/chronacle-icon.png`
- Create: `src/lib/assets/tex-starfield.png`
- Create: `src/lib/assets/tex-circuit.png`
- Create: `src/lib/assets/tex-aura.png`

- [ ] **Step 1: Copy the four asset files from the design system**

```bash
cd /Users/admin/Code/github.com/nunico/chronacle
mkdir -p src/lib/assets
cp .claude/skills/chronacle-design/assets/chronacle-icon.png src/lib/assets/
cp .claude/skills/chronacle-design/assets/tex-starfield.png src/lib/assets/
cp .claude/skills/chronacle-design/assets/tex-circuit.png src/lib/assets/
cp .claude/skills/chronacle-design/assets/tex-aura.png src/lib/assets/
```

- [ ] **Step 2: Verify the four files exist**

Run: `ls -la src/lib/assets/`
Expected: 4 PNG files, each > 0 bytes.

- [ ] **Step 3: Commit**

```bash
git add src/lib/assets/
git commit -m "chore: copy Chronacle brand assets into src/lib/assets"
```

---

## Task 3: Add design-system tokens

**Files:**
- Create: `src/lib/tokens.css`

- [ ] **Step 1: Copy `colors_and_type.css` from the design system, dropping its Google Fonts @import line**

```bash
cd /Users/admin/Code/github.com/nunico/chronacle
# Copy verbatim, then strip the @import url(...) line at the top
sed '/^@import url(.https:..fonts.googleapis.com/d' \
  .claude/skills/chronacle-design/colors_and_type.css \
  > src/lib/tokens.css
```

- [ ] **Step 2: Verify the file has tokens but no Google Fonts import**

Run: `head -8 src/lib/tokens.css`
Expected: comment block + `:root {` (the `@import url('https://fonts.googleapis.com/...')` line is gone).

Run: `grep -c 'fonts.googleapis.com' src/lib/tokens.css`
Expected: `0`.

Run: `grep -c -- '--bg-void' src/lib/tokens.css`
Expected: `1`.

- [ ] **Step 3: Commit**

```bash
git add src/lib/tokens.css
git commit -m "chore: add design-system tokens (colors + type) as src/lib/tokens.css"
```

---

## Task 4: Wire fonts and tokens into `main.ts`

**Files:**
- Modify: `src/main.ts`

- [ ] **Step 1: Read current `main.ts`**

Run: `cat src/main.ts`
Expected: a small file mounting `App` to `#app`.

- [ ] **Step 2: Add font imports + tokens import at the top of `main.ts`**

Add these lines to the very top of `src/main.ts`, above the existing imports:

```ts
import '@fontsource-variable/cinzel';
import '@fontsource-variable/spectral';
import '@fontsource-variable/hanken-grotesk';
import '@fontsource-variable/jetbrains-mono';
import './lib/tokens.css';
```

- [ ] **Step 3: Verify the dev server still builds**

Run: `pnpm typecheck`
Expected: PASS.

Run: `pnpm build`
Expected: success; output mentions the four font files in the bundle.

- [ ] **Step 4: Commit**

```bash
git add src/main.ts
git commit -m "feat: self-host design-system fonts via @fontsource-variable and load tokens.css"
```

---

## Task 5: Add `EyeMark` presentation component

**Files:**
- Create: `src/components/EyeMark.svelte`

- [ ] **Step 1: Create the directory and component file**

```bash
mkdir -p src/components
```

Create `src/components/EyeMark.svelte` with this content (typed copy of the kit's `EyeMark.svelte`):

```svelte
<script lang="ts">
  let { size = 34, glow = true }: { size?: number; glow?: boolean } = $props();
  const uid = 'e' + Math.random().toString(36).slice(2, 9);
  let h = $derived(Math.round(size * 0.72));
</script>

<svg
  width={size}
  height={h}
  viewBox="0 0 84 60"
  style="display:block;filter:{glow ? 'drop-shadow(0 0 10px rgba(123,92,255,0.55))' : 'none'}"
>
  <defs>
    <radialGradient id={'ir' + uid} cx="50%" cy="46%" r="60%">
      <stop offset="0%" stop-color="#EAF0FF" />
      <stop offset="34%" stop-color="#C8D6FF" />
      <stop offset="62%" stop-color="#7B5CFF" />
      <stop offset="100%" stop-color="#2A3FE0" />
    </radialGradient>
    <linearGradient id={'ld' + uid} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#8AA0FF" />
      <stop offset="100%" stop-color="#5B78FF" />
    </linearGradient>
  </defs>
  <path
    d="M4 30 C 22 6, 62 6, 80 30 C 62 54, 22 54, 4 30 Z"
    fill="#070912"
    stroke={'url(#ld' + uid + ')'}
    stroke-width="2.4"
  />
  <circle cx="42" cy="30" r="16" fill={'url(#ir' + uid + ')'} />
  <circle cx="42" cy="30" r="6.5" fill="#05060F" />
  <circle cx="47" cy="24" r="2.6" fill="#EAF0FF" />
</svg>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/components/EyeMark.svelte
git commit -m "feat: add EyeMark scrying-eye avatar component"
```

---

## Task 6: Add `Icon` component (Lucide ESM)

**Files:**
- Create: `src/components/Icon.svelte`

- [ ] **Step 1: Create the component**

Create `src/components/Icon.svelte`:

```svelte
<script lang="ts">
  import { icons, createElement, type IconNode } from 'lucide';

  let {
    name,
    size = 18,
    strokeWidth = 1.75,
    color = '',
    className = '',
  }: {
    name: string;
    size?: number;
    strokeWidth?: number;
    color?: string;
    className?: string;
  } = $props();

  let el = $state<HTMLSpanElement | undefined>(undefined);

  // Lucide names are kebab-case ("book-open"); the icons map is keyed PascalCase ("BookOpen").
  function toPascal(kebab: string): string {
    return kebab
      .split('-')
      .map((p) => (p ? p[0].toUpperCase() + p.slice(1) : ''))
      .join('');
  }

  $effect(() => {
    if (!el) return;
    const node = (icons as Record<string, IconNode>)[toPascal(name)];
    el.innerHTML = '';
    if (!node) return;
    const svg = createElement(node);
    svg.setAttribute('width', String(size));
    svg.setAttribute('height', String(size));
    svg.setAttribute('stroke-width', String(strokeWidth));
    el.appendChild(svg);
  });
</script>

<span
  bind:this={el}
  class={className}
  style="display:inline-flex;align-items:center;justify-content:center;width:{size}px;height:{size}px;line-height:0;{color
    ? `color:${color};`
    : ''}"
></span>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/components/Icon.svelte
git commit -m "feat: add Icon component wrapping lucide ESM exports"
```

---

## Task 7: Extract pure ruling-parse logic

**Files:**
- Create: `src/views/ruling-parse.ts`
- Create: `src/views/ruling-parse.test.ts`

This task pulls the citation-rendering helpers out of `ChatPage.svelte` so they can be unit-tested independently of the Svelte renderer. The functions stay byte-for-byte identical to the current `ChatPage` implementation; we just relocate them.

- [ ] **Step 1: Create the test file first (TDD)**

```bash
mkdir -p src/views
```

Create `src/views/ruling-parse.test.ts`:

```ts
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
    expect(html).toContain('data-quote="A grappled creature&#x27;s speed is 0."'.replace('#x27;', 'apos;')) // tolerate either escaping
      ? null
      : expect(html).toMatch(/data-quote="A grappled creature.*speed is 0\."/);
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
```

- [ ] **Step 2: Run the test — should fail with module not found**

Run: `pnpm test --run src/views/ruling-parse.test.ts`
Expected: FAIL with `Cannot find module './ruling-parse'`.

- [ ] **Step 3: Create the implementation**

Create `src/views/ruling-parse.ts`:

```ts
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

  // Strip source markers, then split verdict from why on the first sentence boundary.
  const stripped = text.replace(SOURCE_RE, '').trim();
  const sentenceEnd = stripped.search(/[.!?\n]/);
  let verdict: string;
  let whyText: string;
  if (sentenceEnd === -1) {
    verdict = stripped;
    whyText = '';
  } else {
    verdict = stripped.slice(0, sentenceEnd).trim();
    whyText = stripped.slice(sentenceEnd + 1).trim();
  }

  // Re-apply renderContent to whyText so embedded markers (if any) become badges.
  // (We already stripped markers from stripped, but the caller may pass partial text.)
  const why = renderContent(whyText);

  return { verdict, why, cites };
}
```

- [ ] **Step 4: Run tests — should pass**

Run: `pnpm test --run src/views/ruling-parse.test.ts`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/views/ruling-parse.ts src/views/ruling-parse.test.ts
git commit -m "feat: extract ruling-parse helpers (escapeAttr, renderContent, parseRuling)"
```

---

## Task 8: Add `RulingCard` presentation component

**Files:**
- Create: `src/components/RulingCard.svelte`

- [ ] **Step 1: Create the component**

Create `src/components/RulingCard.svelte`:

```svelte
<script lang="ts">
  import Icon from './Icon.svelte';
  import EyeMark from './EyeMark.svelte';
  import type { RulingData } from '../views/ruling-parse';

  let { data, defaultOpen = false }: { data: RulingData; defaultOpen?: boolean } = $props();
  let open = $state(defaultOpen ? 0 : -1);
</script>

<div class="msg">
  <div class="who-av eye-badge"><EyeMark size={28} /></div>
  <div class="ruling">
    <div class="who">
      <span>Chronacle</span>
      <span class="tag">· ruling</span>
    </div>
    {#if data.verdict}
      <p class="verdict">{data.verdict}</p>
    {/if}
    {#if data.why}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <p class="why">{@html data.why}</p>
    {/if}
    {#if data.cites.length > 0}
      <div class="cite-row">
        {#each data.cites as c, i (c.label + i)}
          <button class="cite" onclick={() => (open = open === i ? -1 : i)}>
            <Icon name="quote" size={14} />
            {c.label}
            <Icon name={open === i ? 'chevron-up' : 'chevron-down'} size={13} />
          </button>
        {/each}
      </div>
      {#if open >= 0 && data.cites[open]}
        <div class="passage">
          <div class="src">{data.cites[open].src}</div>
          <div class="quote">{data.cites[open].quote || 'No supporting quote available.'}</div>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .msg {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    margin: 18px 0;
  }
  .who-av {
    flex: none;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .ruling {
    flex: 1;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 14px 16px;
    box-shadow: var(--shadow-card);
    min-width: 0;
  }
  .who {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--fg-2);
    margin-bottom: 6px;
  }
  .tag {
    color: var(--fg-3);
    font-weight: 500;
  }
  .verdict {
    font-family: var(--font-serif);
    font-size: 18px;
    line-height: 1.45;
    color: var(--fg-1);
    margin: 0 0 8px;
  }
  .why {
    font-family: var(--font-serif);
    font-size: 16px;
    line-height: 1.65;
    color: var(--fg-2);
    margin: 0 0 12px;
  }
  .cite-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .cite {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: var(--r-full);
    border: 1px solid var(--line);
    color: var(--arcane-300);
    font-family: var(--font-mono);
    font-size: 12px;
    background: rgba(91, 120, 255, 0.06);
  }
  .cite:hover {
    border-color: var(--line-strong);
    color: var(--gem);
  }
  .passage {
    margin-top: 10px;
    padding: 12px 14px;
    background: var(--bg-inset);
    border: 1px solid var(--line-faint);
    border-radius: var(--r-md);
  }
  .src {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-3);
    margin-bottom: 6px;
    letter-spacing: 0.02em;
  }
  .quote {
    font-family: var(--font-serif);
    font-style: italic;
    color: var(--fg-2);
    font-size: 14.5px;
    line-height: 1.55;
  }
</style>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/components/RulingCard.svelte
git commit -m "feat: add RulingCard component for verdict/why/citations rendering"
```

---

## Task 9: Add `NOTE_CATEGORIES` shared config

**Files:**
- Create: `src/shell/note-categories.ts`

- [ ] **Step 1: Create directory and config file**

```bash
mkdir -p src/shell
```

Create `src/shell/note-categories.ts`:

```ts
export type NoteCategoryId =
  | 'sessions'
  | 'player_characters'
  | 'npcs'
  | 'locations'
  | 'factions'
  | 'creatures'
  | 'items'
  | 'events'
  | 'misc';

export interface NoteCategory {
  id: NoteCategoryId;
  label: string;
  icon: string; // Lucide kebab-case name
  group: 'Notebook' | 'Entities';
  folder: string;
  sub: string;
}

export const NOTE_CATEGORIES: NoteCategory[] = [
  {
    id: 'sessions',
    label: 'Sessions',
    icon: 'history',
    group: 'Notebook',
    folder: 'sessions',
    sub: 'Your campaign timeline — recaps, rewards, and open threads.',
  },
  {
    id: 'player_characters',
    label: 'Player Characters',
    icon: 'users-round',
    group: 'Entities',
    folder: 'entities/player_characters',
    sub: 'The party — sheets, hooks, and where each one stands.',
  },
  {
    id: 'npcs',
    label: 'NPCs',
    icon: 'drama',
    group: 'Entities',
    folder: 'entities/npcs',
    sub: "Everyone the party has met, and a few they haven't yet.",
  },
  {
    id: 'locations',
    label: 'Locations',
    icon: 'map-pin',
    group: 'Entities',
    folder: 'entities/locations',
    sub: "Places your party has been — and the ones they're avoiding.",
  },
  {
    id: 'factions',
    label: 'Factions',
    icon: 'flag',
    group: 'Entities',
    folder: 'entities/factions',
    sub: 'The powers moving behind your campaign.',
  },
  {
    id: 'creatures',
    label: 'Creatures',
    icon: 'paw-print',
    group: 'Entities',
    folder: 'entities/creatures',
    sub: 'Beasts and horrors stalking the world.',
  },
  {
    id: 'items',
    label: 'Items',
    icon: 'gem',
    group: 'Entities',
    folder: 'entities/items',
    sub: 'Artifacts, relics, and loot worth noting.',
  },
  {
    id: 'events',
    label: 'Events',
    icon: 'milestone',
    group: 'Entities',
    folder: 'entities/events',
    sub: 'The moments that shaped the campaign.',
  },
  {
    id: 'misc',
    label: 'Misc',
    icon: 'shapes',
    group: 'Entities',
    folder: 'entities/misc',
    sub: 'Everything else worth keeping.',
  },
];

export function findCategory(id: NoteCategoryId): NoteCategory {
  const cat = NOTE_CATEGORIES.find((c) => c.id === id);
  if (!cat) throw new Error(`Unknown note category: ${id}`);
  return cat;
}
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/shell/note-categories.ts
git commit -m "feat: add NOTE_CATEGORIES shared config for rail + NotesView"
```

---

## Task 10: Add `CampaignSwitcher` popover

**Files:**
- Create: `src/shell/CampaignSwitcher.svelte`

- [ ] **Step 1: Create the component**

Create `src/shell/CampaignSwitcher.svelte`:

```svelte
<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import type { Campaign } from '../lib/commands';

  let {
    campaigns,
    activeCampaignId,
    onSelect,
    onManage,
    onClose,
  }: {
    campaigns: Campaign[];
    activeCampaignId: string | null;
    onSelect: (id: string | null) => void;
    onManage: () => void;
    onClose: () => void;
  } = $props();

  let firstBtn = $state<HTMLButtonElement | undefined>(undefined);

  $effect(() => {
    firstBtn?.focus();
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  function onBackdropClick(e: MouseEvent) {
    // Close only when clicking the backdrop itself, not the popover content.
    if (e.target === e.currentTarget) onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="backdrop"
  role="presentation"
  onclick={onBackdropClick}
  onkeydown={() => {}}
></div>

<div class="popover" role="dialog" aria-label="Switch campaign">
  <button
    bind:this={firstBtn}
    class="row"
    class:active={activeCampaignId === null}
    onclick={() => {
      onSelect(null);
      onClose();
    }}
  >
    <span class="gem-dot"></span>
    <span class="nm">Global</span>
    <span class="mt">no campaign</span>
    {#if activeCampaignId === null}
      <Icon name="check" size={14} />
    {/if}
  </button>
  {#each campaigns as c (c.id)}
    <button
      class="row"
      class:active={activeCampaignId === c.id}
      onclick={() => {
        onSelect(c.id);
        onClose();
      }}
    >
      <span class="gem-dot"></span>
      <span class="nm">{c.name}</span>
      <span class="mt">{c.system ?? '—'}</span>
      {#if activeCampaignId === c.id}
        <Icon name="check" size={14} />
      {/if}
    </button>
  {/each}
  <div class="sep"></div>
  <button
    class="row manage"
    onclick={() => {
      onManage();
      onClose();
    }}
  >
    <Icon name="settings" size={14} />
    <span class="nm">Manage campaigns…</span>
  </button>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
  }
  .popover {
    position: absolute;
    top: 68px;
    left: 12px;
    width: 232px;
    z-index: 100;
    padding: 6px;
    background: rgba(16, 19, 42, 0.8);
    backdrop-filter: blur(14px);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-3);
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 13px;
    text-align: left;
    background: none;
    border: 0;
  }
  .row:hover {
    background: rgba(124, 148, 255, 0.07);
    color: var(--fg-1);
  }
  .row.active {
    color: var(--fg-1);
  }
  .row.active .gem-dot {
    box-shadow: var(--glow-arcane);
  }
  .gem-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--grad-gem);
    flex: none;
  }
  .row .nm {
    flex: 1;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row .mt {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-3);
  }
  .sep {
    height: 1px;
    background: var(--line-faint);
    margin: 6px 4px;
  }
  .manage {
    color: var(--arcane-300);
  }
</style>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/shell/CampaignSwitcher.svelte
git commit -m "feat: add CampaignSwitcher popover anchored to rail's campaign card"
```

---

## Task 11: Add `Topbar` component

**Files:**
- Create: `src/shell/Topbar.svelte`

- [ ] **Step 1: Create the component**

Create `src/shell/Topbar.svelte`:

```svelte
<script lang="ts">
  let { title, sub = '' }: { title: string; sub?: string } = $props();
</script>

<header class="topbar">
  <div>
    <div class="title">{title}</div>
    {#if sub}
      <div class="sub">{sub}</div>
    {/if}
  </div>
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 26px 14px;
    border-bottom: 1px solid var(--line-faint);
  }
  .title {
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 700;
    letter-spacing: 0.01em;
    color: var(--fg-1);
  }
  .sub {
    font-family: var(--font-sans);
    font-size: 12.5px;
    color: var(--fg-3);
    margin-top: 2px;
  }
</style>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/shell/Topbar.svelte
git commit -m "feat: add Topbar component with title + subtitle"
```

---

## Task 12: Add `CampaignRail` (without integration)

**Files:**
- Create: `src/shell/CampaignRail.svelte`

Note: this task builds the rail in isolation. It does not yet need to be reachable from the running app — wiring happens in Task 20.

- [ ] **Step 1: Create the component**

Create `src/shell/CampaignRail.svelte`:

```svelte
<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { NOTE_CATEGORIES } from './note-categories.ts';
  import type { Campaign } from '../lib/commands';
  import type { NoteCategoryId } from './note-categories.ts';

  export type View =
    | 'oracle'
    | 'campaign'
    | 'settings'
    | { kind: 'notebook'; category: NoteCategoryId };

  let {
    view,
    activeCampaign,
    setView,
    onOpenSwitcher,
    onOpenUpload,
  }: {
    view: View;
    activeCampaign: Campaign | null;
    setView: (v: View) => void;
    onOpenSwitcher: () => void;
    onOpenUpload: () => void;
  } = $props();

  function isNotebook(v: View, cat: NoteCategoryId): boolean {
    return typeof v === 'object' && v.kind === 'notebook' && v.category === cat;
  }
</script>

<aside class="rail" aria-label="Campaign rail">
  <div class="rail-head">
    <div class="rail-mark" aria-hidden="true"></div>
    <div class="rail-word">Chron<b>a</b>cle</div>
  </div>

  <button
    class="campaign"
    class:active={view === 'campaign'}
    title="Switch campaign"
    aria-label="Switch campaign"
    onclick={onOpenSwitcher}
  >
    <span class="gem"></span>
    <span class="campaign-text">
      <span class="nm">{activeCampaign?.name ?? 'Global'}</span>
      <span class="mt">{activeCampaign?.system ?? 'no campaign'}</span>
    </span>
    <Icon name="chevrons-up-down" size={15} className="chev" />
  </button>

  <nav class="nav primary">
    <button
      class="nav-item"
      class:active={view === 'oracle'}
      onclick={() => setView('oracle')}
    >
      <Icon name="sparkles" size={18} className="ic" />
      Oracle
    </button>
  </nav>

  <div class="rail-scroll">
    {#each ['Notebook', 'Entities'] as group (group)}
      <div class="rail-section">{group}</div>
      <nav class="nav">
        {#each NOTE_CATEGORIES.filter((c) => c.group === group) as c (c.id)}
          <button
            class="nav-item"
            class:active={isNotebook(view, c.id)}
            onclick={() => setView({ kind: 'notebook', category: c.id })}
          >
            <Icon name={c.icon} size={18} className="ic" />
            {c.label}
            <span class="ct">—</span>
          </button>
        {/each}
      </nav>
    {/each}
  </div>

  <div class="rail-foot">
    <button class="foot-btn" onclick={onOpenUpload} title="Upload a PDF">
      <Icon name="upload" size={16} />
      Upload PDF
    </button>
    <button
      class="foot-btn"
      class:active={view === 'campaign'}
      onclick={() => setView('campaign')}
      title="Manage campaign and source collections"
    >
      <Icon name="library" size={16} />
      Campaign &amp; sources
    </button>
    <button
      class="foot-btn icon-only"
      class:active={view === 'settings'}
      onclick={() => setView('settings')}
      title="Settings"
      aria-label="Settings"
    >
      <Icon name="settings" size={16} />
    </button>
  </div>
</aside>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: linear-gradient(180deg, rgba(16, 19, 42, 0.86), rgba(10, 12, 26, 0.86));
    border-right: 1px solid var(--line);
    backdrop-filter: blur(12px);
    position: relative;
  }
  .rail-head {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 16px 16px 14px;
  }
  .rail-mark {
    width: 36px;
    height: 36px;
    border-radius: 11px;
    background: var(--brand-mark) center/cover;
    box-shadow: 0 0 0 1px var(--line), var(--glow-arcane);
    flex: none;
  }
  .rail-word {
    font-family: var(--font-display);
    font-weight: 800;
    font-size: 19px;
    letter-spacing: 0.04em;
    color: var(--fg-1);
  }
  .rail-word b {
    color: var(--violet-400);
  }
  .campaign {
    margin: 4px 12px 12px;
    padding: 11px 12px;
    border-radius: var(--r-md);
    background: var(--bg-panel);
    border: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 10px;
    width: calc(100% - 24px);
    text-align: left;
  }
  .campaign:hover {
    border-color: var(--line-strong);
  }
  .campaign.active {
    border-color: var(--line-glow);
    box-shadow: var(--glow-arcane);
  }
  .campaign .gem {
    width: 26px;
    height: 26px;
    border-radius: var(--r-full);
    background: var(--grad-gem);
    box-shadow: var(--glow-violet);
    flex: none;
  }
  .campaign-text {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .campaign .nm {
    font-weight: 700;
    font-size: 13.5px;
    color: var(--fg-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .campaign .mt {
    font-size: 11px;
    color: var(--fg-3);
    font-family: var(--font-mono);
  }
  .nav {
    padding: 6px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 9px 11px;
    border-radius: var(--r-sm);
    color: var(--fg-2);
    font-weight: 600;
    font-size: 14px;
    background: none;
    border: 0;
    text-align: left;
  }
  .nav-item:hover {
    background: rgba(124, 148, 255, 0.07);
    color: var(--fg-1);
  }
  .nav-item.active {
    background: rgba(91, 120, 255, 0.14);
    color: var(--fg-1);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .nav-item .ct {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--fg-3);
  }
  .rail-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 8px;
  }
  .rail-section {
    margin-top: 14px;
    padding: 0 18px 8px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .rail-foot {
    margin-top: auto;
    padding: 12px;
    border-top: 1px solid var(--line-faint);
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 6px;
  }
  .foot-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 8px 10px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--bg-panel);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 12.5px;
  }
  .foot-btn:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .foot-btn.active {
    border-color: var(--line-glow);
    color: var(--fg-1);
    box-shadow: var(--glow-arcane);
  }
  .foot-btn.icon-only {
    padding: 8px;
  }
</style>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/shell/CampaignRail.svelte
git commit -m "feat: add CampaignRail (brand + campaign card + nav + foot actions)"
```

---

## Task 13: Add `NotesView` Phase 2 placeholder

**Files:**
- Create: `src/views/NotesView.svelte`

- [ ] **Step 1: Create the component**

Create `src/views/NotesView.svelte`:

```svelte
<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { findCategory, type NoteCategoryId } from '../shell/note-categories.ts';

  let { category }: { category: NoteCategoryId } = $props();
  let cat = $derived(findCategory(category));
</script>

<div class="scroll">
  <div class="notes">
    <div class="notes-head">
      <div>
        <h1>{cat.label}</h1>
        <p class="sub">{cat.sub}</p>
        <div class="notes-path">
          <Icon name="folder" size={12} />
          {cat.folder}/
        </div>
      </div>
    </div>

    <div class="empty">
      <div class="glyph">✦</div>
      <h2>Coming in Phase 2</h2>
      <p>
        Your campaign's <code>{cat.folder}/</code> will live here — searchable notes, file-backed,
        linked to entities Chronacle can answer about.
      </p>
    </div>
  </div>
</div>

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .notes {
    max-width: 820px;
    margin: 0 auto;
    padding: 30px 26px 40px;
  }
  .notes-head {
    margin-bottom: 22px;
  }
  .notes-head h1 {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 28px;
    margin: 0;
    color: var(--fg-1);
  }
  .notes-head .sub {
    font-family: var(--font-serif);
    font-size: 15px;
    color: var(--fg-2);
    margin: 6px 0 8px;
    max-width: 60ch;
  }
  .notes-path {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--fg-3);
  }
  .empty {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 36px 28px;
    box-shadow: var(--shadow-card);
    text-align: center;
  }
  .glyph {
    font-family: var(--font-display);
    font-size: 28px;
    color: var(--arcane-300);
    margin-bottom: 10px;
  }
  .empty h2 {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 20px;
    margin: 0 0 8px;
    color: var(--fg-1);
  }
  .empty p {
    font-family: var(--font-serif);
    font-size: 15px;
    color: var(--fg-2);
    max-width: 56ch;
    margin: 0 auto;
    line-height: 1.55;
  }
  .empty code {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--arcane-300);
  }
</style>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/views/NotesView.svelte
git commit -m "feat: add NotesView Phase 2 placeholder"
```

---

## Task 14: Add `SettingsView` (restyled clone of `SettingsPage`)

**Files:**
- Create: `src/views/SettingsView.svelte`

- [ ] **Step 1: Copy the entire current `src/SettingsPage.svelte` to `src/views/SettingsView.svelte`**

```bash
cp src/SettingsPage.svelte src/views/SettingsView.svelte
```

- [ ] **Step 2: Replace the `<style>` block of `src/views/SettingsView.svelte` with the new tokens**

Open `src/views/SettingsView.svelte`. Replace the entire `<style>` block (everything between `<style>` and `</style>`) with:

```css
.settings-page {
  max-width: 720px;
  margin: 0 auto;
  padding: 28px 26px 40px;
  font-family: var(--font-sans);
}
h2 {
  font-family: var(--font-display);
  font-size: 28px;
  margin: 0 0 22px;
  color: var(--fg-1);
}
h3 {
  font-family: var(--font-sans);
  font-size: 14px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--arcane-300);
  margin: 0 0 12px;
}
.status-banner {
  padding: 10px 14px;
  border-radius: var(--r-md);
  margin-bottom: 16px;
  font-size: 13.5px;
  border: 1px solid var(--line);
}
.status-banner.success {
  background: var(--success-bg);
  color: var(--success);
  border-color: rgba(79, 209, 160, 0.4);
}
.status-banner.error {
  background: var(--danger-bg);
  color: var(--danger);
  border-color: rgba(242, 103, 75, 0.4);
}
.status-section,
.config-section {
  background: var(--bg-panel);
  border: 1px solid var(--line);
  border-radius: var(--r-lg);
  padding: 18px 18px 16px;
  margin-bottom: 16px;
  box-shadow: var(--shadow-card);
}
.status-grid {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 6px 14px;
  font-size: 14px;
  color: var(--fg-2);
}
.status-grid .label {
  color: var(--fg-3);
}
label {
  display: block;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--fg-3);
  margin: 14px 0 6px;
  letter-spacing: 0.02em;
}
select,
input {
  width: 100%;
  padding: 9px 12px;
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  background: var(--bg-inset);
  color: var(--fg-1);
  font-family: var(--font-sans);
  font-size: 14px;
  box-sizing: border-box;
}
select:focus,
input:focus {
  outline: none;
  border-color: var(--line-glow);
  box-shadow: var(--glow-focus);
}
.actions {
  display: flex;
  gap: 8px;
  margin-top: 18px;
}
.actions button {
  flex: 1;
  padding: 10px 14px;
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  background: var(--bg-panel-2);
  color: var(--fg-1);
  font-family: var(--font-sans);
  font-weight: 600;
  font-size: 13.5px;
}
.actions button:hover:not(:disabled) {
  border-color: var(--line-strong);
}
.actions .primary {
  background: var(--grad-arcane);
  border-color: transparent;
  color: var(--fg-on-accent);
  box-shadow: var(--glow-arcane);
}
.actions .primary:hover:not(:disabled) {
  filter: brightness(1.08);
}
.actions button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.hint {
  font-size: 12.5px;
  color: var(--fg-3);
  text-align: center;
  margin: 24px 0 16px;
}
.muted {
  font-size: 13px;
  color: var(--fg-3);
  margin: 0 0 10px;
}
hr {
  border: none;
  border-top: 1px solid var(--line-faint);
  margin: 24px 0;
}
.custom-provider-card {
  background: var(--bg-panel-2);
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  padding: 12px 14px;
  margin-bottom: 12px;
}
.provider-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.type-badge {
  font-size: 11px;
  background: rgba(91, 120, 255, 0.12);
  color: var(--arcane-300);
  padding: 2px 6px;
  border-radius: var(--r-sm);
  font-family: var(--font-mono);
}
.provider-detail {
  font-size: 13px;
  color: var(--fg-2);
  margin-bottom: 6px;
}
.provider-detail .label {
  color: var(--fg-3);
  margin-right: 4px;
}
.provider-detail code {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--arcane-300);
  background: var(--bg-inset);
  padding: 2px 6px;
  border-radius: 4px;
}
.model-list {
  list-style: none;
  padding: 0;
  margin: 4px 0;
}
.model-list li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 0;
  font-size: 13px;
}
.model-id {
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--fg-3);
}
.small-btn {
  background: none;
  border: 1px solid var(--line);
  border-radius: var(--r-sm);
  color: var(--fg-2);
  cursor: pointer;
  font-size: 12px;
  padding: 4px 9px;
  font-family: var(--font-sans);
}
.small-btn:hover {
  border-color: var(--line-strong);
  color: var(--fg-1);
}
.small-btn.danger {
  color: var(--danger);
  border-color: rgba(242, 103, 75, 0.4);
}
.small-btn.danger:hover {
  background: var(--danger-bg);
}
.small-btn.primary {
  background: var(--grad-arcane);
  border-color: transparent;
  color: var(--fg-on-accent);
}
.add-provider-form,
.add-model-form {
  background: var(--bg-inset);
  border: 1px solid var(--line);
  border-radius: var(--r-md);
  padding: 12px 14px;
  margin-bottom: 12px;
}
.add-model-form {
  display: flex;
  gap: 6px;
  align-items: center;
}
.add-model-form input {
  flex: 1;
  padding: 6px 10px;
  font-size: 13px;
}
.form-actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}
.empty-state {
  color: var(--fg-3);
  font-size: 13px;
  text-align: center;
  padding: 12px;
}
.text-muted {
  color: var(--fg-3);
  font-size: 13px;
}
.reindex-progress {
  margin-top: 10px;
  font-size: 13px;
  color: var(--fg-3);
  font-family: var(--font-mono);
}
.reindex-error {
  margin-top: 10px;
  padding: 8px 12px;
  border-radius: var(--r-md);
  background: var(--danger-bg);
  color: var(--danger);
  font-size: 13px;
}
.reindex-success {
  margin-top: 10px;
  padding: 8px 12px;
  border-radius: var(--r-md);
  background: var(--success-bg);
  color: var(--success);
  font-size: 13px;
}
```

- [ ] **Step 3: Replace the emoji indicators in the markup**

In `src/views/SettingsView.svelte`, find and replace:

| Find | Replace with |
|------|--------------|
| `{apiKeyConfigured ? '✅ Configured' : '❌ Not set'}` | `{apiKeyConfigured ? 'Configured' : 'Not set'}` (the icon swap goes in step 4) |

- [ ] **Step 4: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/views/SettingsView.svelte
git commit -m "feat: add SettingsView (restyled clone of SettingsPage)"
```

---

## Task 15: Add `OracleView` (replaces ChatPage)

**Files:**
- Create: `src/views/OracleView.svelte`

- [ ] **Step 1: Create the component**

Create `src/views/OracleView.svelte`:

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    chatSend,
    getChatHistory,
    getChunkForCitation,
    type CitationChunk,
  } from '../lib/commands';
  import Icon from '../components/Icon.svelte';
  import EyeMark from '../components/EyeMark.svelte';
  import RulingCard from '../components/RulingCard.svelte';
  import { renderContent, parseRuling, splitHeading } from './ruling-parse';

  let {
    activeCampaignId,
    onOpenUpload,
  }: {
    activeCampaignId: string | null;
    onOpenUpload: () => void;
  } = $props();

  let messages = $state<Array<{ role: string; content: string }>>([]);
  let input = $state('');
  let isLoading = $state(false);
  let currentResponse = $state('');
  let unlistenListener: UnlistenFn | null = null;
  let scrollEl = $state<HTMLDivElement | undefined>(undefined);

  let citationPopover = $state<
    | {
        source: string;
        page: number | null;
        quote: string | null;
        chunk: CitationChunk | null;
        loading: boolean;
        x: number;
        y: number;
      }
    | null
  >(null);

  const suggestions = [
    { icon: 'swords', text: 'Can I cast a spell while grappled?' },
    { icon: 'shield', text: 'How does cover affect spell attacks?' },
    { icon: 'dices', text: 'Roll initiative for the party' },
    { icon: 'book-open', text: "What's in the rulebook I just uploaded?" },
  ];

  async function loadHistory(campaignId: string | null) {
    try {
      const history = await getChatHistory(campaignId);
      messages = history;
    } catch (e) {
      console.error('Failed to load chat history:', e);
    }
  }

  // Refetch when the active campaign changes.
  $effect(() => {
    loadHistory(activeCampaignId);
  });

  // Auto-scroll thread on new messages or while streaming.
  $effect(() => {
    messages;
    currentResponse;
    isLoading;
    if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
  });

  onMount(async () => {
    unlistenListener = await listen<{ token: string; done: boolean }>('chat-token', (event) => {
      if (event.payload.done) {
        if (currentResponse) {
          messages = [...messages, { role: 'assistant', content: currentResponse }];
        }
        currentResponse = '';
        isLoading = false;
      } else {
        currentResponse += event.payload.token;
      }
    });
  });

  onDestroy(() => {
    if (unlistenListener) unlistenListener();
  });

  async function sendMessage(text?: string) {
    const t = (text ?? input).trim();
    if (!t || isLoading) return;
    messages = [...messages, { role: 'user', content: t }];
    input = '';
    isLoading = true;
    currentResponse = '';
    try {
      await chatSend(t, activeCampaignId);
    } catch (e) {
      console.error('Chat send failed:', e);
      isLoading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  }

  async function handleThreadClick(event: MouseEvent) {
    const target = (event.target as HTMLElement | null)?.closest('.citation-badge');
    if (!(target instanceof HTMLElement)) return;
    event.stopPropagation();
    const source = target.dataset.source ?? '';
    const pageStr = target.dataset.page;
    const page = pageStr ? parseInt(pageStr, 10) : null;
    const inlineQuote = target.dataset.quote ?? null;
    const rect = target.getBoundingClientRect();

    if (inlineQuote) {
      citationPopover = {
        source,
        page,
        quote: inlineQuote,
        chunk: null,
        loading: false,
        x: rect.left,
        y: rect.bottom + 6,
      };
      return;
    }
    citationPopover = {
      source,
      page,
      quote: null,
      chunk: null,
      loading: true,
      x: rect.left,
      y: rect.bottom + 6,
    };
    try {
      const chunk = await getChunkForCitation(source, page);
      if (citationPopover && citationPopover.source === source && citationPopover.page === page) {
        citationPopover = { ...citationPopover, chunk, loading: false };
      }
    } catch (e) {
      console.error('Failed to load citation chunk:', e);
      if (citationPopover && citationPopover.source === source && citationPopover.page === page) {
        citationPopover = { ...citationPopover, chunk: null, loading: false };
      }
    }
  }

  function handleWindowClick(event: MouseEvent) {
    if (!citationPopover) return;
    const t = event.target as HTMLElement | null;
    if (t?.closest('.citation-popover') || t?.closest('.citation-badge')) return;
    citationPopover = null;
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') citationPopover = null;
  }

  function hasCitation(text: string): boolean {
    return /\[Source:\s*"/.test(text);
  }

  function plainHtml(text: string): string {
    return renderContent(text);
  }
</script>

<svelte:window onclick={handleWindowClick} onkeydown={handleWindowKeydown} />

<div class="scroll" bind:this={scrollEl}>
  <div class="thread" onclick={handleThreadClick} role="presentation">
    {#each messages as msg, i (i)}
      {#if msg.role === 'user'}
        <div class="msg user">
          <div class="bubble">{msg.content}</div>
          <div class="who-av">GM</div>
        </div>
      {:else if hasCitation(msg.content)}
        <RulingCard data={parseRuling(msg.content)} />
      {:else}
        <div class="msg">
          <div class="who-av eye-badge"><EyeMark size={28} /></div>
          <div class="plain">
            <!-- eslint-disable-next-line svelte/no-at-html-tags -->
            {@html plainHtml(msg.content)}
          </div>
        </div>
      {/if}
    {/each}

    {#if isLoading && currentResponse}
      <div class="msg">
        <div class="who-av eye-badge"><EyeMark size={28} /></div>
        <div class="plain streaming">
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          {@html plainHtml(currentResponse)}
        </div>
      </div>
    {/if}

    {#if isLoading && !currentResponse}
      <div class="msg">
        <div class="who-av eye-badge"><EyeMark size={28} /></div>
        <div class="thinking">
          <span class="tdot"></span><span class="tdot"></span><span class="tdot"></span>
          <span class="label">consulting your tomes…</span>
        </div>
      </div>
    {/if}

    {#if messages.length === 0 && !isLoading}
      <div class="suggest">
        {#each suggestions as s (s.text)}
          <button class="sug" onclick={() => sendMessage(s.text)}>
            <Icon name={s.icon} size={15} />
            {s.text}
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

{#if citationPopover}
  <div
    class="citation-popover"
    style="left: {citationPopover.x}px; top: {citationPopover.y}px"
    role="dialog"
    aria-label="Source passage"
  >
    <div class="popover-header">
      <strong>{citationPopover.source}</strong>
      {#if citationPopover.page !== null}
        <span class="muted">p.{citationPopover.page}</span>
      {/if}
      <button
        type="button"
        class="popover-close"
        aria-label="Close"
        onclick={() => (citationPopover = null)}>×</button>
    </div>
    {#if citationPopover.quote}
      {@const split = splitHeading(citationPopover.quote)}
      {#if split.heading}
        <div class="popover-heading">{split.heading}</div>
      {/if}
      <div class="popover-body popover-quote">"{split.body}"</div>
    {:else if citationPopover.loading}
      <div class="popover-body muted">Loading…</div>
    {:else if citationPopover.chunk}
      {#if citationPopover.chunk.section_heading}
        <div class="popover-heading">{citationPopover.chunk.section_heading}</div>
      {/if}
      <div class="popover-body">{citationPopover.chunk.text}</div>
    {:else}
      <div class="popover-body muted">No supporting quote available.</div>
    {/if}
  </div>
{/if}

<div class="composer-wrap">
  <div class="composer">
    <Icon name="sparkles" size={20} />
    <input
      bind:value={input}
      onkeydown={handleKeydown}
      placeholder="Ask a rule, a name, a place…"
      disabled={isLoading}
    />
    <button class="tool" onclick={onOpenUpload} title="Attach a rulebook" aria-label="Attach a rulebook">
      <Icon name="paperclip" size={18} />
    </button>
    <button class="tool" title="Roll — coming soon" aria-label="Roll dice" disabled>
      <Icon name="dices" size={18} />
    </button>
    <button
      class="send-btn"
      disabled={!input.trim() || isLoading}
      onclick={() => sendMessage()}
      aria-label="Send"
    >
      <Icon name="arrow-up" size={18} />
    </button>
  </div>
</div>

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 18px 26px 8px;
  }
  .thread {
    max-width: 760px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
  }
  .msg {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    margin: 14px 0;
  }
  .msg.user {
    justify-content: flex-end;
  }
  .msg.user .bubble {
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 10px 14px;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 14.5px;
    max-width: 70%;
  }
  .who-av {
    flex: none;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 13px;
    color: var(--fg-2);
  }
  .who-av.eye-badge {
    background: var(--bg-inset);
  }
  .plain {
    flex: 1;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 12px 14px;
    font-family: var(--font-serif);
    font-size: 16px;
    color: var(--fg-2);
    line-height: 1.65;
    white-space: pre-wrap;
    word-wrap: break-word;
    box-shadow: var(--shadow-card);
  }
  .streaming::after {
    content: '▊';
    color: var(--arcane-300);
    animation: blink 0.8s step-end infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0;
    }
  }
  .thinking {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 12px 14px;
    color: var(--fg-3);
    font-family: var(--font-sans);
    font-size: 13.5px;
  }
  .tdot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--arcane-300);
    box-shadow: var(--glow-arcane);
    animation: tdot 1s var(--ease-arcane) infinite;
  }
  .tdot:nth-child(2) {
    animation-delay: 0.15s;
  }
  .tdot:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes tdot {
    0%, 60%, 100% { opacity: 0.35; transform: translateY(0); }
    30% { opacity: 1; transform: translateY(-2px); }
  }
  .suggest {
    margin-top: 24px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: center;
  }
  .sug {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-full);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .sug:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .composer-wrap {
    padding: 12px 26px 20px;
  }
  .composer {
    max-width: 760px;
    margin: 0 auto;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px 8px 14px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-full);
    box-shadow: var(--shadow-card);
  }
  .composer:focus-within {
    border-color: var(--line-glow);
    box-shadow: var(--glow-arcane);
  }
  .composer input {
    flex: 1;
    border: 0;
    background: transparent;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 15px;
    padding: 8px 0;
  }
  .composer input:focus {
    outline: none;
  }
  .composer input::placeholder {
    color: var(--fg-3);
  }
  .tool {
    padding: 8px;
    border-radius: var(--r-md);
    border: 0;
    background: none;
    color: var(--fg-3);
  }
  .tool:hover:not(:disabled) {
    color: var(--fg-1);
  }
  .tool:disabled {
    opacity: 0.45;
  }
  .send-btn {
    padding: 8px 12px;
    border-radius: var(--r-full);
    border: 0;
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
  }
  .send-btn:disabled {
    opacity: 0.5;
    box-shadow: none;
    background: var(--bg-panel-2);
    color: var(--fg-3);
  }
  /* Citation popover (lifted) */
  .citation-popover {
    position: fixed;
    z-index: 100;
    max-width: min(440px, 90vw);
    background: rgba(16, 19, 42, 0.85);
    color: var(--fg-1);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-md);
    backdrop-filter: blur(14px);
    box-shadow: var(--shadow-3);
    overflow: hidden;
  }
  .popover-header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-panel);
    font-family: var(--font-mono);
    font-size: 12.5px;
  }
  .popover-header .muted {
    color: var(--fg-3);
    font-size: 12px;
  }
  .popover-close {
    margin-left: auto;
    background: transparent;
    color: var(--fg-3);
    border: 0;
    font-size: 16px;
    line-height: 1;
  }
  .popover-close:hover {
    color: var(--fg-1);
  }
  .popover-heading {
    padding: 6px 12px 0;
    font-size: 12px;
    color: var(--fg-3);
    font-style: italic;
  }
  .popover-body {
    padding: 10px 12px 12px;
    font-family: var(--font-serif);
    font-size: 14px;
    line-height: 1.5;
    max-height: 320px;
    overflow-y: auto;
    white-space: pre-wrap;
  }
  .popover-body.muted {
    color: var(--fg-3);
    font-style: italic;
  }
  .popover-quote {
    font-style: italic;
    color: var(--fg-2);
  }
  /* Citation badges injected via {@html} need un-scoped styling.
     Defined here scoped to .plain / .why containers — the regex output
     uses class="citation-badge", so a global :global() rule binds it. */
  :global(.citation-badge) {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    padding: 1px 8px;
    border-radius: var(--r-full);
    border: 1px solid var(--line);
    color: var(--arcane-300);
    background: rgba(91, 120, 255, 0.08);
    font-family: var(--font-mono);
    font-size: 12px;
    margin: 0 2px;
    cursor: pointer;
  }
  :global(.citation-badge:hover) {
    border-color: var(--line-strong);
    color: var(--gem);
  }
</style>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/views/OracleView.svelte
git commit -m "feat: add OracleView (chat with ruling cards, suggestions, thinking indicator)"
```

---

## Task 16: Add `CampaignView` (replaces CampaignsPage)

**Files:**
- Create: `src/views/collection-icons.ts`
- Create: `src/views/CampaignView.svelte`

- [ ] **Step 1: Create the collection-icon lookup**

Create `src/views/collection-icons.ts`:

```ts
/** Map a collection name (lowercased) to a Lucide icon name. Falls back to 'book-open'. */
export function collectionIcon(name: string): string {
  const n = name.toLowerCase();
  if (n.includes('rule')) return 'book-open';
  if (n.includes('lore') || n.includes('codex') || n.includes('realm')) return 'castle';
  if (n.includes('home') || n.includes('table')) return 'scroll-text';
  if (n.includes('best') || n.includes('monster') || n.includes('creature')) return 'paw-print';
  return 'book-open';
}
```

- [ ] **Step 2: Create the view**

Create `src/views/CampaignView.svelte`:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from '../components/Icon.svelte';
  import {
    getCollections,
    getCampaignCollections,
    addCampaignCollection,
    removeCampaignCollection,
    getSources,
    deleteSource,
    createCampaign,
    updateCampaign,
    deleteCampaign,
    type Collection,
    type Campaign,
    type Source,
  } from '../lib/commands';
  import { collectionIcon } from './collection-icons';
  import { SvelteMap } from 'svelte/reactivity';

  let {
    activeCampaignId,
    campaigns,
    setActiveCampaignId,
    onOpenUpload,
    refreshCampaigns,
  }: {
    activeCampaignId: string | null;
    campaigns: Campaign[];
    setActiveCampaignId: (id: string | null) => void;
    onOpenUpload: (collectionId: string) => void;
    refreshCampaigns: () => Promise<void>;
  } = $props();

  let collections = $state<Collection[]>([]);
  let subscribed = $state<Collection[]>([]);
  let sourcesByCol = $state<Map<string, Source[]>>(new SvelteMap());
  let expanded = $state<Set<string>>(new Set());
  let error = $state('');

  let manageOpen = $state(false);
  let newName = $state('');
  let newSystem = $state('');
  let editingId = $state<string | null>(null);
  let editName = $state('');
  let editSystem = $state('');

  let active = $derived(campaigns.find((c) => c.id === activeCampaignId) ?? null);

  onMount(async () => {
    try {
      collections = await getCollections();
    } catch (e) {
      error = String(e);
    }
    await refreshSubscribed();
  });

  $effect(() => {
    activeCampaignId;
    refreshSubscribed();
  });

  async function refreshSubscribed() {
    if (!activeCampaignId) {
      subscribed = [];
      return;
    }
    try {
      subscribed = await getCampaignCollections(activeCampaignId);
    } catch (e) {
      error = String(e);
    }
  }

  function isSubscribed(id: string): boolean {
    return subscribed.some((c) => c.id === id);
  }

  async function toggleSubscribe(c: Collection) {
    if (!activeCampaignId) return;
    error = '';
    try {
      if (isSubscribed(c.id)) {
        await removeCampaignCollection(activeCampaignId, c.id);
      } else {
        await addCampaignCollection(activeCampaignId, c.id);
      }
      await refreshSubscribed();
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleExpand(c: Collection) {
    const next = new Set(expanded);
    if (next.has(c.id)) {
      next.delete(c.id);
    } else {
      next.add(c.id);
      if (!sourcesByCol.has(c.id)) {
        try {
          const list = await getSources(c.id);
          sourcesByCol.set(c.id, list);
          sourcesByCol = new SvelteMap(sourcesByCol);
        } catch (e) {
          error = String(e);
        }
      }
    }
    expanded = next;
  }

  async function removeSource(s: Source, colId: string) {
    if (!confirm('Delete this source and all its indexed chunks?')) return;
    try {
      await deleteSource(s.id);
      const list = await getSources(colId);
      sourcesByCol.set(colId, list);
      sourcesByCol = new SvelteMap(sourcesByCol);
    } catch (e) {
      error = String(e);
    }
  }

  async function createNewCampaign() {
    if (!newName.trim()) return;
    try {
      const c = await createCampaign(newName.trim(), newSystem.trim());
      newName = '';
      newSystem = '';
      await refreshCampaigns();
      setActiveCampaignId(c.id);
    } catch (e) {
      error = String(e);
    }
  }

  function startEdit(c: Campaign) {
    editingId = c.id;
    editName = c.name;
    editSystem = c.system ?? '';
  }

  async function commitEdit() {
    if (!editingId || !editName.trim()) {
      editingId = null;
      return;
    }
    try {
      await updateCampaign(editingId, editName.trim(), editSystem.trim());
      editingId = null;
      await refreshCampaigns();
    } catch (e) {
      error = String(e);
      editingId = null;
    }
  }

  async function removeCampaign(c: Campaign) {
    if (!confirm(`Delete campaign "${c.name}"?`)) return;
    try {
      await deleteCampaign(c.id);
      if (activeCampaignId === c.id) setActiveCampaignId(null);
      await refreshCampaigns();
    } catch (e) {
      error = String(e);
    }
  }

  let subCount = $derived(subscribed.length);
  let bookCount = $derived(
    subscribed.reduce((n, c) => n + (sourcesByCol.get(c.id)?.length ?? 0), 0),
  );
</script>

<div class="scroll">
  <div class="cv">
    {#if error}
      <div class="error">{error}</div>
    {/if}

    <section class="hero">
      <div class="gem"></div>
      <div class="hero-text">
        <div class="eyebrow">Campaign</div>
        <h1>{active?.name ?? 'Global — no campaign selected'}</h1>
        <p class="meta">
          {active?.system ?? '—'}
          {#if !active}<span class="hint"> · select or create a campaign below</span>{/if}
        </p>
      </div>
      {#if active}
        <button class="edit" onclick={() => startEdit(active)}>
          <Icon name="pencil" size={14} />
          Edit details
        </button>
      {/if}
    </section>

    <div class="stats">
      <div class="stat"><span class="n">{subCount}</span><span class="l">collections</span></div>
      <div class="stat"><span class="n">{bookCount}</span><span class="l">books loaded</span></div>
      <div class="stat"><span class="n">—</span><span class="l">notebook entries</span></div>
      <div class="stat"><span class="n">—</span><span class="l">sessions logged</span></div>
    </div>

    <section class="manage">
      <button class="manage-head" onclick={() => (manageOpen = !manageOpen)}>
        <Icon name={manageOpen ? 'chevron-down' : 'chevron-right'} size={16} />
        Manage campaigns
        <span class="ct">{campaigns.length}</span>
      </button>
      {#if manageOpen}
        <div class="manage-body">
          {#each campaigns as c (c.id)}
            <div class="manage-row" class:active={activeCampaignId === c.id}>
              {#if editingId === c.id}
                <input class="m-edit" bind:value={editName} placeholder="Name" />
                <input class="m-edit" bind:value={editSystem} placeholder="System (optional)" />
                <button class="m-btn primary" onclick={commitEdit}>Save</button>
                <button class="m-btn" onclick={() => (editingId = null)}>Cancel</button>
              {:else}
                <button class="m-pick" onclick={() => setActiveCampaignId(c.id)}>
                  <span class="m-nm">{c.name}</span>
                  {#if c.system}<span class="m-sys">{c.system}</span>{/if}
                </button>
                <button class="m-btn" onclick={() => startEdit(c)} title="Rename">
                  <Icon name="pencil" size={13} />
                </button>
                <button class="m-btn danger" onclick={() => removeCampaign(c)} title="Delete">
                  <Icon name="trash-2" size={13} />
                </button>
              {/if}
            </div>
          {/each}
          <div class="manage-new">
            <input bind:value={newName} placeholder="New campaign name" />
            <input bind:value={newSystem} placeholder="System (optional)" />
            <button class="m-btn primary" onclick={createNewCampaign}>+ Create</button>
          </div>
        </div>
      {/if}
    </section>

    <section class="collections">
      <div class="sec-head">
        <h2>Source collections</h2>
        <p>
          Subscribe this campaign to the rulebooks and lore it should draw from. Collections are
          shared across campaigns; subscribing is per-campaign.
        </p>
      </div>

      {#if collections.length === 0}
        <p class="muted">No collections yet. Upload a PDF to create one.</p>
      {/if}

      {#each collections as c (c.id)}
        {@const on = isSubscribed(c.id)}
        {@const isOpen = expanded.has(c.id)}
        {@const list = sourcesByCol.get(c.id) ?? []}
        <div class="coll" class:on>
          <button class="coll-head" onclick={() => toggleExpand(c)}>
            <span class="coll-ic"><Icon name={collectionIcon(c.name)} size={18} /></span>
            <span class="coll-text">
              <span class="nm">{c.name}</span>
              <span class="ct">
                {list.length} {list.length === 1 ? 'book' : 'books'} ·
                {#if !activeCampaignId}
                  shared
                {:else if on}
                  subscribed
                {:else}
                  not subscribed
                {/if}
              </span>
            </span>
            <span
              class="sub-toggle"
              class:on
              role="switch"
              aria-checked={on}
              tabindex="0"
              aria-label="Subscribe to {c.name}"
              onclick={(e) => {
                e.stopPropagation();
                toggleSubscribe(c);
              }}
              onkeydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  toggleSubscribe(c);
                }
              }}
            >
              <span class="knob"></span>
            </span>
            <Icon name={isOpen ? 'chevron-up' : 'chevron-down'} size={16} />
          </button>
          {#if isOpen}
            <div class="books">
              {#each list as s (s.id)}
                <div class="book">
                  <Icon name="file-text" size={14} />
                  <span class="bnm">{s.display_name}</span>
                  <span
                    class="book-status"
                    class:ok={s.index_status === 'done'}
                    class:idx={s.index_status === 'pending' || s.index_status === 'indexing'}
                    class:err={s.index_status === 'error'}
                  >
                    {s.index_status === 'done'
                      ? 'Indexed'
                      : s.index_status === 'error'
                        ? 'Error'
                        : 'Indexing…'}
                  </span>
                  <button class="m-btn danger" onclick={() => removeSource(s, c.id)} title="Delete">
                    <Icon name="trash-2" size={13} />
                  </button>
                </div>
              {/each}
              <button class="add-book" onclick={() => onOpenUpload(c.id)}>
                <Icon name="plus" size={14} />
                Add book
              </button>
            </div>
          {/if}
        </div>
      {/each}
    </section>
  </div>
</div>

<style>
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .cv {
    max-width: 820px;
    margin: 0 auto;
    padding: 30px 26px 40px;
    font-family: var(--font-sans);
  }
  .error {
    padding: 8px 12px;
    background: var(--danger-bg);
    color: var(--danger);
    border: 1px solid rgba(242, 103, 75, 0.4);
    border-radius: var(--r-md);
    margin-bottom: 14px;
    font-size: 13px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 18px;
    margin-bottom: 22px;
  }
  .hero .gem {
    width: 56px;
    height: 56px;
    border-radius: var(--r-lg);
    background: var(--grad-gem);
    box-shadow: var(--glow-violet);
    flex: none;
  }
  .eyebrow {
    font-family: var(--font-sans);
    font-weight: 700;
    font-size: 11px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--arcane-300);
    margin-bottom: 4px;
  }
  .hero h1 {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 26px;
    margin: 0;
    color: var(--fg-1);
  }
  .meta {
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--fg-3);
    margin: 4px 0 0;
  }
  .meta .hint {
    color: var(--arcane-300);
  }
  .edit {
    margin-left: auto;
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    color: var(--fg-2);
    font-weight: 600;
    font-size: 13px;
    background: none;
  }
  .edit:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    margin-bottom: 24px;
  }
  .stat {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 12px 14px;
    box-shadow: var(--shadow-card);
  }
  .stat .n {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 700;
    color: var(--fg-1);
    display: block;
  }
  .stat .l {
    font-size: 11.5px;
    color: var(--fg-3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .manage {
    margin-bottom: 22px;
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .manage-head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    background: none;
    border: 0;
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-weight: 600;
    font-size: 13.5px;
    text-align: left;
  }
  .manage-head .ct {
    margin-left: auto;
    color: var(--fg-3);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .manage-body {
    border-top: 1px solid var(--line);
    padding: 8px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .manage-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: var(--r-sm);
  }
  .manage-row.active {
    background: rgba(91, 120, 255, 0.08);
  }
  .m-pick {
    flex: 1;
    text-align: left;
    background: none;
    border: 0;
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
    padding: 6px 8px;
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .m-sys {
    color: var(--fg-3);
    font-size: 12px;
  }
  .m-edit {
    flex: 1;
    padding: 5px 8px;
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .manage-new {
    display: flex;
    gap: 6px;
    padding: 4px 6px;
  }
  .manage-new input {
    flex: 1;
    padding: 5px 8px;
    background: var(--bg-inset);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13px;
  }
  .m-btn {
    padding: 5px 10px;
    background: var(--bg-panel-2);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    color: var(--fg-2);
    font-family: var(--font-sans);
    font-size: 12.5px;
  }
  .m-btn:hover {
    border-color: var(--line-strong);
    color: var(--fg-1);
  }
  .m-btn.primary {
    background: var(--grad-arcane);
    border-color: transparent;
    color: var(--fg-on-accent);
  }
  .m-btn.danger {
    color: var(--danger);
    border-color: rgba(242, 103, 75, 0.4);
  }
  .m-btn.danger:hover {
    background: var(--danger-bg);
  }
  .collections .sec-head {
    margin-bottom: 12px;
  }
  .collections h2 {
    font-family: var(--font-display);
    font-size: 18px;
    margin: 0 0 4px;
    color: var(--fg-1);
  }
  .collections .sec-head p {
    color: var(--fg-3);
    font-size: 13px;
    margin: 0;
  }
  .coll {
    background: var(--bg-panel);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    margin-bottom: 10px;
  }
  .coll.on {
    border-color: var(--line-strong);
  }
  .coll-head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: none;
    border: 0;
    text-align: left;
  }
  .coll-ic {
    color: var(--violet-300);
  }
  .coll-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .coll-text .nm {
    color: var(--fg-1);
    font-weight: 600;
    font-size: 14px;
  }
  .coll-text .ct {
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--fg-3);
  }
  .sub-toggle {
    width: 32px;
    height: 18px;
    border-radius: var(--r-full);
    background: var(--bg-inset);
    border: 1px solid var(--line);
    flex: none;
    position: relative;
    cursor: pointer;
  }
  .sub-toggle .knob {
    position: absolute;
    top: 1.5px;
    left: 1.5px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--fg-3);
    transition: transform var(--dur) var(--ease-arcane), background var(--dur);
  }
  .sub-toggle.on {
    background: rgba(91, 120, 255, 0.3);
    border-color: var(--line-glow);
    box-shadow: var(--glow-arcane);
  }
  .sub-toggle.on .knob {
    transform: translateX(13px);
    background: var(--gem);
  }
  .books {
    border-top: 1px solid var(--line-faint);
    padding: 8px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .book {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 4px;
    font-size: 13px;
    color: var(--fg-2);
  }
  .bnm {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .book-status {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 2px 7px;
    border-radius: var(--r-full);
    background: var(--bg-inset);
  }
  .book-status.ok {
    color: var(--success);
    background: var(--success-bg);
  }
  .book-status.idx {
    color: var(--warning);
    background: var(--warning-bg);
  }
  .book-status.err {
    color: var(--danger);
    background: var(--danger-bg);
  }
  .add-book {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    align-self: flex-start;
    border-radius: var(--r-full);
    border: 1px dashed var(--line);
    background: none;
    color: var(--fg-3);
    font-family: var(--font-sans);
    font-size: 12.5px;
    margin-top: 4px;
  }
  .add-book:hover {
    border-color: var(--line-glow);
    color: var(--arcane-300);
  }
  .muted {
    color: var(--fg-3);
    font-size: 13px;
  }
</style>
```

- [ ] **Step 3: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/views/collection-icons.ts src/views/CampaignView.svelte
git commit -m "feat: add CampaignView (active campaign hero + per-collection book management)"
```

---

## Task 17: Add `Shell` (rail + topbar + views + picker dialog)

**Files:**
- Create: `src/shell/Shell.svelte`

- [ ] **Step 1: Create the shell**

Create `src/shell/Shell.svelte`:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    getCampaigns,
    getCollections,
    createCollection,
    uploadSource,
    getMruCollectionId,
    setMruCollectionId,
    type Campaign,
    type Collection,
  } from '../lib/commands';
  import CampaignRail, { type View } from './CampaignRail.svelte';
  import CampaignSwitcher from './CampaignSwitcher.svelte';
  import Topbar from './Topbar.svelte';
  import OracleView from '../views/OracleView.svelte';
  import CampaignView from '../views/CampaignView.svelte';
  import NotesView from '../views/NotesView.svelte';
  import SettingsView from '../views/SettingsView.svelte';
  import UploadProgress from '../UploadProgress.svelte';
  import { findCategory } from './note-categories';

  const ACTIVE_KEY = 'chronacle_active_campaign_id';

  let view = $state<View>('oracle');
  let campaigns = $state<Campaign[]>([]);
  let activeCampaignId = $state<string | null>(null);
  let switcherOpen = $state(false);

  // Upload dialog state (lifted from old App.svelte)
  let isUploading = $state(false);
  let uploadProgress = $state(0);
  let uploadStatus = $state('');
  let uploadedSourceName = $state('');
  let collections = $state<Collection[]>([]);
  let pendingPath = $state<string | null>(null);
  let pendingName = $state<string | null>(null);
  let showPicker = $state(false);
  let pickerCollectionId = $state('');
  let pickerNewName = $state('');
  let showNewCollectionInput = $state(false);
  let pickerError = $state('');

  onMount(async () => {
    try {
      campaigns = await getCampaigns();
    } catch (e) {
      console.error('Failed to load campaigns:', e);
    }
    const stored = localStorage.getItem(ACTIVE_KEY);
    if (stored && campaigns.some((c) => c.id === stored)) {
      activeCampaignId = stored;
    } else {
      activeCampaignId = null;
    }
  });

  function setActiveCampaignId(id: string | null) {
    activeCampaignId = id;
    if (id) localStorage.setItem(ACTIVE_KEY, id);
    else localStorage.removeItem(ACTIVE_KEY);
  }

  async function refreshCampaigns() {
    campaigns = await getCampaigns();
    if (activeCampaignId && !campaigns.some((c) => c.id === activeCampaignId)) {
      setActiveCampaignId(null);
    }
  }

  let activeCampaign = $derived(campaigns.find((c) => c.id === activeCampaignId) ?? null);

  // Topbar copy
  let head = $derived.by(() => {
    if (view === 'oracle')
      return { title: 'Oracle', sub: 'Ask in plain language — answers come cited' };
    if (view === 'campaign')
      return { title: 'Campaign', sub: 'Manage details & subscribed source collections' };
    if (view === 'settings')
      return { title: 'Settings', sub: 'Provider, models, and re-indexing' };
    const cat = findCategory(view.category);
    return { title: cat.label, sub: cat.sub };
  });

  async function openFilePicker(initialCollectionId?: string) {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    });
    if (!selected) return;
    const path = typeof selected === 'string' ? selected : selected[0];
    const name = path.split('/').pop()?.split('\\').pop() ?? 'document.pdf';
    pendingPath = path;
    pendingName = name;

    if (initialCollectionId) {
      // Skip the picker dialog: upload straight into the given collection.
      await startUpload(path, name, initialCollectionId);
      return;
    }

    try {
      collections = await getCollections();
    } catch (e) {
      console.error('Failed to load collections:', e);
      collections = [];
    }
    const mru = getMruCollectionId();
    pickerCollectionId =
      mru && collections.some((c) => c.id === mru) ? mru : (collections[0]?.id ?? '');
    showPicker = true;
    pickerError = '';
    showNewCollectionInput = false;
    pickerNewName = '';
  }

  async function handlePickerCreateNew() {
    if (!pickerNewName.trim()) return;
    pickerError = '';
    try {
      const newCol = await createCollection(pickerNewName.trim());
      collections = [...collections, newCol];
      pickerCollectionId = newCol.id;
      pickerNewName = '';
      showNewCollectionInput = false;
    } catch (e) {
      pickerError = String(e);
    }
  }

  async function confirmUpload() {
    if (!pickerCollectionId || !pendingPath || !pendingName) return;
    pickerError = '';
    const path = pendingPath;
    const name = pendingName;
    const colId = pickerCollectionId;
    showPicker = false;
    pendingPath = null;
    pendingName = null;
    setMruCollectionId(colId);
    await startUpload(path, name, colId);
  }

  async function startUpload(path: string, name: string, collectionId: string) {
    isUploading = true;
    uploadProgress = 0;
    uploadStatus = 'Uploading…';
    uploadedSourceName = name;
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    try {
      unlistenProgress = await listen<{
        source_id: string;
        status: string;
        progress: number;
        step?: string;
      }>('ingestion-progress', (event) => {
        uploadProgress = Math.round(event.payload.progress * 100);
        if (event.payload.status === 'done') {
          uploadStatus = 'Ready!';
          uploadProgress = 100;
        } else if (event.payload.step) {
          uploadStatus = event.payload.step;
        } else {
          uploadStatus = 'Indexing PDF…';
        }
      });
      unlistenError = await listen<{ source_id: string; error: string }>(
        'ingestion-error',
        (event) => {
          uploadStatus = `Error: ${event.payload.error}`;
          console.error('Ingestion error:', event.payload.error);
          isUploading = false;
        },
      );
      await uploadSource(path, name, 'rules', collectionId);
    } catch (e) {
      uploadStatus = `Upload failed: ${String(e)}`;
      isUploading = false;
    } finally {
      if (unlistenProgress) unlistenProgress();
      if (unlistenError) unlistenError();
      isUploading = false;
    }
  }
</script>

<div class="app">
  <CampaignRail
    {view}
    {activeCampaign}
    setView={(v) => (view = v)}
    onOpenSwitcher={() => (switcherOpen = true)}
    onOpenUpload={() => openFilePicker()}
  />

  {#if switcherOpen}
    <CampaignSwitcher
      {campaigns}
      {activeCampaignId}
      onSelect={setActiveCampaignId}
      onManage={() => (view = 'campaign')}
      onClose={() => (switcherOpen = false)}
    />
  {/if}

  <main class="main">
    <Topbar title={head.title} sub={head.sub} />
    {#if view === 'oracle'}
      <OracleView {activeCampaignId} onOpenUpload={() => openFilePicker()} />
    {:else if view === 'campaign'}
      <CampaignView
        {activeCampaignId}
        {campaigns}
        {setActiveCampaignId}
        onOpenUpload={(colId) => openFilePicker(colId)}
        {refreshCampaigns}
      />
    {:else if view === 'settings'}
      <SettingsView />
    {:else}
      <NotesView category={view.category} />
    {/if}

    <UploadProgress
      filename={uploadedSourceName}
      status={uploadStatus}
      progress={uploadProgress}
      isActive={isUploading}
    />
  </main>

  {#if showPicker}
    <div class="picker-overlay">
      <div class="picker-dialog" role="dialog" aria-modal="true" aria-labelledby="picker-title">
        <h3 id="picker-title">Add "{pendingName}" to collection</h3>
        {#if pickerError}
          <div class="picker-error">{pickerError}</div>
        {/if}
        {#if collections.length > 0}
          <select bind:value={pickerCollectionId} class="picker-select">
            {#each collections as col (col.id)}
              <option value={col.id}>{col.name}</option>
            {/each}
          </select>
        {:else}
          <p class="picker-hint">No collections yet.</p>
        {/if}
        {#if showNewCollectionInput}
          <div class="picker-new">
            <input
              bind:value={pickerNewName}
              placeholder="New collection name"
              onkeydown={(e) => e.key === 'Enter' && handlePickerCreateNew()}
            />
            <button class="picker-create-btn" onclick={handlePickerCreateNew}>Create</button>
            <button class="picker-cancel-btn" onclick={() => (showNewCollectionInput = false)}
              >Cancel</button>
          </div>
        {:else}
          <button class="picker-new-btn" onclick={() => (showNewCollectionInput = true)}
            >+ Create new collection</button>
        {/if}
        <div class="picker-actions">
          <button
            class="picker-cancel-btn"
            data-testid="picker-cancel"
            onclick={() => {
              showPicker = false;
              pendingPath = null;
              pendingName = null;
            }}>Cancel</button>
          <button class="picker-confirm-btn" disabled={!pickerCollectionId} onclick={confirmUpload}
            >Upload</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 264px 1fr;
    height: 100%;
    background:
      radial-gradient(70% 80% at 100% 0%, rgba(123, 92, 255, 0.1), transparent 55%),
      var(--bg-void) var(--tex-starfield);
    background-size: auto, 900px;
    color: var(--fg-1);
    font-family: var(--font-sans);
    position: relative;
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .picker-overlay {
    position: fixed;
    inset: 0;
    background: var(--bg-scrim);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }
  .picker-dialog {
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-3);
    padding: 18px;
    width: 340px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-family: var(--font-sans);
  }
  .picker-dialog h3 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 16px;
    color: var(--fg-1);
  }
  .picker-error {
    color: var(--danger);
    background: var(--danger-bg);
    border-radius: var(--r-sm);
    padding: 6px 10px;
    font-size: 12.5px;
  }
  .picker-select,
  .picker-new input {
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg-inset);
    color: var(--fg-1);
    font-family: var(--font-sans);
    font-size: 13.5px;
  }
  .picker-hint {
    font-size: 13px;
    color: var(--fg-3);
    margin: 0;
  }
  .picker-new {
    display: flex;
    gap: 6px;
  }
  .picker-new input {
    flex: 1;
  }
  .picker-new-btn {
    background: none;
    border: 1px dashed var(--line);
    border-radius: var(--r-md);
    padding: 6px 12px;
    font-size: 12.5px;
    color: var(--fg-3);
    font-family: var(--font-sans);
  }
  .picker-new-btn:hover {
    border-color: var(--line-glow);
    color: var(--arcane-300);
  }
  .picker-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .picker-cancel-btn {
    background: none;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 7px 12px;
    font-size: 13px;
    color: var(--fg-2);
    font-family: var(--font-sans);
  }
  .picker-confirm-btn,
  .picker-create-btn {
    border: 0;
    border-radius: var(--r-md);
    padding: 7px 14px;
    font-size: 13px;
    font-weight: 600;
    background: var(--grad-arcane);
    color: var(--fg-on-accent);
    box-shadow: var(--glow-arcane);
    font-family: var(--font-sans);
  }
  .picker-confirm-btn:disabled {
    opacity: 0.5;
    box-shadow: none;
  }
</style>
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/shell/Shell.svelte
git commit -m "feat: add Shell (rail + topbar + active view + collection picker)"
```

---

## Task 18: Restyle `ModelDownload` and `UploadProgress` in place

**Files:**
- Modify: `src/ModelDownload.svelte`
- Modify: `src/UploadProgress.svelte`

- [ ] **Step 1: Read `ModelDownload.svelte` and `UploadProgress.svelte` to inspect current markup**

Run: `cat src/ModelDownload.svelte src/UploadProgress.svelte`
Expected: small components with their own `<style>` blocks using the old palette (`--bg`, `--accent`, etc.).

- [ ] **Step 2: Replace the legacy palette references in `ModelDownload.svelte` style block**

In `src/ModelDownload.svelte`, perform these substitutions across the file:

| Find | Replace |
|------|---------|
| `var(--bg)` | `var(--bg-void)` |
| `var(--bg-surface)` | `var(--bg-panel)` |
| `var(--bg-input)` | `var(--bg-inset)` |
| `var(--border)` | `var(--line)` |
| `var(--accent)` | `var(--arcane-500)` |
| `var(--accent-hover)` | `var(--arcane-400)` |
| `var(--text)` | `var(--fg-1)` |
| `var(--text-muted)` | `var(--fg-3)` |

If `ModelDownload.svelte` does not define a heading font, add `font-family: var(--font-display);` to the title rule and `font-family: var(--font-sans);` to the body text rule.

- [ ] **Step 3: Repeat the same substitutions in `UploadProgress.svelte`**

Apply the same find/replace table to `src/UploadProgress.svelte`. Additionally, if the component is positioned with `position: fixed`, set `z-index: 150` so it sits above the rail but below the picker dialog (z-index 200).

- [ ] **Step 4: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/ModelDownload.svelte src/UploadProgress.svelte
git commit -m "style: restyle ModelDownload and UploadProgress with new tokens"
```

---

## Task 19: Rewrite `src/app.css`

**Files:**
- Modify: `src/app.css`

- [ ] **Step 1: Replace the entire file contents**

Replace `src/app.css` with:

```css
@import './lib/tokens.css';

:root {
  --tex-starfield: url('./lib/assets/tex-starfield.png');
  --tex-circuit: url('./lib/assets/tex-circuit.png');
  --tex-aura: url('./lib/assets/tex-aura.png');
  --brand-mark: url('./lib/assets/chronacle-icon.png');
}

*,
*::before,
*::after {
  box-sizing: border-box;
}

html,
body {
  margin: 0;
  padding: 0;
  height: 100%;
  background: var(--bg-void);
  color: var(--fg-1);
  font-family: var(--font-sans);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  overflow: hidden;
}

#app {
  height: 100%;
}

button {
  font-family: inherit;
}

textarea {
  font-family: inherit;
}

::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}

::-webkit-scrollbar-track {
  background: transparent;
}

::-webkit-scrollbar-thumb {
  background: rgba(124, 148, 255, 0.16);
  border-radius: 999px;
  border: 3px solid transparent;
  background-clip: padding-box;
}

::-webkit-scrollbar-thumb:hover {
  background: rgba(124, 148, 255, 0.28);
  background-clip: padding-box;
}

:focus-visible {
  outline: none;
  box-shadow: var(--glow-focus);
  border-radius: var(--r-sm);
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.001ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.001ms !important;
  }
}
```

- [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/app.css
git commit -m "style: rewrite app.css to use design-system tokens"
```

---

## Task 20: Swap `App.svelte` to the new shell and delete old pages

**Files:**
- Modify: `src/App.svelte`
- Delete: `src/ChatPage.svelte`
- Delete: `src/CampaignsPage.svelte`
- Delete: `src/SettingsPage.svelte`

This is the integration step. After this commit, the app renders the new UI; old pages are gone.

- [ ] **Step 1: Replace `src/App.svelte` entirely with the new shell-gated version**

Replace `src/App.svelte` with:

```svelte
<script lang="ts">
  import ModelDownload from './ModelDownload.svelte';
  import Shell from './shell/Shell.svelte';

  let modelReady = $state(false);

  function onModelReady() {
    modelReady = true;
  }
</script>

{#if !modelReady}
  <ModelDownload {onModelReady} />
{:else}
  <Shell />
{/if}
```

- [ ] **Step 2: Delete the three legacy pages**

```bash
cd /Users/admin/Code/github.com/nunico/chronacle
git rm src/ChatPage.svelte src/CampaignsPage.svelte src/SettingsPage.svelte
```

- [ ] **Step 3: Verify typecheck and lint**

Run: `pnpm typecheck`
Expected: PASS (no references to the deleted files remain).

Run: `pnpm lint`
Expected: PASS.

- [ ] **Step 4: Verify dev build**

Run: `pnpm build`
Expected: success, output prints bundle size summary. No "module not found" errors.

- [ ] **Step 5: Commit**

```bash
git add src/App.svelte
git commit -m "feat: replace App with model-gated Shell; delete legacy pages"
```

---

## Task 21: Rewrite `App.test.ts`

**Files:**
- Modify: `src/App.test.ts`

The old test referenced the deleted top-bar nav and upload button. The new test asserts: ModelDownload gate gates; once ready, the rail is present with Oracle + Campaign & sources + Settings.

- [ ] **Step 1: Write the new test file**

Replace `src/App.test.ts` with:

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import App from './App.svelte';

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const checkEmbeddingModel = vi.fn();
const downloadEmbeddingModel = vi.fn();
const getCampaigns = vi.fn();
const getCollections = vi.fn();
const getChatHistory = vi.fn();
const getSettings = vi.fn();
const getLlmProviderStatus = vi.fn();
const getCustomProviders = vi.fn();

vi.mock('./lib/commands', () => ({
  checkEmbeddingModel: (...a: unknown[]) => checkEmbeddingModel(...a),
  downloadEmbeddingModel: (...a: unknown[]) => downloadEmbeddingModel(...a),
  getCampaigns: (...a: unknown[]) => getCampaigns(...a),
  getCollections: (...a: unknown[]) => getCollections(...a),
  getChatHistory: (...a: unknown[]) => getChatHistory(...a),
  getSettings: (...a: unknown[]) => getSettings(...a),
  getLlmProviderStatus: (...a: unknown[]) => getLlmProviderStatus(...a),
  getCustomProviders: (...a: unknown[]) => getCustomProviders(...a),
  getMruCollectionId: vi.fn().mockReturnValue(null),
  setMruCollectionId: vi.fn(),
}));

describe('App — model-download gate', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    checkEmbeddingModel.mockResolvedValue(false);
    downloadEmbeddingModel.mockResolvedValue(undefined);
    getCampaigns.mockResolvedValue([]);
    getCollections.mockResolvedValue([]);
    getChatHistory.mockResolvedValue([]);
    getSettings.mockResolvedValue({});
    getLlmProviderStatus.mockResolvedValue({
      provider_type: 'openai',
      model: 'gpt-4o-mini',
      api_key_configured: false,
    });
    getCustomProviders.mockResolvedValue([]);
  });

  it('shows the ModelDownload gate before the model is ready', async () => {
    render(App);
    // ModelDownload renders some recognizable text; either way the rail
    // is not yet rendered.
    await waitFor(() => {
      expect(screen.queryByLabelText('Campaign rail')).toBeNull();
    });
  });

  it('renders the Shell once the model is ready', async () => {
    checkEmbeddingModel.mockResolvedValue(true);
    render(App);
    await waitFor(() => {
      expect(screen.getByLabelText('Campaign rail')).toBeTruthy();
    });
    // Oracle nav item is present
    expect(screen.getByRole('button', { name: /Oracle/i })).toBeTruthy();
    // Campaign & sources footer button
    expect(
      screen.getByRole('button', { name: /Campaign.*&.*sources/i }),
    ).toBeTruthy();
    // Settings icon-only button by aria-label
    expect(screen.getByRole('button', { name: /^Settings$/i })).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the test**

Run: `pnpm test --run src/App.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 3: Commit**

```bash
git add src/App.test.ts
git commit -m "test: rewrite App.test.ts for the new Shell + gate"
```

---

## Task 22: Replace `CampaignsPage.test.ts` with `CampaignView.test.ts`

**Files:**
- Delete: `src/CampaignsPage.test.ts`
- Create: `src/views/CampaignView.test.ts`

- [ ] **Step 1: Delete the old test file**

```bash
cd /Users/admin/Code/github.com/nunico/chronacle
git rm src/CampaignsPage.test.ts
```

- [ ] **Step 2: Write the new test file (TDD: tests describe the behavior we already built in Task 16)**

Create `src/views/CampaignView.test.ts`:

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import CampaignView from './CampaignView.svelte';
import * as commands from '../lib/commands';

vi.mock('../lib/commands', () => ({
  getCollections: vi.fn().mockResolvedValue([]),
  getCampaignCollections: vi.fn().mockResolvedValue([]),
  addCampaignCollection: vi.fn(),
  removeCampaignCollection: vi.fn(),
  getSources: vi.fn().mockResolvedValue([]),
  deleteSource: vi.fn(),
  createCampaign: vi.fn(),
  updateCampaign: vi.fn(),
  deleteCampaign: vi.fn(),
}));

const m = vi.mocked(commands);

function col(id: string, name: string) {
  return { id, name, description: null };
}
function camp(id: string, name: string, system: string | null = null) {
  return { id, name, system };
}
function src(id: string, name: string, status = 'done') {
  return {
    id,
    filename: name,
    display_name: name,
    source_type: 'rules',
    page_count: 12,
    index_status: status,
    embed_model: 'nomic-embed-text-v1.5',
    collection_id: null,
  };
}

describe('CampaignView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getCollections.mockResolvedValue([]);
    m.getCampaignCollections.mockResolvedValue([]);
    m.getSources.mockResolvedValue([]);
  });

  it('renders the active campaign name in the hero', async () => {
    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Hollow Reach', '5e')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
      },
    });
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /Hollow Reach/i })).toBeTruthy();
    });
  });

  it('shows a Global hero when no campaign is active', async () => {
    render(CampaignView, {
      props: {
        activeCampaignId: null,
        campaigns: [],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
      },
    });
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /Global/i })).toBeTruthy();
    });
  });

  it('toggles subscription via the switch and calls addCampaignCollection', async () => {
    m.getCollections.mockResolvedValue([col('c-1', 'Rules')]);
    m.getCampaignCollections.mockResolvedValue([]);

    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Reach')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
      },
    });

    const sw = await screen.findByRole('switch', { name: /Subscribe to Rules/i });
    await fireEvent.click(sw);

    await waitFor(() => {
      expect(m.addCampaignCollection).toHaveBeenCalledWith('camp-1', 'c-1');
    });
  });

  it('expands a collection and calls onOpenUpload(collectionId) on Add book', async () => {
    m.getCollections.mockResolvedValue([col('c-1', 'Rules')]);
    m.getCampaignCollections.mockResolvedValue([col('c-1', 'Rules')]);
    m.getSources.mockResolvedValue([src('s-1', 'PHB.pdf')]);

    const onOpenUpload = vi.fn();
    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Reach')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload,
        refreshCampaigns: vi.fn(),
      },
    });

    // Click the collection header to expand
    const head = await screen.findByRole('button', { name: /Rules/ });
    await fireEvent.click(head);

    // Sources are listed, and Add book is reachable
    await waitFor(() => {
      expect(screen.getByText('PHB.pdf')).toBeTruthy();
    });
    const addBtn = screen.getByRole('button', { name: /Add book/i });
    await fireEvent.click(addBtn);
    expect(onOpenUpload).toHaveBeenCalledWith('c-1');
  });

  it('creates a new campaign and sets it active', async () => {
    const created = camp('new-1', 'New Saga', '5e');
    m.createCampaign.mockResolvedValue(created);
    const setActive = vi.fn();
    const refresh = vi.fn().mockResolvedValue(undefined);

    render(CampaignView, {
      props: {
        activeCampaignId: null,
        campaigns: [],
        setActiveCampaignId: setActive,
        onOpenUpload: vi.fn(),
        refreshCampaigns: refresh,
      },
    });

    // Open Manage campaigns
    const manageHead = await screen.findByRole('button', { name: /Manage campaigns/i });
    await fireEvent.click(manageHead);

    const nameInput = await screen.findByPlaceholderText('New campaign name');
    await fireEvent.input(nameInput, { target: { value: 'New Saga' } });
    const sysInput = screen.getByPlaceholderText('System (optional)');
    await fireEvent.input(sysInput, { target: { value: '5e' } });

    const createBtn = screen.getByRole('button', { name: /\+ Create/ });
    await fireEvent.click(createBtn);

    await waitFor(() => {
      expect(m.createCampaign).toHaveBeenCalledWith('New Saga', '5e');
    });
    await waitFor(() => {
      expect(refresh).toHaveBeenCalled();
      expect(setActive).toHaveBeenCalledWith('new-1');
    });
  });
});
```

- [ ] **Step 3: Run the test**

Run: `pnpm test --run src/views/CampaignView.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 4: Commit**

```bash
git add src/views/CampaignView.test.ts
git commit -m "test: add CampaignView.test.ts (replaces CampaignsPage.test.ts)"
```

---

## Task 23: Add `OracleView.test.ts`

**Files:**
- Create: `src/views/OracleView.test.ts`

- [ ] **Step 1: Write the test**

Create `src/views/OracleView.test.ts`:

```ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import OracleView from './OracleView.svelte';
import * as commands from '../lib/commands';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('../lib/commands', () => ({
  getChatHistory: vi.fn().mockResolvedValue([]),
  chatSend: vi.fn().mockResolvedValue(undefined),
  getChunkForCitation: vi.fn().mockResolvedValue(null),
}));

const m = vi.mocked(commands);

describe('OracleView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getChatHistory.mockResolvedValue([]);
  });

  it('shows suggestion chips when the thread is empty', async () => {
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    expect(
      await screen.findByRole('button', { name: /Can I cast a spell while grappled/i }),
    ).toBeTruthy();
  });

  it('hides suggestions once a message exists', async () => {
    m.getChatHistory.mockResolvedValue([{ role: 'user', content: 'hi' }]);
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /spell while grappled/i })).toBeNull();
    });
  });

  it('renders a ruling card for an assistant message with a [Source] citation', async () => {
    m.getChatHistory.mockResolvedValue([
      {
        role: 'assistant',
        content:
          'Yes, but at disadvantage. The grapple imposes disadvantage on the roll. [Source: "SRD 5.2", p.190, quote: "Speed becomes 0."]',
      },
    ]);
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    await waitFor(() => {
      expect(screen.getByText(/Yes, but at disadvantage/i)).toBeTruthy();
      expect(screen.getByRole('button', { name: /SRD 5\.2 p\.190/i })).toBeTruthy();
    });
  });

  it('Enter submits, calling chatSend with the active campaign id', async () => {
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    const input = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(input, { target: { value: 'How does cover work?' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => {
      expect(m.chatSend).toHaveBeenCalledWith('How does cover work?', 'camp-1');
    });
  });

  it('the paperclip button triggers onOpenUpload', async () => {
    const onOpenUpload = vi.fn();
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload },
    });
    const paperclip = await screen.findByRole('button', { name: /Attach a rulebook/i });
    await fireEvent.click(paperclip);
    expect(onOpenUpload).toHaveBeenCalled();
  });

  it('does not inject raw <script> when a citation source name is malicious', async () => {
    m.getChatHistory.mockResolvedValue([
      {
        role: 'assistant',
        content: 'Foo. [Source: "<script>alert(1)</script>", p.1]',
      },
    ]);
    const { container } = render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    await waitFor(() => {
      expect(container.querySelector('script')).toBeNull();
    });
  });
});
```

- [ ] **Step 2: Run the test**

Run: `pnpm test --run src/views/OracleView.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 3: Commit**

```bash
git add src/views/OracleView.test.ts
git commit -m "test: add OracleView.test.ts covering ruling parse, suggestions, send, and XSS"
```

---

## Task 24: Full verification

**Files:** (none modified)

- [ ] **Step 1: Run the full test suite**

Run: `pnpm test --run`
Expected: all tests PASS. No skipped or failed.

- [ ] **Step 2: Typecheck and lint**

Run: `pnpm typecheck && pnpm lint`
Expected: PASS.

- [ ] **Step 3: Build**

Run: `pnpm build`
Expected: success.

- [ ] **Step 4: Manual UI smoke (dev server)**

Run: `pnpm dev` (in one terminal) and `cargo tauri dev` (if a full Tauri smoke is desired — optional for this task).

Visual checklist (look at the running app and confirm):
- App opens to ModelDownload gate (if model not cached) — gate is styled with new tokens (dark cosmic bg, Cinzel title).
- Once ready, the rail shows on the left with the brand mark + wordmark.
- Campaign card reads "Global / no campaign" (no campaigns yet) or the persisted active campaign.
- Clicking the campaign card opens a popover listing Global + campaigns; Esc closes it.
- Oracle view shows the four suggestion chips when empty.
- Typing into the composer + Enter sends a message; the thinking 3-dot indicator + EyeMark appears.
- If the response contains `[Source: …]` markers, the assistant message renders as a ruling card; clicking the cite pill unfurls the quote.
- Clicking a citation badge inside the rendered HTML opens the citation popover.
- Campaign & sources nav opens the CampaignView; the hero shows the active campaign; Manage campaigns expands; subscribe toggles work; expanding a collection shows its books + "Add book" button.
- Clicking "Add book" inside an expanded collection opens the file dialog directly (no picker dialog).
- Clicking "Upload PDF" in the rail footer opens the file dialog → then the collection picker dialog.
- Settings gear opens SettingsView (restyled, structurally identical).
- A Notebook category nav item opens the "Coming in Phase 2" placeholder.

- [ ] **Step 5: Final commit if anything was tweaked during smoke**

If the smoke test surfaced any minor adjustments (typos, spacing), commit them with a message like `style: smoke-test polish`. Otherwise this task ends here.

---

## Self-review

**Spec coverage:**
- §3 Decisions log: every decision lands in a task (kit hybrid → Tasks 2/3/5/6/8; assets → Task 2; Notebook placeholder → Task 13; Settings via rail gear → Task 14 + 17; switcher popover → Task 10; per-collection Add book → Task 16; delete old pages → Task 20; self-host fonts → Tasks 1+4; ruling-card upgrade → Tasks 7+8+15; campaign mgmt in CampaignView → Task 16).
- §4 File layout: every file listed appears as a Create/Modify in some task (Tasks 2–17, 20, 21–23). The `lib/events.ts` is untouched and not in any task — correct, it didn't change.
- §5 Component contracts: Shell (Task 17), CampaignRail (12), CampaignSwitcher (10), Topbar (11), OracleView (15), CampaignView (16), NotesView (13), SettingsView (14), Icon (6), EyeMark (5), RulingCard (8), NOTE_CATEGORIES (9). All implemented.
- §6 Data flow: no backend changes — confirmed.
- §7 Visuals & assets: tokens (3), fonts (1+4), shell background (19), component styles inline in each component (5–17), citation badge styled in OracleView's `:global()` rule (15).
- §8 Accessibility: `aria-label="Campaign rail"` (12), `role="dialog"` on switcher (10) + picker (17), `role="switch"` (16), `:focus-visible` ring (19), `prefers-reduced-motion` (19), color contrast — relies on token discipline; no test for it.
- §9 Testing: App.test (21), CampaignView.test (22), OracleView.test (23). E2E Playwright updates — **NOT** in this plan; flagged as a follow-up below.
- §10 Risks: malicious citation regression covered in Task 23; localStorage stale id in Task 17 (`refreshCampaigns()`); bundle weight handled by Lucide ESM tree-shaking by default.

**Gaps / follow-ups (outside this plan):**
- Playwright UI e2e updates (`tests/e2e/ui/`) — spec calls for selector changes (Oracle nav, composer placeholder). Should be a separate small follow-up plan or a single task added to a future sprint; not blocking.
- Animated polish (starfield drift, EyeMark glow-pulse) — explicit non-goal.

**Placeholder scan:** no "TBD"/"TODO"/"implement later" lines remain. Each step has its actual code or command.

**Type consistency:** `View` is defined in Task 12 (`CampaignRail.svelte`) and re-imported in Task 17 (`Shell.svelte`). `NoteCategoryId` defined in Task 9 and consumed in Tasks 12, 13, 17. `RulingData`/`Cite` defined in Task 7 and consumed in Task 8 (RulingCard). `Campaign`/`Collection`/`Source` come from the existing `commands.ts` (unchanged). All consistent.
