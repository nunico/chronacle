# Chronacle Website and Manual Design

**Date:** 2026-08-16
**Status:** Approved for implementation planning

## Summary

Build a static public website for Chronacle under `apps/website`. The website combines a restrained
product landing page with a bilingual, searchable user manual. It uses SvelteKit, TypeScript, and
Tailwind CSS and follows the Chronacle “Arcane Terminal” design system.

The landing page uses one unprefixed route, `/`, and changes its copy between English and German in
place. The manuals use explicit locale routes:

- `/en/manual/...`
- `/de/handbuch/...`

The site is useful before it is persuasive. It explains what Chronacle does, shows a cited-answer
example, provides a download route, and makes the manual fast to navigate during actual use. Copy is
informal and specific. It avoids exaggerated promises, fake social proof, and unnecessary references
to protected game or product names.

## Goals

- Explain Chronacle accurately in a short, informal landing page.
- Make the complete manual easy to browse on desktop and mobile.
- Provide fast, language-scoped full-text search without a server.
- Publish first-class English and German content with an extensible locale model.
- Produce a fully static build that can be hosted on ordinary static hosting.
- Preserve the Chronacle visual identity without copying the desktop application layout directly.
- Keep the site free of cookies, local storage, analytics, and consent banners.
- Attribute any SRD 5.1 Open Game Content correctly under OGL 1.0a.

## Non-goals

- A hosted Chronacle service, account system, or cloud dashboard.
- Analytics, behavioural tracking, advertising, or newsletter collection.
- A blog, community forum, pricing page, or speculative roadmap.
- Runtime content management or server-side search.
- Cross-language search.
- Platform-specific download asset discovery at runtime.
- Additional locales in the first release.

## Package and build architecture

The package is named `apps/website`. “Website” accurately covers both public product information and
the manual; “info-site” is vague, while “guide” would understate the landing page.

The package is a SvelteKit application configured with the static adapter. All public routes are
prerendered. Tailwind CSS provides the utility and build foundation. Brand colours, typography,
radii, shadows, and motion come from local CSS tokens derived from the `chronacle-design` skill.
Chronacle’s approved mark and texture assets are copied into the package rather than referenced from
the agent skill directory.

Fonts are self-hosted so page rendering does not make requests to a third-party font service. The
site contains no analytics or third-party embeds.

Manual articles are Markdown compiled through mdsvex. A typed content registry reads their
frontmatter and drives routing, section order, breadcrumbs, navigation, translation pairing, and
search metadata. Pagefind indexes the rendered static manual after the SvelteKit build. Separate
English and German indexes prevent cross-language results.

The root pnpm workspace includes `apps/website`. Website checks become part of the repository’s
frontend quality script so the package does not sit outside the normal pull-request gate.

## Content model

Canonical manual content lives at:

```text
apps/website/src/content/manual/
├── en/
│   ├── getting-started/
│   ├── ai-providers/
│   ├── source-library/
│   ├── campaigns/
│   ├── codex/
│   ├── notes-and-sessions/
│   ├── vault/
│   ├── settings/
│   ├── troubleshooting/
│   └── glossary/
└── de/
    └── matching translated sections
```

Every page has validated frontmatter with:

- a stable translation key;
- locale;
- route slug;
- title and summary;
- section identifier;
- section and page order;
- optional on-page navigation label;
- optional search exclusion flag.

English is the editorial source. German follows the same information architecture but is translated
as natural German rather than sentence-by-sentence literal text. Stable translation keys pair pages
even where English and German slugs differ. Adding a future locale requires a locale definition,
translated navigation labels, and a new content tree; it does not change shared components.

The existing `docs/user-guide.md` and `docs/user-guide/` content is migrated, rewritten, and then
removed. The website content becomes the only canonical user manual. Migration is complete only
when every still-current instruction has a destination and outdated instructions have been corrected
against the application.

## Routes and locale behaviour

### Landing page

`/` is the only landing-page route. It is statically rendered in English for a useful no-JavaScript
fallback. On each fresh visit, client-side locale initialisation reads the browser’s preferred
language. German browsers see German when JavaScript starts; English and all other browsers see
English.

The language control changes all landing-page copy in place, including navigation, metadata exposed
to client-side navigation, call-to-action labels, accessible names, and manual links. It stores
nothing. Reloading the page applies browser-language detection again.

### Manual

Manual routes are explicit and deterministic:

- English: `/en/manual` and `/en/manual/<section>/<page>`
- German: `/de/handbuch` and `/de/handbuch/<section>/<page>`

The manual does not redirect based on browser language. Its language switch uses the current page’s
translation key to navigate directly to the paired page. Every published page must have both an
English and German version, so the switch never drops the user at an unrelated overview.

Landing-page manual links follow the language currently selected on that page. A direct link to a
manual route always wins over browser preference.

### External routes

- Download actions open the repository’s latest releases page.
- Source actions open the repository root.
- External links are visually and accessibly identified where the context does not already make the
  destination clear.

## Landing-page composition

The landing page uses a concise sequence:

1. **Header** — Chronacle mark and wordmark, section link, manual link, source link, language control,
   and download action.
2. **Hero** — the plain-language idea: add reference material, ask a question, receive an answer with
   a citation. Primary action is “Download Chronacle”; secondary action opens the matching manual.
3. **Product example** — a framed, realistic question, direct verdict, short explanation, and source
   citation. This demonstrates the product instead of relying on broad claims.
4. **Useful capabilities** — source search, cited answers, campaign material, and structured notes.
5. **Three-step flow** — add sources, ask normally, check the cited passage.
6. **Storage and provider explanation** — accurate boundaries around local storage and online AI
   providers.
7. **Download section** — supported desktop platforms, release link, and source link without hardcoded
   release asset names.
8. **Footer** — manual, source, releases, application license, website attribution, and OGL legal
   links. It does not include placeholder community or roadmap links.

The approved storage/provider copy communicates only current behaviour:

- Chronacle stores source files, its search index, and notes on the user’s computer.
- Compatible online AI providers are the normal answer-generation setup.
- A configured online answer provider receives the question and the context Chronacle supplies for
  the answer. Depending on the selected campaign, sources, and available data, this can include
  relevant source excerpts; entity names, summaries, notes, and compiled Codex articles; player
  names and character class, level, and status; event start and end dates; session numbers, titles,
  played dates, and notes; and compiled rules. Some campaign entity and session context is supplied
  in full campaign scope rather than relevance-filtered.
- A configured remote embedding provider separately receives the searchable text and question or
  search text needed for indexing and retrieval, as applicable.
- Local models are supported as a secondary option.

The page does not claim that Chronacle will never offer accounts or hosted services. It also does not
imply per-source controls over what context is sent to an online provider.

## Manual experience

The approved “Reference desk” layout prioritises lookup speed:

- a sticky site header with manual search and locale switch;
- persistent section navigation on desktop;
- a drawer containing the same navigation on narrow screens;
- a focused reading column;
- a compact on-page outline on wide screens;
- breadcrumbs, previous/next links, and a route back to the section overview;
- styled notes, warnings, examples, procedures, tables, and code blocks;
- visible heading anchors and copy-link controls;
- task-oriented overview cards for common starting points.

Desktop navigation uses three visual columns: manual navigation, article, and on-page outline. The
article remains the dominant surface. Tablet widths remove the on-page outline before collapsing the
manual navigation. Mobile keeps a single reading column and opens navigation as a labelled drawer.

The layout uses semantic landmarks and preserves manual navigation and reading without JavaScript.

## Search

Pagefind builds a static index from rendered manual pages after the SvelteKit build. Landing and legal
pages are excluded. English and German indexes remain separate, and the current manual locale selects
the index.

Search opens from:

- the manual header;
- the manual overview;
- `Command + K` on macOS;
- `Control + K` elsewhere.

The search dialog includes:

- an autofocus search field;
- page title, section name, and a short highlighted excerpt per result;
- arrow-key selection;
- Enter to open the selected result;
- Escape to close;
- a focus trap and focus restoration;
- an accessible result-count announcement.

An empty query shows a short set of useful manual destinations. No results uses a plain localized
message and links to troubleshooting and the manual overview. If the search index cannot load, the
dialog explains that search is unavailable and preserves direct navigation links. The site never
sends queries to a server.

## Visual direction

The site follows the Chronacle “Arcane Terminal” system:

- cosmic blue-black grounds;
- electric blue and violet as controlled light accents;
- gem-white highlights and one restrained rune-gold accent;
- Cinzel for display titles and wordmark;
- Spectral for reading text;
- Hanken Grotesk for interface copy;
- JetBrains Mono for citations and game data;
- faint starfield, circuit, and aura textures from the approved asset set;
- low-opacity light borders, squircle radii, and deep-black plus arcane-glow elevation.

Glow remains emphasis currency. Primary actions, keyboard focus, and the example citation receive it;
ordinary cards do not. Motion is limited to a coordinated page entrance, subtle focus and hover
transitions, the search dialog, and mobile navigation. All motion respects `prefers-reduced-motion`.

The landing page is visually related to the product but not shaped like a generic SaaS template. It
uses one strong product window, asymmetric light and texture, generous space, and a compact feature
sequence. The manual is quieter and denser so long reading sessions remain comfortable.

## Voice and terminology

Copy is informal, economical, and factual. It addresses the reader as “you” where useful. It leads
with what a feature does and avoids claims about transforming a person’s life or table.

The site does not use:

- inflated language such as “revolutionary,” “game-changing,” or “unlock your potential”;
- fake testimonials, customer counts, or adoption claims;
- unnecessary protected game names or protected role names;
- emoji in product or manual chrome;
- claims that all processing stays local when an online AI provider is configured.

Third-party service names appear only where required to document supported integrations. Their use is
descriptive and is not expanded into marketing copy.

## SRD 5.1 and OGL 1.0a treatment

SRD 5.1 examples may be used when they make a Chronacle interaction concrete. They are confined to
clearly labelled example components and are not used as general brand language.

Every SRD-derived example is marked as Open Game Content in its visible caption. A dedicated legal
route contains:

- the complete, unmodified Open Game License Version 1.0a text;
- the exact Section 15 notices from the official SRD 5.1 OGL release;
- a clear statement identifying the marked SRD-derived example portions as Open Game Content;
- a link to the official SRD 5.1 OGL document.

The legal route is linked from every page footer and from each SRD example caption. Product Identity
listed by the official SRD is not used in those examples. German explanatory copy may surround an
example, but the OGL text and required copyright notice remain unmodified.

## Privacy and storage

The website stores no data in cookies, local storage, session storage, IndexedDB, or service-worker
caches. It includes no analytics, advertising pixels, third-party embeds, or consent-management
platform.

Browser language is read only to select the initial landing-page copy for the current page load.
Search is performed against static files in the browser. The website does not receive search terms or
record language choices.

## Error handling and validation

The content build fails for:

- invalid or incomplete frontmatter;
- duplicate public routes;
- duplicate translation keys within a locale;
- a missing English or German translation pair;
- unknown section identifiers;
- invalid section or page ordering;
- broken internal links;
- missing top-level article headings;
- an SRD-derived example without its Open Game Content marker.

Unknown manual routes render a localized not-found page containing search, a manual-overview link,
and a homepage link. The general site not-found page follows the landing-page browser language after
JavaScript starts and otherwise falls back to English.

If JavaScript is unavailable, the English landing page, all manual articles, breadcrumbs, section
navigation, previous/next navigation, and external links remain usable. In-place landing translation,
search, the mobile drawer, and small interaction enhancements require JavaScript. Mobile manual
navigation also exposes a no-JavaScript section index at the beginning of each article.

## Accessibility

- Text and controls target WCAG 2.2 AA contrast.
- All functionality is keyboard accessible.
- Focus is visible and never represented by glow alone.
- Search and mobile navigation trap and restore focus correctly.
- Dialog state and search result counts are announced.
- Heading order, landmarks, link purpose, and form labels are explicit.
- Decorative textures have no accessibility-tree representation.
- The wordmark and approved brand image have useful alternative text where needed.
- Touch targets remain at least 44 by 44 CSS pixels on narrow screens.
- Long German navigation labels wrap without truncating meaning.
- Reduced-motion users receive no entrance or drifting-background animation.

## Testing and verification

### Automated tests

- Unit tests cover route generation, locale helpers, translation pairing, content ordering, and
  browser-language selection without persistence.
- Component tests cover the locale switcher, search dialog, keyboard navigation, focus management,
  mobile navigation, article shell, and no-result/error states.
- Content validation tests cover every build-failure condition listed above.
- Static-build tests verify that all registered English and German routes are present in build output.
- Search-index tests verify language separation and representative title, heading, and body matches.
- Browser smoke tests cover the landing page, language switching, an English article, its paired
  German article, search, not-found handling, external actions, and responsive navigation.
- Automated accessibility checks run against the landing page, manual overview, representative long
  article, search dialog, and not-found page.

### Manual verification

- Review desktop, tablet, and narrow mobile widths.
- Test keyboard-only navigation from entry through search result selection.
- Test a reduced-motion environment.
- Inspect long German headings, navigation labels, code blocks, tables, and search excerpts.
- Confirm that the browser developer tools show no cookies or site storage created by the website.
- Confirm that no third-party requests occur during ordinary page load or search.
- Review English copy for tone and German copy for a clearly marked proofreading handoff.
- Compare every SRD example and the legal page with the official SRD 5.1 OGL source.

### Repository gates

The website package provides format, lint, type-check, unit-test, browser-test, and production-build
commands. The repository frontend quality script runs the website’s non-browser quality commands.
Before a pull request, the repository’s authoritative `scripts/ci/local-pr.sh` gate must pass.

## Completion criteria

The feature is complete when:

- `apps/website` builds a static English/German site;
- the landing page and approved manual layout match this design across target widths;
- every current manual topic has an accurate English and German page;
- language-scoped search works by mouse and keyboard;
- the website creates no client-side storage and loads no tracking or third-party embed code;
- SRD-derived examples and OGL attribution satisfy the treatment above;
- automated and manual verification passes;
- the repository frontend quality and local pull-request gates pass.
