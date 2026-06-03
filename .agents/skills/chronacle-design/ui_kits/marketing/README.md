# Chronacle — Marketing site UI kit

A high-fidelity recreation of the **Chronacle marketing landing page**.

Open **`index.html`**.

## Sections
- **Nav** — sticky, blurred, with brand lockup + primary CTA.
- **Hero** — floating app-mark halo, gradient headline, lede, dual CTAs, and a **live demo ruling card** showing the product's signature verdict-and-citation output.
- **Features** — six cards (cited rulings, import your tomes, fantasy-or-sci-fi, house-rule overrides, inline dice, runs-on-your-machine).
- **How it works** — three connected steps.
- **Open source** — a "Free & open source" panel: AGPL-3.0 framing, Download + Star-on-GitHub CTAs, macOS/Windows/Linux platform buttons, and an offline/no-cloud/fork-it meta row. (No SaaS pricing — Chronacle is a local-first desktop app.)
- **CTA band** + **footer**.

## Built with Svelte 5
Components are **Svelte 5** single-file components (`.svelte`, runes). No build step — `_loader.js` compiles each `.svelte` in the browser with the official Svelte 5 compiler and mounts the root; runtime + compiler load from esm.sh via the page's importmap.

## Components
| File | Component | Notes |
|---|---|---|
| `Icon.svelte` · `EyeMark.svelte` | `Icon`, `EyeMark` | Shared chrome — Lucide wrapper + scrying-eye SVG. |
| `Nav.svelte` · `Footer.svelte` | `Nav`, `Footer` | Sticky nav + footer. |
| `Hero.svelte` | `Hero` | Hero + demo ruling card. |
| `Features.svelte` · `HowItWorks.svelte` · `OpenSource.svelte` · `CtaBand.svelte` | — | Page body sections. |
| `Site.svelte` | `Site` | Composes the page (mounted by `_loader.js`). |
| `_loader.js` | `boot()` | In-browser Svelte compile + mount. |
| `marketing.css` | — | All styles (imports `../../colors_and_type.css`). |

Copy is illustrative, in Chronacle's voice. Download links, platform builds, and the GitHub URL are placeholders.
