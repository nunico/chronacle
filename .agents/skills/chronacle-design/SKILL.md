---
name: chronacle-design
description: Use this skill to generate well-branded interfaces and assets for Chronacle, either for production or throwaway prototypes/mocks/etc. Contains essential design guidelines, colors, type, fonts, assets, and UI kit components for prototyping.
user-invocable: true
---

Read the README.md file within this skill, and explore the other available files.
If creating visual artifacts (slides, mocks, throwaway prototypes, etc), copy assets out and create static HTML files for the user to view. If working on production code, you can copy assets and read the rules here to become an expert in designing with this brand.
If the user invokes this skill without any other guidance, ask them what they want to build or design, ask some questions, and act as an expert designer who outputs HTML artifacts _or_ production code, depending on the need.

## Quick orientation

- **Chronacle** is an open-source (AGPL-3.0), local-first **desktop app** — a RAG AI Game Master's assistant that resolves TTRPG rules questions and navigates campaign lore, always **with a citation**. No cloud (yet); it runs on the user's machine. It serves **both fantasy and sci-fi** settings. Voice: a calm, precise, never-bluffing oracle. See README.md §3 for voice, §4 for visual foundations.
- **Aesthetic:** "Arcane Terminal" — an AI oracle bound in a spellbook. Deep cosmic blue-black grounds, electric arcane-blue/violet glow, gem-white highlights, one warm rune-gold accent. **Dark-native only.**
- **Tokens:** `colors_and_type.css` — import it everywhere. Families: Cinzel (display), Spectral (lore/reading), Hanken Grotesk (UI), JetBrains Mono (dice/citations).
- **Assets:** `assets/` (app mark, starfield/circuit/aura textures). The brand **Eye** (scrying-eye SVG) is the assistant avatar — see `ui_kits/*/` for the `EyeMark` component. Never redraw the mark.
- **Icons:** Lucide via CDN, line style, 1.75px stroke. No emoji.
- **UI kits:** `ui_kits/app/` (the oracle product) and `ui_kits/marketing/` (landing page) are working component references to copy from.

## Signature moves (get these right)

- Lead rulings with the **verdict**, then the why, then a mono **citation** pill.
- Glow is emphasis currency — primary actions, focus, the Eye, "thinking" states. Don't over-glow.
- Light borders (low-opacity arcane blue), squircle radii, deep-black + arcane-glow dual elevation, translucent blurred popovers over the starfield.
