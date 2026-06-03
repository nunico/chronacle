# Chronacle — Design System

> **Chronacle** is an **open-source desktop app** (AGPL-3.0) that acts as your TTRPG **Game Master's assistant**. Powered by on-device RAG, it resolves rules questions, cites the exact passage it ruled from, and helps you navigate the lore of your campaign setting — like a learned oracle bound inside your rulebooks. It works equally well for **fantasy and sci-fi** settings (and any homebrew). It runs **entirely on your machine** — no account, no cloud, no telemetry.

This repository is a self-contained **brand + design system**: color and type tokens, fonts, brand assets, a card-based visual reference (the Design System tab), and high-fidelity **UI kits** that recreate Chronacle's product surfaces in HTML/JSX.

---

## 1. Product context

Chronacle sits at the table between a Game Master and their shelf of rulebooks, lore documents, and campaign notes — whether that shelf holds spellbooks or starship manuals. A GM (or player) asks a natural-language question — _"Can I cast a spell while grappled?"_, _"How long does it take to vent a breached airlock?"_, _"What does the Ashen Concord want with the heir?"_ — and Chronacle answers **with a citation**: the ruling, plus the exact rule/lore passage it drew from, so the table can trust it and move on.

It is a **local-first desktop app**, released **open source under AGPL-3.0**. The RAG index and the AI run on the user's own machine; there is no cloud backend (yet) and nothing is uploaded.

**Core jobs:**

- **Rules adjudication** — fast, cited answers to "how does X work?" mid-session.
- **Lore navigation** — ask about people, places, factions, and history of _your_ setting; surface connected entries.
- **Session companion** — quick dice, stat-block lookups, and an at-a-glance campaign rail.

**The feeling we are designing for:** sitting at a candle-lit (or console-lit) table, opening a glowing tome that _knows things_. Ancient and trustworthy, but quick and precise like good software. The "arcane technology" motif — spellbook fused with circuitry — is deliberately **genre-spanning**: it reads as magic to a fantasy table and as advanced tech to a sci-fi one.

### Sources provided

- `assets/chronacle-icon.png` — the **app icon / brand mark**: an open grimoire whose pages are etched with glowing circuit traces, crowned by a floating arcane scrying-gem ("the Eye") on a deep cosmic-dark field. This single mark is the genesis of the entire visual language below.

---

## 2. The system at a glance

**Aesthetic direction — "Arcane Terminal."** An AI oracle bound in a spellbook. Deep cosmic grounds, electric arcane linework (the circuit-traces from the mark), gem-white glow, and the discipline of a good developer tool.

- **Grounds:** cosmic blue-black (`--bg-void` → `--bg-panel`). Dark-first, always.
- **Arcane:** electric blue `#3D5BFF` (primary) → violet `#7B5CFF` (the page-glow). Used as light, not as fill — glows, hairlines, single accents.
- **Gem-white:** `#C8D6FF` highlight, the brightest note (the gem at the Eye's center).
- **Rune-gold:** `#E8B86A` — the one warm color, for treasure/rune/highlight moments and warnings.
- **Type:** **Cinzel** (engraved display caps, the wordmark) · **Spectral** (serif, for lore & reading) · **Hanken Grotesk** (sans, UI chrome) · **JetBrains Mono** (dice, rule citations, stat blocks).

Tokens live in **`colors_and_type.css`**.

---

## 3. Content fundamentals — voice & copy

Chronacle speaks like a **knowledgeable, unflappable GM's aide** — part wise sage, part sharp reference tool. Never whimsical for its own sake; the magic is in _precision_, not purple prose.

- **Person:** Addresses the user as **"you"**; refers to itself rarely and plainly ("Here's the ruling," not "I think that…"). The product name is used as a noun: _"Ask Chronacle."_
- **Tone:** Calm, confident, economical. It resolves; it does not hedge. When uncertain, it says what it _can_ cite and flags the gap — it never bluffs. Trust is the whole product.
- **Casing:** Sentence case everywhere in UI. **Cinzel display headings render in caps** by virtue of the typeface (small-caps energy), not by `text-transform` on body copy.
- **Rulings have a shape:** a short **verdict** first ("Yes — but at disadvantage."), then the **why**, then the **citation** (source + page/section, in mono). Lead with the answer.
- **Lore is written like an in-world archivist** — present tense, evocative but grounded: _"The Ashen Concord keeps no court. Its writ travels by raven and rumor."_
- **Emoji:** **None.** The brand uses arcane glyphs, dice notation, and small line-icons instead. A six-pointed spark/star (✦) or diamond (◆) may appear as a decorative divider, sparingly.
- **Numbers & game data** are always set in mono: `2d6+3`, `DC 15`, `PHB p.190`, `+4 to hit`.

**Voice examples**

- Button: _"Ask Chronacle"_ · _"Show the passage"_ · _"Roll it"_ · _"Add to session"_
- Empty state: _"No rulings yet. Ask anything — a rule, a name, a place."_
- Ruling: **"Yes, but at disadvantage.** You can cast a spell with a somatic component while grappled, but the grapple imposes disadvantage on the attack roll if the spell calls for one. — `SRD 5.2 · Grappling`"
- Citation chip: _"Cited from `Codex of the Hollow Reach · ch. 4`"_
- System note: _"Indexing 312 pages… your tomes will be searchable in a moment."_
- Error: _"That isn't in your indexed sources yet. Upload the rulebook and I'll learn it."_

---

## 4. Visual foundations

**Backgrounds.** Always dark and cosmic. The base is a near-black blue (`--bg-void` `#05060F`). Depth is built in layers: a faint **starfield** (`assets/tex-starfield.png`), occasional **circuit traces** bleeding from a corner or behind a panel (`assets/tex-circuit.png`), and a soft **gem-glow aura** (`assets/tex-aura.png` / `--aura`) pooled behind the most important element on screen. Never flat-fill a hero — there is always one light source. No daylight/white surfaces anywhere in-product.

**Color vibe of imagery.** Cool, nocturnal, high-contrast — blue-violet light against black, with bloom/glow. Think bioluminescence and gemstone refraction. The single warm note (rune-gold) earns its place by contrast. Avoid muddy mid-tones; let darks go truly dark.

**Type in use.** Cinzel for the wordmark and big section titles only (it's expensive — never set paragraphs in it). Spectral carries everything you _read_ (rulings prose, lore entries) — it gives the "open tome" feel. Hanken Grotesk runs the UI: buttons, labels, nav, metadata. JetBrains Mono is reserved for game mechanics and citations, which doubles as a trust signal ("this is sourced data").

**Spacing & layout.** 4pt scale. Generous, unhurried — content breathes against the dark. Reading columns cap around 64–72ch. Key surfaces are fixed: a left **campaign rail**, a top **oracle bar**, content center. Comfortable density (this is a reference tool used mid-conversation, not a dashboard to scan).

**Corner radii.** Squircle-leaning, echoing the icon's rounded-square frame. Cards `--r-lg` (18px) to `--r-xl` (26px); inputs/buttons `--r-md` (12px); chips/pills `--r-full`. Nothing hard-cornered.

**Cards.** Raised panel (`--bg-panel`) on the abyss, a **1px inner hairline** (`--line`) rather than a heavy border, soft deep shadow (`--shadow-card`), and — for the active/important card — a faint outer **arcane glow** and/or a thin top light-line. Cards feel like illuminated pages, not material paper.

**Borders & hairlines.** Borders are _light_, not ink: low-opacity arcane-blue (`--line` family) so edges read as a faint glow catching an edge. The strong variant + `--line-glow` marks focus/active.

**Shadows & elevation.** Two systems stacked: (1) deep, soft black shadows for physical lift in the dark, and (2) **arcane outer-glow** for "energized" states (focus, primary buttons, the active ruling). Inset light-line (`inset 0 1px 0 var(--line-faint)`) gives panels a top-lit edge.

**Glow & bloom.** The signature move. Primary buttons, focus rings, the Eye/avatar, and live/"thinking" states emit a colored glow (`--glow-arcane` / `--glow-violet` / `--glow-gem`). Use it as emphasis currency — if everything glows, nothing does.

**Transparency & blur.** Popovers, the oracle bar, and modals use a translucent panel over `backdrop-filter: blur(14–20px)` so the starfield/circuitry shimmers through — like looking through enchanted glass. Scrims (`--bg-scrim`) dim the world behind modals.

**Hover states.** Surfaces lighten subtly (raise toward `--bg-panel-2`) and their hairline brightens toward `--line-strong`; interactive glyphs shift from `--fg-3` to `--fg-1`. Primary actions intensify their glow on hover rather than changing color.

**Press states.** A quick `scale(0.97)` settle plus a momentary glow dampen — tactile, like pressing an inset gem. Fast (`--dur-fast`).

**Animation.** Smooth and weighty, never bouncy. Default easing `--ease-arcane` (settle). Content fades + rises a few px on enter. The signature motions: a **glow pulse** on "thinking"/streaming, a faint **starfield drift**, and citations that **unfurl** (height + fade) beneath a ruling. Respect `prefers-reduced-motion`.

**Focus / accessibility.** Visible focus = `--glow-focus` arcane ring. Text contrast targets AA on the dark grounds (fg-1/fg-2 over bg-abyss/panel).

---

## 5. Iconography

Chronacle has **no bespoke icon font** (none was provided). The system uses **[Lucide](https://lucide.dev)** — clean 1.5–2px stroke line-icons — loaded from CDN. _(This is a substitution: a consistent, well-stocked open line set that matches the thin, glowing linework of the brand mark. If the team has or wants a custom occult/rune set, swap it in — see Caveats.)_

**Rules of use:**

- **Line, not fill.** Icons are stroked to read like the circuit-traces and constellation lines of the mark. Default `1.75px` stroke, `--fg-3`/`--fg-2`, brightening to `--fg-1` (or a brand glow) when active.
- **Sizes:** 16 / 20 / 24px on a 4pt rhythm; 16 inline with text.
- **Arcane glyphs over clip-art.** For magical concepts prefer geometric glyphs — the **six-pointed spark ✦**, diamond ◆, eye, hexagram — drawn from the mark, rather than literal illustration.
- **Brand motifs (not icons):** the **Eye/gem** (used for the assistant avatar & "thinking" indicator), the **open book**, and **circuit-traces**. These come from `assets/` — never redraw them.
- **No emoji**, ever, in product chrome.

Representative Lucide icons in product: `dices`, `book-open`, `scroll-text`, `sparkles`, `search`, `swords`, `map`, `users`, `shield`, `wand-2`, `quote`, `chevron-right`, `paperclip`, `plus`.

### Brand assets (`assets/`)

| File                 | What it is                                                    |
| -------------------- | ------------------------------------------------------------- |
| `chronacle-icon.png` | The app mark — grimoire + circuit pages + Eye. Logo source.   |
| `tex-starfield.png`  | Tileable constellation/star texture for backgrounds.          |
| `tex-circuit.png`    | Glowing circuit-trace texture (corner bleeds, panel backers). |
| `tex-aura.png`       | Soft violet→blue gem-glow aura for hero light-pooling.        |

---

## 6. Index — what's in this repo

| Path                  | Purpose                                                                                                                                     |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `README.md`           | This file — context, voice, visual foundations, iconography.                                                                                |
| `colors_and_type.css` | All design tokens: color, type families, semantic type roles, spacing, radii, shadows, glows, motion. **Import this everywhere.**           |
| `SKILL.md`            | Agent-Skill manifest so this system can be used as a Claude skill.                                                                          |
| `assets/`             | Brand mark, logo, textures.                                                                                                                 |
| `ui_kits/app/`        | **Chronacle app** UI kit — the oracle/chat product (rulings with citations, lore, dice, campaign rail). `index.html` + Svelte 5 components. |
| `ui_kits/marketing/`  | **Marketing site** UI kit — landing page recreation. `index.html` + Svelte 5 components.                                                    |

**UI kits** are high-fidelity, interactive recreations built with **Svelte 5** (single-file `.svelte` components, runes) — assembled from small reusable components, click-through but not production-wired. They compile in the browser (no build step), so you can just open each kit's `index.html`.
