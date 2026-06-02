# Apply the Chronacle App UI Kit — Design

**Status:** Draft — awaiting review
**Date:** 2026-06-03
**Author:** Claude (with Nico)

---

## 1. Goal

Adopt the "Arcane Terminal" design language (per `.claude/skills/chronacle-design/`) and the **app UI kit** layout in the production Svelte frontend, replacing the current Chat/Campaigns/Settings page structure with a single left-rail-plus-main-view shell. Backend (Tauri commands, Rust services, data model) is untouched; this change is presentational + structural in the frontend only.

The kit's pure-presentation pieces (`EyeMark`, `RulingCard`, `Icon`, tokens, assets) are copied; the kit's wired containers (`App`, `CampaignRail`, `OracleView`, `CampaignView`, `NotesView`) are rebuilt as TypeScript-typed Svelte components against the real `commands.ts` API.

## 2. Non-goals

- Backend changes. No new commands, no schema migrations.
- Notebook backend (Phase 2 — sessions/, entities/<type>/*.md, real RAG over notes). NotesView is a placeholder until then.
- Animated polish — kit's starfield drift, idle EyeMark glow-pulse — deferred to a later pass.
- Mobile / narrow-window collapse. Desktop only; rail does not fold.
- Settings page redesign beyond restyling primitives (LLM provider/custom providers/re-index UX is structurally unchanged).

## 3. Decisions log

| # | Decision | Why |
|---|----------|-----|
| 1 | Full kit adoption (Option C — hybrid) | Copy presentation, rebuild typed wired containers. Avoids retrofitting kit's JS-only/`window.lucide`/single-fake-campaign demo containers. |
| 2 | Copy brand assets into `src/lib/assets/` | Vite-imported; matches existing project conventions. |
| 3 | Notebook: "Coming in Phase 2" empty state | Hides absent backend without dropping the rail's visual structure. |
| 4 | Settings: keep page, gear icon in rail footer | Lowest risk; existing form logic preserved. |
| 5 | Multi-campaign: card opens a switcher popover | Closest to kit's intent; Campaign view manages the active one. |
| 6 | Upload: per-collection `+ Add book` + global Upload PDF button in rail footer | Both flows; per-collection skips the picker dialog, global keeps it for the no-collection case. |
| 7 | Delete old pages (`ChatPage`, `CampaignsPage`, `SettingsPage`) | Logic migrates into new views; no dead code. |
| 8 | Self-host fonts via `@fontsource-variable/*` | Local-first; no Google Fonts runtime fetch. |
| 9 | Upgrade chat output to ruling cards when citations present | Visible behavior change, makes "the signature move" actually appear. |
| 10 | Multi-campaign management (create/rename/delete) folds into CampaignView | Single management surface instead of a separate page. |

## 4. File layout

```
src/
├── App.svelte                   ── REWRITTEN — gate → Shell
├── app.css                       ── REWRITTEN — imports tokens, applies shell base
├── main.ts                       ── modified — adds 4 @fontsource-variable imports
├── lib/
│   ├── commands.ts              ── unchanged
│   ├── events.ts                ── unchanged
│   ├── tokens.css               ── NEW — copy of colors_and_type.css (no Google Fonts @import)
│   └── assets/
│       ├── chronacle-icon.png
│       ├── tex-starfield.png
│       ├── tex-circuit.png
│       └── tex-aura.png
├── components/
│   ├── Icon.svelte              ── NEW — Lucide ESM wrapper (replaces window.lucide)
│   ├── EyeMark.svelte           ── NEW — copied from kit, typed props
│   └── RulingCard.svelte        ── NEW — copied from kit, typed props
├── shell/
│   ├── Shell.svelte             ── NEW — rail + topbar + active view + picker dialog
│   ├── CampaignRail.svelte      ── NEW
│   ├── CampaignSwitcher.svelte  ── NEW — popover anchored to rail's campaign card
│   ├── Topbar.svelte            ── NEW
│   └── note-categories.ts       ── NEW — shared NOTE_CATEGORIES config (rail + NotesView)
├── views/
│   ├── OracleView.svelte        ── NEW — replaces ChatPage
│   ├── CampaignView.svelte      ── NEW — replaces CampaignsPage
│   ├── NotesView.svelte         ── NEW — Phase 2 placeholder
│   └── SettingsView.svelte      ── NEW — restyled SettingsPage
├── ModelDownload.svelte         ── modified — restyled in place; gate logic unchanged
├── UploadProgress.svelte        ── modified — restyled in place
├── App.test.ts                   ── REWRITTEN
├── views/CampaignView.test.ts    ── NEW — replaces CampaignsPage.test.ts
└── views/OracleView.test.ts      ── NEW — covers ruling parse + thinking + suggestions

DELETED:
- src/ChatPage.svelte
- src/CampaignsPage.svelte
- src/SettingsPage.svelte
- src/CampaignsPage.test.ts
```

## 5. Component contracts

### Shell.svelte
State: `view: View`, `activeCampaignId: string | null`, `campaigns: Campaign[]`, plus upload state lifted from current `App.svelte` (`isUploading`, `uploadProgress`, `uploadStatus`, `uploadedSourceName`, `pendingUploadPath`, `pendingUploadName`, `showCollectionPicker`, `pickerCollectionId`, `pickerNewName`, `showNewCollectionInput`, `pickerError`, `collections`).

```ts
type View = 'oracle' | 'campaign' | 'settings'
         | `notebook:${NoteCategoryId}`;
type NoteCategoryId =
  | 'sessions' | 'player_characters' | 'npcs' | 'locations'
  | 'factions' | 'creatures' | 'items' | 'events' | 'misc';
```

- Loads `getCampaigns()` on mount; restores `activeCampaignId` from `localStorage['chronacle_active_campaign_id']`; falls back to `null` (Global).
- Persists `activeCampaignId` to `localStorage` whenever it changes.
- Provides via props/callbacks to children:
  - `setView(v: View)`
  - `setActiveCampaignId(id: string | null)`
  - `openFilePicker(initialCollectionId?: string)` — if `initialCollectionId` is given, skips the picker dialog and starts upload directly into that collection.
  - `refreshCampaigns()` — re-fetches after create/delete/rename.
- Renders: `<CampaignRail … />` + `<main>{<Topbar/>}{<viewComponent/>}</main>`.
- Owns the collection-picker dialog (lifted verbatim from current `App.svelte`) and the `<UploadProgress>` strip (fixed top-right, beneath topbar).

### CampaignRail.svelte
Props: `view: View`, `activeCampaign: Campaign | null`, `setView`, `onOpenSwitcher`, `onOpenUpload`.

- Brand block: `--brand-mark` background image + Cinzel wordmark "Chron**a**cle".
- Campaign card: shows `activeCampaign?.name ?? 'Global'` + `system ?? '—'`; clicking calls `onOpenSwitcher`. Active state when `view === 'campaign'`.
- Primary nav (single item): **Oracle** (sparkles icon).
- Notebook section header → **Sessions** item (history icon).
- Entities section header → 8 items from `NOTE_CATEGORIES` (shared static config — see below): PCs (users-round), NPCs (drama), Locations (map-pin), Factions (flag), Creatures (paw-print), Items (gem), Events (milestone), Misc (shapes). Counts shown as `—` (Phase 2).
- Footer (3 buttons, side-by-side or stacked):
  - **Upload PDF** (`upload` icon) → `onOpenUpload()`.
  - **Campaign & sources** (`library` icon) → `setView('campaign')`.
  - **Settings** (`settings` gear icon) → `setView('settings')`.

### CampaignSwitcher.svelte
Props: `campaigns: Campaign[]`, `activeCampaignId: string | null`, `onSelect(id: string | null)`, `onManage()`, `onClose()`.

- `role="dialog"`, translucent blurred popover anchored to the rail's campaign card.
- Lists `Global` (id = null) + all campaigns. Active highlighted with `--line-glow`.
- Each row sets `activeCampaignId` via `onSelect` and closes.
- Bottom row: `+ Manage campaigns…` → `onManage()` which routes to Campaign view.
- Closes on outside click + Escape; first focusable element focused on open.

### OracleView.svelte (replaces ChatPage)
Props: `activeCampaignId: string | null`, `onOpenUpload: () => void` (wired by Shell, drives the composer's paperclip).

Logic ported **verbatim** from `ChatPage.svelte`:
- `messages`, `input`, `isLoading`, `currentResponse` state.
- `getChatHistory(activeCampaignId)` on mount; refetch when prop changes.
- `chat-token` event listener via `listen()`; unlistens on destroy.
- `chatSend(text, activeCampaignId)` on submit.
- Citation parser `renderContent(text)` (regex over `[Source: "X"(, p.N(-M)?)?(, quote: "…")? ]`) → `<button class="citation-badge">` markup with `data-*` attributes.
- `splitHeading(quote)` heuristic.
- Citation popover: click on `.citation-badge` opens floating popover with inline `data-quote` or fallback `getChunkForCitation(source, page)`.
- `handleWindowClick` / `handleWindowKeydown` (Esc) dismiss the popover.

New rendering rules:
- **User message** → right-aligned bubble (`bg-panel-2`, no glow), with `GM` avatar disc on the right.
- **Assistant message containing `[Source: …]` markers** → render via `<RulingCard data={parseRuling(content)} />`. Parser:
  ```ts
  function parseRuling(text: string): RulingData {
    // verdict = first sentence (split on first '. ' or '\n'), stripped of trailing space
    // why    = remaining text with renderContent() applied to embed citation buttons
    // cites  = each [Source: …] marker → { label: `${name}${page?` p.${page}`:''}`, src: `${name}${page?` · p.${page}`:''}`, quote }
    //          (markers without an inline quote contribute { quote: '' } and the unfurl shows "No supporting quote available.")
  }
  ```
- **Assistant message without citations** → render in a simpler ruling card (verdict-less) — single Spectral-serif paragraph on `bg-panel` with EyeMark avatar.
- **Streaming partial response** → same as above but with an ellipsis cursor at the end (existing `.streaming::after`).

Thinking state (`isLoading && !currentResponse`):
- Kit's 3-dot indicator + EyeMark badge + "consulting your tomes…" label.

Composer (lifted from kit):
- Sparkles icon (left), `<input>` placeholder `"Ask a rule, a name, a place…"`, Paperclip → triggers `Shell.openFilePicker()` (via callback prop), Dice → no-op for now (button rendered, `disabled` with tooltip "Roll — coming soon"), Send arrow.
- Enter submits, Shift+Enter newline (current `ChatPage` behavior).

Suggestion chips: shown only when `messages.length === 0 && !isLoading`. Four hard-coded prompts, genre-neutral:
- "How does cover affect spell attacks?"
- "Can I cast a spell while grappled?"
- "Roll initiative for the party"
- "What's in this PDF I just uploaded?"

Campaign context selector: replaced by the rail's campaign card / switcher popover. No in-Oracle dropdown.

### CampaignView.svelte (replaces CampaignsPage)
Props: `activeCampaignId: string | null`, `campaigns: Campaign[]`, `setActiveCampaignId`, `onOpenUpload(collectionId: string)`, `refreshCampaigns()`.

Sections (top to bottom):

1. **Hero** — gem dot + eyebrow `CAMPAIGN` + h1 with active campaign name (or "Global — no campaign selected" when null) + meta `system · last-touched` (last-touched = `—` until we add the field; Phase 1 just shows system).
   - Inline rename/system edit on click of "Edit details" button.
   - If `activeCampaignId === null`, hero shows a CTA "Select or create a campaign to manage details." with a button → opens the campaigns list section below.

2. **4 stat tiles** — `# collections subscribed`, `# books across them` (sum of `sources.length`), `notebook entries: —`, `sessions logged: —`.

3. **Manage campaigns** (collapsible, default closed) — list all campaigns; each row has rename pencil and trash icon; bottom: "+ New campaign" inline form (name + system). Calls `createCampaign`/`updateCampaign`/`deleteCampaign` then `refreshCampaigns()`.

4. **Source collections** — every collection in `getCollections()` renders as a row:
   - Icon — static lookup by `collection.name.toLowerCase()` → Lucide icon (`'rules' → 'book-open'`, `'lore' → 'castle'`, `'homebrew' → 'scroll-text'`, default `'book-open'`).
   - Name + `${sources.length} books · {subscribed | not subscribed}` meta.
   - Subscribe toggle (`role="switch"`, kit's `.sub-toggle.on` styling). Calls `addCampaignCollection` / `removeCampaignCollection`. Disabled when `activeCampaignId === null`.
   - Expand chevron. When open, lists the collection's sources with index status pills (`Indexed` green / `Indexing…` amber / `Error` red, mapped from `source.index_status` ∈ {done, pending, indexing, error}).
   - Inside the expanded section: **`+ Add book`** button → `onOpenUpload(collection.id)`.
   - Each source row has a trash icon → `deleteSource(id)` (existing confirm prompt).

State invariants:
- Subscribed-collection list is derived from `getCampaignCollections(activeCampaignId)`; cached in component state, refetched on toggle.
- Sources per-collection fetched lazily on first expand; cached in a `Map<collectionId, Source[]>`.

### NotesView.svelte
Props: `category: NoteCategoryId`.

Renders the kit's `notes-head` (label + sub + folder path, looked up from the shared `NOTE_CATEGORIES` config), then a single empty-state card on `--bg-panel`:

> ✦ **Coming in Phase 2.** Your campaign's `<folder>/` will live here — searchable notes, file-backed, linked to entities Chronacle can answer about.

No backend calls. No state.

### NOTE_CATEGORIES (shared config)
Lives in `src/shell/note-categories.ts`. Used by `CampaignRail` (nav items, icons) and `NotesView` (label, sub, folder). Mirrors the kit's `data.js` `categories` array minus the demo content:

```ts
export type NoteCategoryId =
  | 'sessions' | 'player_characters' | 'npcs' | 'locations'
  | 'factions' | 'creatures' | 'items' | 'events' | 'misc';

export interface NoteCategory {
  id: NoteCategoryId;
  label: string;
  icon: string;            // Lucide name
  group: 'Notebook' | 'Entities';
  folder: string;          // e.g. 'entities/locations'
  sub: string;             // short description used in NotesView head
}

export const NOTE_CATEGORIES: NoteCategory[] = [ /* 9 entries, kit-faithful */ ];
```

### SettingsView.svelte
Same logic as current `SettingsPage` (LLM provider form, custom providers list/add/edit/delete, re-index all sources). Restyled:
- Cards on `--bg-panel` with `--shadow-card`.
- Hairline borders via `--line`.
- Primary "Save & Connect" button gets `box-shadow: var(--glow-arcane)`.
- Status banners use `--success-bg` / `--danger-bg` / `--warning-bg` (no daylight greens/reds).
- Replace `✅` / `❌` text indicators with Lucide `check` / `x` icons.
- All inputs use `--font-sans`; the "Re-index all sources" technical detail uses `--font-mono`.

### Icon.svelte (new contract)
```svelte
<script lang="ts">
  import { icons, createElement, type LucideIcon } from 'lucide';
  let { name, size = 18, strokeWidth = 1.75, color = '', className = '' }: {
    name: string;
    size?: number;
    strokeWidth?: number;
    color?: string;
    className?: string;
  } = $props();
  let el = $state<HTMLSpanElement>();
  $effect(() => {
    if (!el) return;
    // Lucide icon names are kebab-case (e.g. "book-open"); the icons map keys are PascalCase ("BookOpen").
    const pascal = name.split('-').map(p => p[0].toUpperCase() + p.slice(1)).join('');
    const iconNode = (icons as Record<string, LucideIcon>)[pascal];
    if (!iconNode) return;
    el.innerHTML = '';
    const svg = createElement(iconNode);
    svg.setAttribute('width', String(size));
    svg.setAttribute('height', String(size));
    svg.setAttribute('stroke-width', String(strokeWidth));
    el.appendChild(svg);
  });
</script>
<span bind:this={el} class={className} style="display:inline-flex;align-items:center;justify-content:center;width:{size}px;height:{size}px;{color ? `color:${color};` : ''}"></span>
```

Tree-shaking: Vite drops unused icons from the `icons` map at build (Lucide ships per-icon ESM exports). If bundle size becomes a concern, switch to explicit `import { BookOpen, Search, … } from 'lucide';` and a name → component lookup.

### EyeMark.svelte / RulingCard.svelte
Copied from kit, props typed:
```ts
// EyeMark
let { size = 34, glow = true }: { size?: number; glow?: boolean } = $props();
// RulingCard
type Cite = { label: string; src: string; quote: string };
type RulingData = { verdict: string; why: string; cites: Cite[] };
let { data, defaultOpen = false }: { data: RulingData; defaultOpen?: boolean } = $props();
```

`RulingCard`'s `{@html data.why}` is preserved — `why` is generated by `renderContent()` which only inserts `<button class="citation-badge" data-*="…">` markup with HTML-escaped + attribute-escaped content. Same risk profile as today's `ChatPage` (identical helper).

## 6. Data flow

Backend commands & events: **unchanged**. Same `commands.ts`, same `chat-token` / `ingestion-progress` / `ingestion-error` / `reindex-progress` / `model-download-progress` events.

New client state:
- `activeCampaignId` in `Shell` (persisted to `localStorage`).
- Campaign list cached in `Shell`, invalidated via `refreshCampaigns()` callback after create/delete/rename in CampaignView.
- Subscribed-collection list cached in CampaignView, refetched on toggle.
- Sources-per-collection cached lazily on first expand.

No new Tauri commands. No new events.

## 7. Visuals & assets

### Tokens
- `src/lib/tokens.css` — verbatim copy of `.claude/skills/chronacle-design/colors_and_type.css`, **minus** the Google Fonts `@import` line.
- `src/main.ts` adds:
  ```ts
  import '@fontsource-variable/cinzel';
  import '@fontsource-variable/spectral';
  import '@fontsource-variable/hanken-grotesk';
  import '@fontsource-variable/jetbrains-mono';
  ```
- `src/app.css` imports tokens at the top, then defines:
  ```css
  :root {
    --tex-starfield: url('./lib/assets/tex-starfield.png');
    --tex-circuit:   url('./lib/assets/tex-circuit.png');
    --tex-aura:      url('./lib/assets/tex-aura.png');
    --brand-mark:    url('./lib/assets/chronacle-icon.png');
  }
  ```
- App shell background lifted from kit `app.css`: radial-gradient violet bleed top-right + `var(--bg-void)` + starfield texture at 900px tile.

### Component styles
Live inside each `.svelte` file's `<style>` block (scoped). Kit's monolithic `app.css` is sliced up: each component carries its own rules. Cleaner than the kit's "one global stylesheet" pattern.

### Signature visual moves
- Glow only on: primary actions (Send, Save & Connect, active rail items), focus rings, EyeMark, "thinking" indicator. Secondary buttons hover-lighten without glow.
- All `[Source: …]` rendering in `var(--font-mono)` at 13px.
- Dual elevation: `--shadow-card` (deep + inset hairline) on rulings, hero, collection rows; active card adds `box-shadow: var(--glow-arcane)`.
- Translucent popovers: `CampaignSwitcher` + citation popover use `background: rgba(16,19,42,0.8); backdrop-filter: blur(14px); border: 1px solid var(--line-strong);`.

### Icons
- Lucide via the new `Icon.svelte` (default `strokeWidth = 1.75`).
- All current emoji (📚, 📂, ✏, ✖, ✕, ✅, ❌) replaced: `library`, `folder`, `pencil`, `x`, `trash-2`, `check`, `x`.

## 8. Accessibility

- All interactive elements are `<button>` with `title` + `aria-label` where icon-only.
- `CampaignSwitcher` is `role="dialog"`, focuses first item on open, closes on Esc + outside click.
- Subscribe toggles use `role="switch"` + `aria-checked` (kit preserved).
- `:focus-visible` outline = `--glow-focus` ring, applied globally in `app.css`.
- `@media (prefers-reduced-motion: reduce)` disables glow-pulse and the kit's drift animations.
- Contrast: `--fg-1` on `--bg-abyss` clears AA. `--fg-3` on `--bg-panel` is below AA — restricted to non-essential meta (counts, tags), never interactive labels.

## 9. Testing

### Unit (Vitest)
- `App.test.ts` — REWRITTEN. Asserts: ModelDownload gate still gates; once `modelReady`, the rail renders (`aria-label="Campaign rail"`) with Oracle nav, Campaign & sources button, Settings gear. Drops the old "Chat/Campaigns/Settings top nav" assertions.
- `views/CampaignView.test.ts` — NEW (replaces `CampaignsPage.test.ts`). Asserts: subscribe toggle calls `addCampaignCollection`, expand shows sources, `+ Add book` calls `onOpenUpload(collectionId)` (mocked prop).
- `views/OracleView.test.ts` — NEW. Asserts: citation-bearing message renders as RulingCard with parsed verdict/why/cites; thinking indicator visible during streaming; suggestion chips hidden once messages exist; Enter submits / Shift+Enter inserts newline; malicious `[Source: …]` payload does not inject `<script>` (regression test for the `{@html}` path).

### E2E (Playwright)
- `tests/e2e/backend/` — no changes (drives Tauri commands directly).
- `tests/e2e/ui/` — update the smallest set: navigate via rail Oracle item instead of top Chat button, query composer by `placeholder="Ask a rule, a name, a place…"`. Other PDF-upload + chat-citation paths still work.

### Manual visual check
After each implementation phase: `pnpm dev`, screenshot Oracle empty-state, an active ruling with unfurled citation, Campaign view (collections subscribed + expanded), Settings, ModelDownload gate, ingestion progress strip, citation popover.

## 10. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| Lucide ESM bundle weight | ~15 icons used; tree-shaking via per-icon import keeps it under 30 KB. Switch to explicit imports if needed. |
| `{@html}` in RulingCard | `why` is generated by our `renderContent()` which only inserts `<button>` markup with attribute- and HTML-escaped text. Same risk as today. Regression test for malicious citation payload. |
| Variable-font bundle weight | 4 `@fontsource-variable/*` packages ≈ 200–400 KB. Acceptable for a desktop app. |
| Tauri import map / dev server | All new code uses ESM imports; Vite handles. `lucide` and `@fontsource-variable/*` are standard npm packages. |
| Citation regex sees `[Source: …]` mid-ruling | Same parser as today; behavior unchanged. |
| `localStorage` `activeCampaignId` stale after delete | On `getCampaigns()` load, if persisted id is not in the list, reset to `null`. |

## 11. Migration steps (for the implementation plan)

1. Install: `lucide`, `@fontsource-variable/cinzel`, `@fontsource-variable/spectral`, `@fontsource-variable/hanken-grotesk`, `@fontsource-variable/jetbrains-mono`.
2. Copy 4 brand assets to `src/lib/assets/`. Add `src/lib/tokens.css` (verbatim from design system, minus Google Fonts `@import`). Rewrite `src/app.css` (imports tokens, applies kit shell rules globally, removes legacy palette).
3. Add font imports to `main.ts`. Add `src/components/Icon.svelte` (Lucide ESM wrapper), `EyeMark.svelte`, `RulingCard.svelte`.
4. Build `src/shell/` — `Shell.svelte`, `CampaignRail.svelte`, `CampaignSwitcher.svelte`, `Topbar.svelte`. Wire campaign list, `activeCampaignId` localStorage, collection picker dialog, `UploadProgress` (lifted from current `App.svelte`).
5. Build `src/views/OracleView.svelte` — port all `ChatPage` logic, add the ruling-card parse.
6. Build `src/views/CampaignView.svelte` — port `CampaignsPage` collection management + add active-campaign hero + per-collection `+ Add book` + collapsible Manage campaigns.
7. Add `src/views/NotesView.svelte` (Phase 2 placeholder).
8. Restyle `src/views/SettingsView.svelte` (logic kept).
9. Restyle `src/ModelDownload.svelte` and `src/UploadProgress.svelte` in place.
10. Replace `src/App.svelte` with the new gate→Shell pattern. Delete `ChatPage.svelte`, `CampaignsPage.svelte`, `SettingsPage.svelte`.
11. Rewrite `App.test.ts`, replace `CampaignsPage.test.ts` with `views/CampaignView.test.ts`, add `views/OracleView.test.ts`.
12. `pnpm typecheck && pnpm lint && pnpm test --run` ; manual UI smoke per the visual check list.
