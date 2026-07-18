# I18n, Multilingual Retrieval, and Shared Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship English, German, French, and Spanish UI translation; explicit local/cloud embedding choices; per-message Oracle response language; and reusable accessible Chronacle UI controls.

**Architecture:** A frontend i18n service owns normalized locale selection, typed catalogs, interpolation, and reactive translation. Rust owns embedding-mode construction and the response-language instruction passed into the RAG prompt. The selected embedding model remains the persisted index identity, so changing it reuses the existing explicit stale-index/re-index workflow. Shared Svelte controls centralize repeated visual and accessibility behavior without abstracting interaction-specific controls.

**Tech Stack:** Svelte 5 runes, TypeScript, Vitest and Testing Library, Tauri IPC/events, Rust, fastembed 5, SurrealDB, Playwright-BDD.

---

## Files and responsibilities

- `docs/architecture.md` — amend ADR-003’s approved local-model decision and settings keys.
- `docs/user-guide.md` — explain interface language, Oracle response-language precedence, and embedding-mode trade-offs.
- `apps/desktop/src/lib/i18n/*` — locale normalization, detection, typed message catalogs, and formatter.
- `apps/desktop/src/lib/locale.svelte.ts` — resolve OS locale plus persisted override, retaining number/date formatting.
- `apps/desktop/src/components/ui/*` — `Button`, `ProgressBar`, `FormField`, `Dialog`, and `StatusBadge` primitives.
- `apps/desktop/src/{App,ModelDownload,UploadProgress}.svelte`, `shell/*.svelte`, `views/*.svelte`, and `components/*.svelte` — consume translations and primitives for recurring controls.
- `apps/desktop/src/views/SettingsView.svelte` — UI-language and embedding-mode selection.
- `apps/desktop/src/lib/commands.ts`, `apps/desktop/src/views/OracleView.svelte`, `apps/desktop/src-tauri/src/commands/chat_commands.rs`, and `crates/chronacle-retrieval/src/agent_service/*` — per-turn response language.
- `crates/chronacle-providers/src/embedding/local.rs` and desktop embedding commands — selectable Nomic/E5 local model lifecycle.
- matching `*.test.ts`, Rust unit/integration tests, and a new BDD feature/steps — regression coverage.

### Task 1: Document the embedding choice and acceptance behavior

**Files:**
- Modify: `docs/architecture.md:79-141, 840-845`
- Create: `apps/desktop/tests/e2e/features/i18n-multilingual.feature`

- [ ] **Step 1: Add Gherkin acceptance scenarios before code**

```gherkin
Feature: Interface and Oracle language selection
  Scenario: The saved interface language overrides the operating system
    Given the operating system locale is "en-US"
    And the saved interface language is "de"
    When Chronacle opens Settings
    Then the Settings heading is "Einstellungen"

  Scenario: A supported message language takes precedence for Oracle
    Given the saved interface language is "de"
    When I ask Oracle "Quelle est la règle pour le grappin ?"
    Then the Oracle request response language is "fr"

  Scenario: Switching embedding modes requires re-indexing
    Given sources were indexed with "nomic-embed-text-v1.5"
    When I select the local multilingual embedding mode
    Then Chronacle shows that source embeddings require re-indexing
```

- [ ] **Step 2: Run the generated BDD suite to record the missing steps**

Run: `pnpm -C apps/desktop exec bddgen && pnpm -C apps/desktop exec playwright test tests/e2e/backend --grep "Interface and Oracle language selection"`

Expected: FAIL because the new feature has no matching steps.

- [ ] **Step 3: Amend ADR-003 and the setting-key list**

Replace the single local-model decision with these explicitly supported choices:

```markdown
| Local small | `nomic-embed-text-v1.5` | English-focused, 768 dimensions |
| Local multilingual | `multilingual-e5-base` | offline German/French/Spanish retrieval, 768 dimensions |
| Cloud | configured OpenAI-compatible model | must produce 768 dimensions |
```

Document `embedding_mode` (`local_nomic` | `local_multilingual` | `cloud`) and `ui_locale` (`auto` | `en` | `de` | `fr` | `es`), and state that changing model identity requires explicit re-indexing.

- [ ] **Step 4: Check documentation and feature formatting**

Run: `pnpm -C apps/desktop exec prettier --check ../../docs/architecture.md tests/e2e/features/i18n-multilingual.feature`

Expected: PASS.

- [ ] **Step 5: Commit the specification contract**

```bash
git add docs/architecture.md apps/desktop/tests/e2e/features/i18n-multilingual.feature
git commit -m "docs: define multilingual embedding choices"
```

### Task 2: Build and prove the typed i18n core

**Files:**
- Create: `apps/desktop/src/lib/i18n/types.ts`
- Create: `apps/desktop/src/lib/i18n/messages.ts`
- Create: `apps/desktop/src/lib/i18n/locales/en.ts`
- Create: `apps/desktop/src/lib/i18n/locales/de.ts`
- Create: `apps/desktop/src/lib/i18n/locales/fr.ts`
- Create: `apps/desktop/src/lib/i18n/locales/es.ts`
- Create: `apps/desktop/src/lib/i18n/index.svelte.ts`
- Test: `apps/desktop/src/lib/i18n/index.test.ts`

- [ ] **Step 1: Write the failing formatter/locale tests**

```ts
import { describe, expect, it } from 'vitest';
import { createI18n, normalizeLocale } from './index.svelte';

describe('normalizeLocale', () => {
  it.each([['de-DE', 'de'], ['fr-CA', 'fr'], ['es-MX', 'es'], ['it-IT', 'en']])(
    'normalizes %s to %s',
    (input, expected) => expect(normalizeLocale(input)).toBe(expected),
  );
});

it('switches catalogs reactively and interpolates named values', () => {
  const i18n = createI18n('en');
  expect(i18n.t('progress.source', { current: 1, total: 3 })).toBe('Source 1/3');
  i18n.setLocale('de');
  expect(i18n.t('progress.source', { current: 1, total: 3 })).toBe('Quelle 1/3');
});

it('keeps every shipped catalog complete', () => {
  expect(createI18n('en').missingKeys()).toEqual([]);
  expect(createI18n('de').missingKeys()).toEqual([]);
  expect(createI18n('fr').missingKeys()).toEqual([]);
  expect(createI18n('es').missingKeys()).toEqual([]);
});
```

- [ ] **Step 2: Run the tests and verify red**

Run: `pnpm -C apps/desktop test:run src/lib/i18n/index.test.ts`

Expected: FAIL because the i18n module does not exist.

- [ ] **Step 3: Implement the minimal typed catalog API**

Use English as the `MessageCatalog` source type. Define all UI keys once, including plural-free interpolated entries such as `progress.source`, `progress.percent`, `status.reindexed`, and `error.reindexFailed`. Implement `t()` as a `{name}` replacement formatter, returning the English source only for an absent key. Export `SUPPORTED_LOCALES`, `normalizeLocale`, `createI18n`, and a singleton used by components.

```ts
export type SupportedLocale = 'en' | 'de' | 'fr' | 'es';
export function normalizeLocale(value: string | null | undefined): SupportedLocale {
  const language = value?.toLowerCase().split('-')[0];
  return language === 'de' || language === 'fr' || language === 'es' ? language : 'en';
}
```

Translate every value, not just headings; keep tokens such as model IDs, citation markers, and source/entity names out of catalogs.

- [ ] **Step 4: Run the i18n tests green**

Run: `pnpm -C apps/desktop test:run src/lib/i18n/index.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit the i18n core**

```bash
git add apps/desktop/src/lib/i18n
git commit -m "feat: add typed UI translation catalogs"
```

### Task 3: Persist OS-default language and Settings override

**Files:**
- Modify: `apps/desktop/src/lib/locale.svelte.ts`
- Modify: `apps/desktop/src/App.svelte`
- Modify: `apps/desktop/src/views/SettingsView.svelte`
- Modify: `apps/desktop/src/views/SettingsView.test.ts`
- Modify: `apps/desktop/src/test-setup.ts`

- [ ] **Step 1: Add a failing Settings test for the override**

```ts
it('persists an explicit UI language and updates rendered copy', async () => {
  mockGetSettings.mockResolvedValue({ ui_locale: 'fr' });
  render(SettingsView);
  expect(await screen.findByRole('heading', { name: 'Paramètres' })).toBeInTheDocument();
  await userEvent.selectOptions(screen.getByLabelText('Langue de l’interface'), 'de');
  await userEvent.click(screen.getByRole('button', { name: 'Enregistrer les paramètres' }));
  expect(mockUpdateSetting).toHaveBeenCalledWith('ui_locale', 'de');
});
```

- [ ] **Step 2: Run the focused test and verify red**

Run: `pnpm -C apps/desktop test:run src/views/SettingsView.test.ts`

Expected: FAIL because no language selector exists.

- [ ] **Step 3: Implement locale initialization and selector**

`initLocale()` must first normalize the OS/Tauri locale, then read `getSettings()['ui_locale']`; `auto` retains the OS result and an explicit valid value calls the i18n singleton’s `setLocale`. Add a translated `FormField`/select in Settings with `auto`, English, Deutsch, Français, and Español choices. Saving this setting updates the locale immediately and calls `updateSetting('ui_locale', value)`.

- [ ] **Step 4: Extend Tauri test mocks before running components**

In `test-setup.ts`, mock `plugin:os|locale`, `plugin:event|listen`, `plugin:event|unlisten`, `window.__TAURI_INTERNALS__.transformCallback`, and `window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener` so locale/event paths do not throw after assertions.

- [ ] **Step 5: Run locale and Settings tests green**

Run: `pnpm -C apps/desktop test:run src/lib/i18n/index.test.ts src/views/SettingsView.test.ts src/App.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit locale persistence**

```bash
git add apps/desktop/src/lib/locale.svelte.ts apps/desktop/src/App.svelte apps/desktop/src/views/SettingsView.svelte apps/desktop/src/views/SettingsView.test.ts apps/desktop/src/test-setup.ts
git commit -m "feat: add saved interface language selection"
```

### Task 4: Introduce shared control primitives test-first

**Files:**
- Create: `apps/desktop/src/components/ui/Button.svelte`
- Create: `apps/desktop/src/components/ui/ProgressBar.svelte`
- Create: `apps/desktop/src/components/ui/FormField.svelte`
- Create: `apps/desktop/src/components/ui/Dialog.svelte`
- Create: `apps/desktop/src/components/ui/StatusBadge.svelte`
- Test: `apps/desktop/src/components/ui/Button.test.ts`
- Test: `apps/desktop/src/components/ui/ProgressBar.test.ts`
- Test: `apps/desktop/src/components/ui/Dialog.test.ts`

- [ ] **Step 1: Write failing semantic-control tests**

```ts
it('renders a disabled loading button without invoking its click handler', async () => {
  const onclick = vi.fn();
  render(Button, { props: { loading: true, onclick, ariaLabel: 'Save' } });
  const button = screen.getByRole('button', { name: 'Saving…' });
  expect(button).toBeDisabled();
  await userEvent.click(button);
  expect(onclick).not.toHaveBeenCalled();
});

it('exposes determinate progress to assistive technology', () => {
  render(ProgressBar, { props: { value: 62, label: 'Indexing' } });
  expect(screen.getByRole('progressbar', { name: 'Indexing' })).toHaveAttribute('aria-valuenow', '62');
});
```

- [ ] **Step 2: Run the component tests and verify red**

Run: `pnpm -C apps/desktop test:run src/components/ui/Button.test.ts src/components/ui/ProgressBar.test.ts src/components/ui/Dialog.test.ts`

Expected: FAIL because the controls do not exist.

- [ ] **Step 3: Implement the controls using Svelte 5 snippets and tokens**

`Button` accepts `variant`, `loading`, `iconOnly`, `ariaLabel`, `disabled`, `onclick`, and `children`; only icon-only calls may omit text, and they must provide `ariaLabel`. `ProgressBar` clamps values 0–100 and renders a localized percentage with `role=progressbar`. `Dialog` wraps the existing `modal` action, receives title/body/action snippets, and renders a labelled `role=dialog`. `FormField` creates the label/control/help/error association. `StatusBadge` pairs each semantic color with status text.

- [ ] **Step 4: Run the primitive tests green**

Run: `pnpm -C apps/desktop test:run src/components/ui/Button.test.ts src/components/ui/ProgressBar.test.ts src/components/ui/Dialog.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit shared controls**

```bash
git add apps/desktop/src/components/ui
git commit -m "feat: add shared accessible UI controls"
```

### Task 5: Migrate repeated controls and translate all frontend surfaces

**Files:**
- Modify: `apps/desktop/src/ModelDownload.svelte`, `apps/desktop/src/UploadProgress.svelte`
- Modify: `apps/desktop/src/shell/CampaignRail.svelte`, `CampaignSwitcher.svelte`, `Shell.svelte`
- Modify: `apps/desktop/src/views/CampaignView.svelte`, `MaintenanceView.svelte`, `OracleView.svelte`, `SessionLogView.svelte`, `SettingsView.svelte`, `TimelineView.svelte`
- Modify: `apps/desktop/src/components/AliasField.svelte`, `EntityForm.svelte`, `EntityGraph.svelte`, `EntityManager.svelte`, `ExtractionCard.svelte`, `MergeDialog.svelte`, `RulesPanel.svelte`, `RulingCard.svelte`, `SessionRow.svelte`, `Toast.svelte`, `VaultSyncSettings.svelte`, `WikiLinkEditor.svelte`
- Modify tests: each existing adjacent `*.test.ts`
- Create: `apps/desktop/src/ModelDownload.test.ts`

- [ ] **Step 1: Add failing migration tests for the three progress paths**

```ts
it('announces upload progress through the shared progressbar', () => {
  render(UploadProgress, { props: { status: 'Indexing', progress: 42, onDismiss: vi.fn() } });
  expect(screen.getByRole('progressbar', { name: 'Indexing' })).toHaveAttribute('aria-valuenow', '42');
});
```

Add equivalent tests for ModelDownload and Settings re-indexing.

- [ ] **Step 2: Verify red before migration**

Run: `pnpm -C apps/desktop test:run src/UploadProgress.test.ts src/views/SettingsView.test.ts`

Expected: FAIL because current progress markup has no shared progressbar semantics.

- [ ] **Step 3: Replace only repeated patterns**

Replace all hard-coded UI labels, placeholders, titles, aria labels, empty states, loading copy, and static toast/status copy in the listed files with `t(...)`. Migrate primary/secondary/ghost/danger/icon buttons where an existing `<button>` is a regular action. Migrate upload, model-download, shell mismatch, and Settings re-index determinate bars to `ProgressBar`; use `FormField` in Settings and entity forms; use `Dialog`/`DialogActions` in merge/delete/confirmation flows; use `StatusBadge` for provider/index/maintenance status. Preserve local tabs, entity/citation links, composer tools, and list-row buttons.

- [ ] **Step 4: Run component and view regressions green**

Run: `pnpm -C apps/desktop test:run`

Expected: PASS with no untranslated test expectations left in the rendered UI assertions.

- [ ] **Step 5: Run static Svelte analysis and formatting**

Run: `npx @sveltejs/mcp svelte-autofixer apps/desktop/src/components/ui/Button.svelte --async && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint`

Expected: no autofixer findings, typecheck PASS, lint PASS.

- [ ] **Step 6: Commit the frontend migration**

```bash
git add apps/desktop/src
git commit -m "feat: localize interface and reuse controls"
```

### Task 6: Add selectable local embedding models without weakening index safety

**Files:**
- Modify: `crates/chronacle-providers/src/embedding/local.rs`
- Modify: `crates/chronacle-providers/src/embedding/mod.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/src/commands/embedding_commands.rs`
- Modify: `apps/desktop/src/ModelDownload.svelte`
- Modify: `apps/desktop/src/views/SettingsView.svelte`
- Test: `crates/chronacle-providers/src/embedding/embedding_tests.rs`
- Test: `apps/desktop/src-tauri/src/commands/embedding_commands.rs` tests
- Test: `apps/desktop/src/views/SettingsView.test.ts`

- [ ] **Step 1: Write failing Rust model-selection tests**

```rust
#[test]
fn local_mode_metadata_keeps_supported_models_at_768_dimensions() {
    assert_eq!(LocalEmbeddingMode::Nomic.model_name(), "nomic-embed-text-v1.5");
    assert_eq!(LocalEmbeddingMode::Nomic.dimension(), 768);
    assert_eq!(LocalEmbeddingMode::MultilingualE5Base.model_name(), "multilingual-e5-base");
    assert_eq!(LocalEmbeddingMode::MultilingualE5Base.dimension(), 768);
}
```

- [ ] **Step 2: Verify red**

Run: `cargo test -p chronacle-providers local_mode_metadata_keeps_supported_models_at_768_dimensions`

Expected: FAIL because `LocalEmbeddingMode` does not exist.

- [ ] **Step 3: Implement model-specific construction and download**

Introduce `LocalEmbeddingMode::{Nomic, MultilingualE5Base}` and a single `FastEmbedProvider::try_new(mode, cache_dir)` that uses `EmbeddingModel::NomicEmbedTextV15` or `EmbeddingModel::MultilingualE5Base`. Apply Nomic’s asymmetric prefixes only in Nomic mode; apply E5’s `passage: ` and `query: ` prefixes in E5 mode. Store distinct cache paths/model identities. Update the embedding factory so `embedding_mode` selects Nomic, E5, or the existing cloud provider, and reject a provider whose `dimension()` is not 768.

- [ ] **Step 4: Add a failing Settings test for mode persistence**

```ts
it('saves the multilingual local mode and tells the user to re-index', async () => {
  render(SettingsView);
  await userEvent.selectOptions(screen.getByLabelText('Embedding mode'), 'local_multilingual');
  await userEvent.click(screen.getByRole('button', { name: 'Save embedding provider' }));
  expect(mockUpdateSetting).toHaveBeenCalledWith('embedding_mode', 'local_multilingual');
  expect(await screen.findByText(/Re-index existing sources/)).toBeVisible();
});
```

- [ ] **Step 5: Run red and implement UI/download integration**

Run: `pnpm -C apps/desktop test:run src/views/SettingsView.test.ts`

Expected: FAIL until the mode selector and save behavior exist.

Use translated option cards/select entries to explain local size/offline/language tradeoffs. Make ModelDownload request/download the selected local model and render its localized name/progress through `ProgressBar`. Keep `embedding_backend` compatible as a migration fallback, but write the new canonical `embedding_mode` on save.

- [ ] **Step 6: Verify model selection and index mismatch behavior**

Run: `cargo test -p chronacle-providers embedding && cargo test -p Chronacle embedding && pnpm -C apps/desktop test:run src/views/SettingsView.test.ts src/ModelDownload.test.ts`

Expected: PASS; existing model-mismatch tests still prove re-indexing is explicit.

- [ ] **Step 7: Commit embedding-mode support**

```bash
git add crates/chronacle-providers apps/desktop/src-tauri apps/desktop/src/ModelDownload.svelte apps/desktop/src/views/SettingsView.svelte apps/desktop/src/views/SettingsView.test.ts
git commit -m "feat: add multilingual local embedding mode"
```

### Task 7: Carry resolved response language through Oracle and prompt assembly

**Files:**
- Create: `apps/desktop/src/lib/i18n/detect-language.ts`
- Test: `apps/desktop/src/lib/i18n/detect-language.test.ts`
- Modify: `apps/desktop/src/lib/commands.ts`
- Modify: `apps/desktop/src/views/OracleView.svelte`
- Modify: `apps/desktop/src/views/OracleView.test.ts`
- Modify: `apps/desktop/src-tauri/src/commands/chat_commands.rs`
- Modify: `crates/chronacle-retrieval/src/agent_service/mod.rs`
- Modify: `crates/chronacle-retrieval/src/agent_service/prompt.rs`
- Test: `crates/chronacle-retrieval/src/agent_service/prompt.rs`

- [ ] **Step 1: Write failing language precedence tests**

```ts
it.each([
  ['Quelle est la règle ?', 'de', 'fr'],
  ['Wie funktioniert Grappling?', 'fr', 'de'],
  ['grapple?', 'es', 'es'],
])('uses message language when detected, otherwise the fallback', (message, fallback, expected) => {
  expect(resolveResponseLanguage(message, fallback)).toBe(expected);
});
```

- [ ] **Step 2: Verify red**

Run: `pnpm -C apps/desktop test:run src/lib/i18n/detect-language.test.ts`

Expected: FAIL because the resolver does not exist.

- [ ] **Step 3: Implement deterministic supported-language detection**

Use a small offline detector that returns only `en`, `de`, `fr`, `es`, or `null`; reject low-confidence/short results so `resolveResponseLanguage` returns the UI locale. Do not add a dependency. Update `chatSend(message, campaignId, responseLanguage)` to send `responseLanguage` inside the request, and resolve it at the Oracle send boundary.

- [ ] **Step 4: Add a failing Rust prompt test**

```rust
#[test]
fn system_prompt_requires_the_resolved_response_language() {
    let prompt = build_system_prompt("", "", "", "fr");
    assert!(prompt.contains("Respond in French"));
    assert!(prompt.contains("keep source and entity names exact"));
}
```

- [ ] **Step 5: Verify red and implement the IPC/prompt path**

Run: `cargo test -p chronacle-retrieval system_prompt_requires_the_resolved_response_language`

Expected: FAIL because prompt assembly does not accept response language.

Extend `ChatRequest`, `stream_response`, and `build_system_prompt` with `response_language`. Add a prompt instruction that answers in the selected language, preserves original-language evidence/names, and retains exact `[Source: ...]` / `[Entity: ...]` citation syntax. Default missing IPC values to English for compatibility with older callers and tests.

- [ ] **Step 6: Run response-language tests green**

Run: `pnpm -C apps/desktop test:run src/lib/i18n/detect-language.test.ts src/views/OracleView.test.ts && cargo test -p chronacle-retrieval prompt && cargo test -p Chronacle chat`

Expected: PASS.

- [ ] **Step 7: Commit Oracle language behavior**

```bash
git add apps/desktop/src/lib/i18n apps/desktop/src/lib/commands.ts apps/desktop/src/views/OracleView.svelte apps/desktop/src/views/OracleView.test.ts apps/desktop/src-tauri/src/commands/chat_commands.rs crates/chronacle-retrieval
git commit -m "feat: answer Oracle turns in the selected language"
```

### Task 8: Finish BDD steps and complete verification

**Files:**
- Modify: `docs/user-guide.md`
- Modify: `apps/desktop/tests/e2e/backend/steps/*.ts` (the settings/chat step module selected by existing feature conventions)
- Modify: `apps/desktop/tests/e2e/features/i18n-multilingual.feature`
- Modify: `apps/desktop/src-tauri/tests/integration_test.rs` if the existing chat IPC harness owns request fixtures

- [ ] **Step 1: Implement feature steps against real backend contracts**

Add steps that seed `ui_locale`, inspect the `chat_send` request’s `responseLanguage`, and seed sources with an old `embed_model` before selecting E5. Assert the existing mismatch/re-index response, not a new duplicate mechanism.

- [ ] **Step 2: Amend the GM-facing user guide**

Add a concise “Language and search” subsection to the Settings guidance. It must state:

- `Automatic` follows the operating-system language; English, German, French, and Spanish can be selected explicitly and take effect immediately.
- Oracle replies in a clearly detected English, German, French, or Spanish user message; short or ambiguous messages use the interface-language setting.
- PDFs, entity names, rules, and cited quotes remain in their original language; multilingual and cloud embeddings can retrieve across the supported languages.
- Small local Nomic prioritizes a smaller offline English index; local multilingual E5 Base is a larger offline download for multilingual/cross-language retrieval; cloud embeddings require provider credentials.
- Changing any embedding model requires the GM to re-index sources from Settings before those sources use the new model.

- [ ] **Step 3: Run the feature until green**

Run: `scripts/ci/acceptance.sh`

Expected: PASS, including the new language scenarios.

- [ ] **Step 4: Run focused quality gates**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && cargo deny check && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run`

Expected: all commands PASS.

- [ ] **Step 5: Run the authoritative local PR gate**

Run: `scripts/ci/local-pr.sh`

Expected: PASS.

- [ ] **Step 6: Commit the acceptance coverage and user guide**

```bash
git add docs/user-guide.md apps/desktop/tests/e2e apps/desktop/src-tauri/tests
git commit -m "docs: explain multilingual Chronacle behavior"
```
