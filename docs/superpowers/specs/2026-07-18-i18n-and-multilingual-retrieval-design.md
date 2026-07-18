# I18n, Multilingual Retrieval, and Shared Controls Design

## Purpose

Make Chronacle’s interface available in English, German, French, and Spanish;
allow the GM to choose the interface language; make the Oracle respond in the
language of the chat when that language is supported; and make retrieval
capable of indexing and referring to sources, entities, and compiled rules in
the supported languages. Consolidate repeated UI controls into accessible,
Chronacle-branded primitives as part of the work.

## Scope

- Translate every user-visible frontend string owned by Chronacle into `en`,
  `de`, `fr`, and `es`.
- Select the interface locale from a persisted Settings override, falling back
  to the operating-system locale and then English.
- Add an explicit embedding-mode choice: small local English Nomic, larger
  local multilingual E5 Base, or the existing cloud embedding configuration.
- Make every Oracle turn choose a response language independently: a supported
  language detected from the current user message wins; otherwise use the
  resolved interface locale.
- Preserve source passages, entity names, compiled rules, and citations in
  their original language. Retrieval searches every subscribed collection; it
  does not filter by source language.
- Introduce shared `Button`, `ProgressBar`, `FormField`, `Dialog`, and
  `StatusBadge` components, replacing only genuinely repeated patterns.

Out of scope: translating user-authored content, PDF text, LLM-generated
entity/rule content, or dynamically returned provider/server error details.

## I18n architecture

`apps/desktop/src/lib/i18n/` owns a typed source catalog and one complete
catalog per supported locale. Translation keys represent UI intent rather than
English text. The formatter supports named interpolation and locale-aware
number/date formatting through the existing locale helper.

At startup, the locale service normalizes Tauri OS locale / `navigator.language`
to `en`, `de`, `fr`, or `es`; unsupported locales use English. It then loads
the `ui_locale` setting. `auto` preserves the OS choice, while an explicit
setting overrides it immediately and persistently. Components consume a
reactive `t(key, values?)` API, so changing the setting updates the mounted
interface without a restart.

Tests must enforce catalog completeness and validate fallback, interpolation,
and reactive locale selection. Component tests should assert rendered localized
copy rather than catalog internals.

## Embedding modes and index integrity

The Settings screen presents three mutually exclusive modes:

| Choice | Provider/model | Language capability | Notes |
| --- | --- | --- | --- |
| Small local | `nomic-embed-text-v1.5` | English-focused | Current lightweight on-device default. |
| Multilingual local | `multilingual-e5-base` | German, French, Spanish, and cross-language retrieval | Larger on-device download, 768 dimensions. |
| Cloud | Existing OpenAI-compatible embedding setup | Model-provider dependent; document that the default OpenAI v3 model is multilingual | Requires credentials/network. |

The selected mode and model identity are persisted. The embedding-provider
factory constructs only the selected backend/model. The setup/download UI
downloads the selected local model and reports its own progress.

Changing mode or model never mixes vectors. Existing `embed_model` identity
checks surface a stale-index state; the GM must explicitly re-index. The design
continues to use a 768-dimensional vector schema, which allows the Nomic and
multilingual E5 Base paths to share the current storage shape. Cloud dimensions
must remain validated against that schema before the provider is accepted.

This changes the approved embedding-model surface and therefore requires an
ADR plus an architecture-document update before implementation.

## Oracle response-language policy

The frontend resolves a `responseLanguage` BCP-47 base language for each send:

1. Detect whether the current message is unambiguously German, French, Spanish,
   or English.
2. If it is, use that language.
3. Otherwise, use the resolved UI locale.

The Tauri request passes this value to the retrieval service. Prompt assembly
adds a concise response-language instruction without translating retrieved
evidence. The instruction explicitly keeps source and entity names exact and
requires the existing citation marker syntax unchanged. This means, for
example, a French question may receive a French answer backed by a German PDF,
with the German quoted citation preserved.

If local language identification cannot distinguish a short/ambiguous message,
the setting is the deterministic fallback. The supported set is deliberately
limited to the four shipped UI languages in this phase.

## Shared controls

The components live under `apps/desktop/src/components/ui/` and use existing
Chronacle tokens: dark raised surfaces, low-opacity arcane hairlines, visible
arcane focus, restrained primary glow, squircle radii, and reduced-motion
support.

- `Button` supplies primary, secondary, ghost, danger, and icon-only variants;
  native button semantics; loading/disabled state; and optional leading/trailing
  snippets. Icon-only instances require an accessible label.
- `ProgressBar` exposes determinate progress with `role=progressbar`, min/max/
  current values, optional localized label and percentage, and shared visual
  treatment.
- `FormField` composes a label, optional help, validation text, and control
  content so settings/editor fields retain correct label association.
- `Dialog` provides the existing modal focus behavior, accessible naming, a
  title/body region, and `DialogActions` for consistent cancel/confirm layout.
- `StatusBadge` normalizes info, success, warning, and danger states with
  text—not color alone—for provider, indexing, and maintenance status.

Migration deliberately excludes interaction-specific elements: tabs, entity
links, citation chips, chat-composer tools, and list-row buttons keep their
local markup/styles.

## Data flow

```text
OS locale ──> locale service ──> UI translation + fallback language
                    ▲                    │
ui_locale setting ──┘                    ▼
current chat message ──> language resolver ──> chat_send ──> prompt

embedding mode setting ──> embedding factory ──> model identity ──> explicit re-index
PDFs/entities/rules (original language) ────────────────────────> shared vector search
```

## Acceptance criteria

- With `auto` selected, a supported OS locale renders the matching UI; an
  unsupported one renders English.
- Choosing German, French, or Spanish in Settings changes the full UI without
  restart and survives relaunch.
- All catalogs contain all source keys and interpolate localized values safely.
- A German, French, Spanish, or English Oracle message receives a reply in its
  own language; an ambiguous message receives the Settings language.
- Cross-language sources can be found and cited when using the multilingual
  local or supported cloud embedding mode.
- Switching embedding modes requires explicit re-indexing before retrieval
  resumes with the new vectors.
- Upload, model download, and re-index use the shared accessible progress bar;
  repeated actions/forms/dialogs use the new primitives without visual regressions.
- New behavior is captured in BDD feature scenarios and covered with Rust,
  frontend, and backend Playwright tests.

## Risks and mitigations

- Language identification can be unreliable for one-word messages: use the
  explicit Settings language fallback rather than guessing.
- A cloud provider can expose a non-768-dimensional embedding model: validate
  provider dimensions before accepting the configuration and retain the current
  re-index safety flow.
- Translation catalogs can drift as the UI changes: a completeness test makes
  missing translations a test failure.
- A broad component rewrite can regress special interactions: migrate only the
  repeated patterns listed above, in focused batches with component tests.
