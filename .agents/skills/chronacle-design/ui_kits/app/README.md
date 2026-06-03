# Chronacle — App UI kit

A high-fidelity, click-through recreation of the **Chronacle app**: the oracle product where a GM asks rules and lore questions and gets cited answers.

Open **`index.html`**.

## What's here
- **Oracle** — the core view. Ask in the composer (try *"can I cast while grappled?"*, *"how does cover work?"*, *"who leads the Concord?"*, *"what happened at Greywater Ford?"*) and Chronacle replies with a **ruling card**: verdict → reasoning → citation pills that **unfurl** the exact source passage. Suggestion chips seed the conversation; a "consulting your tomes…" state plays while it thinks.
- **Notebook** — the campaign's notes, mirroring the on-disk markdown layout. The rail splits them into **Notebook → Sessions** (`sessions/NNN-slug.md`, a numbered timeline of recaps) and **Entities** — eight folders under `entities/`: **Player Characters**, **NPCs**, **Locations**, **Factions**, **Creatures**, **Items**, **Events**, and **Misc**. Each category is a searchable grid (sessions are a timeline list) of note cards showing their `.md` filename; clicking one opens a **detail drawer** with in-world prose, the full file path, a metadata card, linked-entity tags, and an "Ask Chronacle about this" action.
- **Campaign** — opened from the campaign card or the rail footer. Hero + stats + the **source-collections** manager (subscribe toggles, expandable book lists with index status).

## Built with Svelte 5
Components are **Svelte 5** single-file components (`.svelte`, runes: `$state` / `$derived` / `$effect` / `$props`). There is **no build step** — `_loader.js` fetches each `.svelte` source, compiles it in the browser with the official Svelte 5 compiler, and mounts the root. Svelte runtime + compiler load from esm.sh via the page's `<script type="importmap">`.

## Components
| File | Component | Notes |
|---|---|---|
| `Icon.svelte` | `Icon` | Lucide line-icon wrapper. |
| `EyeMark.svelte` | `EyeMark` | The scrying-eye avatar (SVG). |
| `CampaignRail.svelte` | `CampaignRail` | Left rail: brand, campaign card, Oracle + Notebook nav. |
| `RulingCard.svelte` | `RulingCard` | Signature ruling: verdict + why + unfurling citation. |
| `OracleView.svelte` | `OracleView` | Thread + composer + thinking/suggestions. |
| `NotesView.svelte` | `NotesView` + `NoteCard.svelte` + `NoteDrawer.svelte` | Notebook category grid/timeline + detail drawer. |
| `CampaignView.svelte` | `CampaignView` | Campaign management + source collections. |
| `App.svelte` | `App` | Shell: rail + top bar + active view (mounted by `_loader.js`). |
| `data.js` | `export const CHRONACLE` | All fake content (collections, `categories`, `notes`, canned answers). |
| `notes-util.js` | `slugify`, `noteFile` | Derives each note's `.md` filename/path. |
| `_loader.js` | `boot()` | In-browser Svelte compile + mount. |
| `app.css` | — | All kit styles (imports `../../colors_and_type.css`). |

The notebook mirrors the real on-disk layout:

```
<campaign-slug>/
  sessions/
    001-the-awakening.md
  entities/
    player_characters/  npcs/  locations/  factions/
    creatures/          items/ events/      misc/
```

`data.js` keys `notes` by category id; `NoteCard`/`NoteDrawer` derive each `.md` filename/path from the note title via `slugify` (in `notes-util.js`).

This is cosmetic, not production-wired: the oracle matches questions on keywords to canned rulings, and the notebook is seeded fiction. Swap the `CHRONACLE` export for real campaign data + a RAG backend to make it live.
