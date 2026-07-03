# Codex Tranche 1 (A0, A1b, A2a, A2b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the foundations of the Codex compiled world model: executable BDD tooling (A0), the two-mode campaign-delete UI (A1b), the Codex schema (A2a), and staleness producers + reference-scope validation (A2b).

**Architecture:** Follows `docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md`. Four independent-ish PRs on non-tracking feature branches: A0 adds `playwright-bdd` acceptance tooling; A1b finishes ADR-010's delete UI; A2a extends `002_wiki_layer.surql` additively; A2b adds a `codex_service` skeleton in `chronacle-extraction`, staleness marking in three producers, and scope validation in `entity_service::relate`.

**Tech Stack:** Rust (SurrealDB embedded, tokio, thiserror), Svelte 5 runes + TypeScript, Playwright + playwright-bdd + @cucumber/cucumber, Vitest.

## Global Constraints

- Every branch: `git checkout --no-track -b <branch> main` — **never track main** (accidental-push protection, user requirement).
- No new Rust crates or `Cargo.toml` entries anywhere in this tranche (repo hard constraint; the only new deps are two npm devDependencies in A0, covered by ADR-011).
- Commit subjects ≤ 72 chars, imperative mood, conventional prefixes; never `--no-verify` (lefthook runs rustfmt/clippy/prettier/eslint).
- All schema statements `DEFINE … OVERWRITE` — migrations re-run on every boot; the file must be pure re-runnable DDL.
- Clippy warnings are errors: `cargo clippy --workspace --all-targets --all-features -- -D warnings` must be clean.
- Svelte 5 runes only (`$state`, `$derived`, `$props`); no `export let` / `$:`. Prettier/ESLint config is authoritative.
- BDD (ADR-011): UI-reachable acceptance scenarios ship as `.feature` files run by `playwright-bdd`; backend-only scenarios ship as Rust integration tests whose names mirror the Gherkin (convention documented in `apps/desktop/tests/e2e/features/README.md`, created in Task 3).
- KNN/scope SurrealQL: never combine MTREE KNN with `id IN (SELECT …)` (silently returns 0 rows). Not exercised in this tranche, but do not introduce it.
- Frontend `invoke()` argument keys are camelCase (`onOwnedCollection`) — Tauri maps them to snake_case Rust parameters automatically (existing convention, see `collectionId` in `commands.ts`).
- Tests use in-memory SurrealDB (`surrealdb::engine::local::Mem`) + `chronacle_db::run_migrations`; mock LLM/embedding via existing `Mock*` providers.
- Each PR ends with: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run` all green before push.

---

# PR A0 — `chore/a0-bdd-tooling`

Executable BDD harness (ADR-011): `playwright-bdd` + `@cucumber/cucumber`, shared IPC mock, smoke feature, CI wiring.

### Task 1: Branch, dependencies, Playwright BDD config

**Files:**
- Modify: `apps/desktop/package.json` (via pnpm; plus one script by hand)
- Modify: `apps/desktop/playwright.config.ts`
- Modify: `.gitignore`

**Interfaces:**
- Produces: Playwright projects `backend` (existing specs) and `bdd` (generated from features); npm script `e2e:backend` = `bddgen && playwright test`; gitignored `apps/desktop/tests/e2e/.features-gen/`.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b chore/a0-bdd-tooling main
```

- [ ] **Step 2: Add devDependencies (tool-managed, never hand-edit the lockfile)**

```bash
pnpm -C apps/desktop add -D playwright-bdd @cucumber/cucumber
```

Expected: `package.json` devDependencies gain both entries; `pnpm-lock.yaml` updates.

- [ ] **Step 3: Replace `apps/desktop/playwright.config.ts` with the BDD-aware config**

```ts
import { defineConfig } from '@playwright/test';
import { defineBddConfig } from 'playwright-bdd';

const bddTestDir = defineBddConfig({
  features: 'tests/e2e/features/**/*.feature',
  steps: 'tests/e2e/backend/steps/**/*.ts',
  outputDir: 'tests/e2e/.features-gen',
});

export default defineConfig({
  timeout: 30000,
  retries: 0,
  use: {
    baseURL: 'http://localhost:1420',
    headless: true,
  },
  webServer: {
    command: 'pnpm dev',
    port: 1420,
    reuseExistingServer: true,
    timeout: 15000,
  },
  projects: [
    // Existing hand-written backend specs (mocked-IPC frontend suite).
    { name: 'backend', testDir: './tests/e2e/backend' },
    // Generated from tests/e2e/features/*.feature by `bddgen` (ADR-011).
    { name: 'bdd', testDir: bddTestDir },
  ],
});
```

Note: the UI E2E suite is `.mjs` driven by mocha and never matches Playwright's `testMatch`, so no exclusion is needed.

- [ ] **Step 4: Add the npm script**

In `apps/desktop/package.json` `"scripts"`, after `"test:coverage"`:

```json
    "e2e:backend": "bddgen && playwright test",
```

- [ ] **Step 5: Gitignore the generated tests**

In the root `.gitignore`, under the `# ── Node ──` section, add:

```
apps/desktop/tests/e2e/.features-gen/
```

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/package.json pnpm-lock.yaml apps/desktop/playwright.config.ts .gitignore
git commit -m "chore(e2e): add playwright-bdd + cucumber tooling (ADR-011)"
```

### Task 2: Shared IPC mock helper with call recording

**Files:**
- Create: `apps/desktop/tests/e2e/backend/ipc-mock.ts`
- Modify: `apps/desktop/tests/e2e/backend/chronacle.spec.ts:4-101` (replace inline `beforeEach` mock with the helper)

**Interfaces:**
- Produces: `installIpcMock(page: Page): Promise<void>` — installs the Tauri IPC mock and records every call into `window.__ipcCalls` (array of `{ cmd, args }`), readable via `page.evaluate`. Used by both hand-written specs and BDD step fixtures (Task 3, Task 9).

- [ ] **Step 1: Create `apps/desktop/tests/e2e/backend/ipc-mock.ts`**

The switch body is moved verbatim from `chronacle.spec.ts` with two additions: call recording and a `delete_campaign` case.

```ts
import type { Page } from '@playwright/test';

/**
 * Install the Tauri IPC mock into the page before app scripts run.
 *
 * We mock window.__TAURI_INTERNALS__.invoke directly instead of importing
 * mockIPC from @tauri-apps/api/mocks, because addInitScript runs in the
 * browser context where the module isn't available.
 *
 * Every invoke() is recorded into window.__ipcCalls so tests and BDD steps
 * can assert which commands were (not) sent.
 */
export async function installIpcMock(page: Page): Promise<void> {
  await page.addInitScript(() => {
    let _cbId = 0;
    // @ts-expect-error -- injected by Tauri at runtime
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    // @ts-expect-error -- test-only call log
    window.__ipcCalls = [] as Array<{ cmd: string; args?: Record<string, unknown> }>;
    // @ts-expect-error -- injected by Tauri at runtime
    window.__TAURI_INTERNALS__ = {
      transformCallback: (_cb: unknown, _once?: boolean) => ++_cbId,
      invoke: (cmd: string, args?: Record<string, unknown>) => {
        // @ts-expect-error -- test-only call log
        window.__ipcCalls.push({ cmd, args });
        switch (cmd) {
          case 'plugin:event|listen':
            return Promise.resolve(0);
          case 'plugin:event|unlisten':
            return Promise.resolve(null);
          case 'plugin:os|locale':
            return Promise.resolve('en-US');
          case 'get_embedding_provider_status':
            return Promise.resolve({
              backend: 'openai',
              model: 'text-embedding-3-small',
              dimension: 1536,
              api_key_configured: true,
              local_available: false,
              local_cached: false,
            });
          case 'get_embedding_model_mismatch':
            return Promise.resolve({ active_model: 'nomic-embed-text-v1.5', stale: [] });
          case 'get_campaigns':
            return Promise.resolve([{ id: 'camp1', name: 'Test Campaign', system: 'D&D 5e' }]);
          case 'get_entity_counts':
            return Promise.resolve({});
          case 'get_sessions':
            return Promise.resolve([]);
          case 'get_collections':
            return Promise.resolve([]);
          case 'get_sources':
            return Promise.resolve([]);
          case 'get_settings':
            return Promise.resolve({
              llm_provider: 'openai',
              llm_model: 'gpt-4o-mini',
              llm_api_key: 'sk-test',
              llm_base_url: '',
              active_campaign_id: '',
            });
          case 'update_setting':
            return Promise.resolve(null);
          case 'get_llm_provider_status':
            return Promise.resolve({
              provider_type: 'openai',
              model: 'gpt-4o-mini',
              api_key_configured: true,
            });
          case 'get_chat_history':
            return Promise.resolve([]);
          case 'chat_send':
            return Promise.resolve(null);
          case 'get_custom_providers':
            return Promise.resolve([]);
          case 'get_provider_models':
            return Promise.resolve([]);
          case 'delete_campaign':
            return Promise.resolve(null);
          case 'get_campaign_collections':
            return Promise.resolve([]);
          default:
            console.warn(`Unhandled IPC mock: ${cmd}`);
            return Promise.resolve(null);
        }
      },
    };
  });
}
```

- [ ] **Step 2: Replace the inline mock in `chronacle.spec.ts`**

Replace lines 1–101 (imports + `beforeEach` with the inline `addInitScript`) with:

```ts
import { test, expect } from '@playwright/test';
import { installIpcMock } from './ipc-mock';

test.describe('Chronacle Backend IPC', () => {
  test.beforeEach(async ({ page }) => {
    await installIpcMock(page);
  });
```

Keep the five existing `test(...)` blocks and closing `});` unchanged.

- [ ] **Step 3: Run the existing suite to prove the refactor is behavior-neutral**

```bash
pnpm -C apps/desktop exec playwright test --project=backend
```

Expected: all existing backend tests PASS (the vite dev server starts via `webServer`).

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/tests/e2e/backend/ipc-mock.ts apps/desktop/tests/e2e/backend/chronacle.spec.ts
git commit -m "refactor(e2e): extract shared IPC mock with call recording"
```

### Task 3: Smoke feature, step fixtures, features README

**Files:**
- Create: `apps/desktop/tests/e2e/features/smoke.feature`
- Create: `apps/desktop/tests/e2e/features/README.md`
- Create: `apps/desktop/tests/e2e/backend/steps/fixtures.ts`
- Create: `apps/desktop/tests/e2e/backend/steps/app.steps.ts`

**Interfaces:**
- Produces: BDD fixture `test` with auto-installed IPC mock; `Given/When/Then` exports from `steps/fixtures.ts` for all future step files; steps `the app is running with a seeded campaign {string}`, `the GM opens the app`, `the topbar shows the app title {string}`.

- [ ] **Step 1: Write the feature (this is the failing test)**

`apps/desktop/tests/e2e/features/smoke.feature`:

```gherkin
Feature: App shell smoke
  The BDD harness (ADR-011) drives the frontend with mocked Tauri IPC.
  This feature proves the toolchain wiring end to end.

  Scenario: GM opens the app and reaches the Oracle
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app
    Then the topbar shows the app title "Oracle"
```

- [ ] **Step 2: Run to verify it fails (no steps defined yet)**

```bash
pnpm -C apps/desktop run e2e:backend
```

Expected: `bddgen` FAILS with "missing step definitions" (it prints step snippets).

- [ ] **Step 3: Write the fixtures and steps**

`apps/desktop/tests/e2e/backend/steps/fixtures.ts`:

```ts
import { test as base, createBdd } from 'playwright-bdd';
import { installIpcMock } from '../ipc-mock';

/** BDD test with the Tauri IPC mock auto-installed before every scenario. */
export const test = base.extend<{ ipcMock: void }>({
  ipcMock: [
    async ({ page }, use) => {
      await installIpcMock(page);
      await use();
    },
    { auto: true },
  ],
});

export const { Given, When, Then } = createBdd(test);
```

`apps/desktop/tests/e2e/backend/steps/app.steps.ts`:

```ts
import { expect } from '@playwright/test';
import { Given, When, Then } from './fixtures';

Given('the app is running with a seeded campaign {string}', async (_ctx, _name: string) => {
  // Seeding comes from the shared IPC mock (get_campaigns → "Test Campaign").
  // The argument documents the precondition in the scenario text.
});

When('the GM opens the app', async ({ page }) => {
  await page.goto('/');
});

Then('the topbar shows the app title {string}', async ({ page }, title: string) => {
  await expect(page.locator('header .title')).toHaveText(title);
});
```

- [ ] **Step 4: Write `apps/desktop/tests/e2e/features/README.md`**

```markdown
# Acceptance features (ADR-011)

Gherkin `.feature` files here are executable: `bddgen` (playwright-bdd)
generates Playwright tests into `tests/e2e/.features-gen/` (gitignored),
run by the `bdd` Playwright project. Step definitions live in
`../backend/steps/`.

Run locally: `pnpm -C apps/desktop run e2e:backend`

## Conventions

- One `.feature` per feature area; scenario text is copied from the
  design spec's BDD section.
- Steps drive the frontend against the shared Tauri IPC mock
  (`../backend/ipc-mock.ts`) — the same harness as the hand-written
  backend specs.
- **Backend-only scenarios** (service-layer behaviour with no UI
  surface, e.g. schema rules or scope validation) cannot execute
  through this harness. They ship instead as Rust integration tests in
  `apps/desktop/src-tauri/tests/`, named to mirror the Gherkin scenario
  (e.g. `relation_between_two_regular_collections_is_rejected`), and the
  spec's scenario list is the source of truth for both.
```

- [ ] **Step 5: Run to verify it passes**

```bash
pnpm -C apps/desktop run e2e:backend
```

Expected: PASS — existing backend project tests plus 1 generated BDD test.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/tests/e2e/features apps/desktop/tests/e2e/backend/steps
git commit -m "test(e2e): first executable BDD feature via playwright-bdd"
```

### Task 4: CI wiring

**Files:**
- Modify: `.github/workflows/ci.yml` (frontend-check job, after the `Test` step at ~line 92-94)

Note: CI currently does **not** run the Playwright suite at all (despite AGENTS.md describing it as required) — this task closes that gap.

- [ ] **Step 1: Add two steps to the `frontend-check` job after `Test`**

```yaml
      - name: Install Playwright browsers
        working-directory: apps/desktop
        run: pnpm exec playwright install --with-deps chromium

      - name: Backend E2E + BDD acceptance
        working-directory: apps/desktop
        run: pnpm run e2e:backend
```

- [ ] **Step 2: Validate YAML locally**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo OK
```

Expected: `OK`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run backend Playwright E2E + BDD acceptance on every PR"
```

### Task 5: Verify, push, PR (A0)

- [ ] **Step 1: Full verification**

```bash
pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
```

Expected: all PASS.

- [ ] **Step 2: Push and open PR**

```bash
git push -u origin chore/a0-bdd-tooling
gh pr create --title "chore: executable BDD acceptance tooling (ADR-011)" --body "$(cat <<'EOF'
## What
playwright-bdd + @cucumber/cucumber devDeps; features/ + steps/ scaffolding; shared IPC mock with call recording; smoke feature; CI now runs the backend Playwright suite (it previously did not).

## Why
ADR-011 makes BDD acceptance scenarios executable and mandatory. Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md (PR A0).

## Tested
pnpm -C apps/desktop typecheck/lint/test:run; pnpm -C apps/desktop run e2e:backend (backend + bdd projects green).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR A1b — `feat/a1b-two-mode-delete-ui`

Finishes ADR-010: the frontend asks cascade-vs-convert; the backend parameter becomes required. Depends on A0 (ships a `.feature`).

### Task 6: Backend — make `on_owned_collection` required

**Files:**
- Modify: `apps/desktop/src-tauri/src/commands/campaign_commands.rs:78-97`

**Interfaces:**
- Consumes: `chronacle_domain::campaign_service::{delete, OnOwnedCollection}` (exists since A1a; serde `snake_case`: `"delete"` / `"convert_to_regular"`).
- Produces: `delete_campaign(id, on_owned_collection)` — parameter now required; omitting it is a Tauri deserialization error.

- [ ] **Step 1: Replace the command (removes the temporary A1a default)**

```rust
/// Delete a campaign, deciding what to do with its owned collection.
///
/// `on_owned_collection` mirrors `campaign_service::OnOwnedCollection` and is
/// deserialized with `snake_case` naming (`"delete"` or `"convert_to_regular"`).
/// The parameter is required (PR-A1b): omitting it is a command error, so the
/// frontend must always make the cascade-vs-convert choice explicit.
#[tauri::command]
pub async fn delete_campaign(
    state: State<'_, Arc<AppState>>,
    id: String,
    on_owned_collection: chronacle_domain::campaign_service::OnOwnedCollection,
) -> Result<(), String> {
    chronacle_domain::campaign_service::delete(&state.db, &id, on_owned_collection).await
}
```

- [ ] **Step 2: Build and test the workspace**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
```

Expected: PASS (service-layer tests already pass `OnOwnedCollection` explicitly since A1a; if any test calls the command with `None`, update it to pass a mode).

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/commands/campaign_commands.rs
git commit -m "feat(commands): require on_owned_collection on delete_campaign"
```

### Task 7: Frontend command wrapper

**Files:**
- Modify: `apps/desktop/src/lib/commands.ts:310-312`

**Interfaces:**
- Produces: `type OnOwnedCollection = 'delete' | 'convert_to_regular'` and `deleteCampaign(id: string, onOwnedCollection: OnOwnedCollection): Promise<void>` — used by Task 8.

- [ ] **Step 1: Replace `deleteCampaign`**

```ts
/** What happens to a campaign's owned collection when the campaign is deleted. */
export type OnOwnedCollection = 'delete' | 'convert_to_regular';

export async function deleteCampaign(
  id: string,
  onOwnedCollection: OnOwnedCollection,
): Promise<void> {
  return invoke('delete_campaign', { id, onOwnedCollection });
}
```

- [ ] **Step 2: Typecheck (expected to FAIL — CampaignView still calls the 1-arg form)**

```bash
pnpm -C apps/desktop typecheck
```

Expected: FAIL in `CampaignView.svelte` — this is the TDD signal for Task 8.

- [ ] **Step 3: Commit together with Task 8** (typecheck must be green per commit; hold this change and commit with the view change in Task 8 Step 5).

### Task 8: Two-mode delete dialog in CampaignView (TDD)

**Files:**
- Modify: `apps/desktop/src/views/CampaignView.svelte:158-167` (replace `removeCampaign`) and template/styles
- Test: `apps/desktop/src/views/CampaignView.test.ts`

**Interfaces:**
- Consumes: `deleteCampaign(id, mode)` + `OnOwnedCollection` from Task 7.
- Produces: dialog with `role="dialog"` and `aria-label="Delete campaign"`; buttons labeled exactly `Delete campaign and its notes`, `Keep notes as a regular collection`, `Cancel` (the A1b `.feature` in Task 9 asserts these strings).

Implementer note: load the `svelte-core-bestpractices` and `chronacle-design` skills before editing `.svelte` files.

- [ ] **Step 1: Write the failing tests** (append inside the existing `describe('CampaignView', …)` in `CampaignView.test.ts`; the module mock at the top of the file already stubs `deleteCampaign`):

```ts
  async function openDeleteDialog() {
    render(CampaignView, {
      props: {
        activeCampaignId: 'camp-1',
        campaigns: [camp('camp-1', 'Hollow Reach', '5e')],
        setActiveCampaignId: vi.fn(),
        onOpenUpload: vi.fn(),
        refreshCampaigns: vi.fn(),
      },
    });
    await fireEvent.click(screen.getByText(/Manage campaigns/));
    await fireEvent.click(screen.getByTitle('Delete'));
    return screen.findByRole('dialog', { name: /delete campaign/i });
  }

  it('delete opens a dialog offering cascade and convert', async () => {
    await openDeleteDialog();
    expect(screen.getByText('Delete campaign and its notes')).toBeTruthy();
    expect(screen.getByText('Keep notes as a regular collection')).toBeTruthy();
    expect(m.deleteCampaign).not.toHaveBeenCalled();
  });

  it('cascade choice forwards mode "delete"', async () => {
    await openDeleteDialog();
    await fireEvent.click(screen.getByText('Delete campaign and its notes'));
    await waitFor(() => expect(m.deleteCampaign).toHaveBeenCalledWith('camp-1', 'delete'));
  });

  it('keep-notes choice forwards mode "convert_to_regular"', async () => {
    await openDeleteDialog();
    await fireEvent.click(screen.getByText('Keep notes as a regular collection'));
    await waitFor(() =>
      expect(m.deleteCampaign).toHaveBeenCalledWith('camp-1', 'convert_to_regular'),
    );
  });

  it('cancel closes the dialog without deleting', async () => {
    await openDeleteDialog();
    await fireEvent.click(screen.getByText('Cancel'));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(m.deleteCampaign).not.toHaveBeenCalled();
  });
```

- [ ] **Step 2: Run to verify they fail**

```bash
pnpm -C apps/desktop test:run -- CampaignView
```

Expected: 4 new tests FAIL (no dialog exists; `confirm()` path still active).

- [ ] **Step 3: Implement in `CampaignView.svelte`**

Script changes — import the type, replace `removeCampaign`:

```ts
import { deleteCampaign, type OnOwnedCollection /* …existing imports… */ } from '../lib/commands';

let deleteTarget: Campaign | null = $state(null);

async function confirmDelete(mode: OnOwnedCollection) {
  if (!deleteTarget) return;
  const target = deleteTarget;
  deleteTarget = null;
  try {
    await deleteCampaign(target.id, mode);
    if (activeCampaignId === target.id) setActiveCampaignId(null);
    await refreshCampaigns();
  } catch (e) {
    error = String(e);
  }
}
```

The old `removeCampaign(c)` body (`confirm()` + delete) becomes simply:

```ts
function removeCampaign(c: Campaign) {
  deleteTarget = c;
}
```

Template — add at the end of the top-level `.cv` container:

```svelte
{#if deleteTarget}
  <div class="modal-overlay" role="presentation" onclick={() => (deleteTarget = null)}>
    <div
      class="modal"
      role="dialog"
      aria-label="Delete campaign"
      onclick={(e) => e.stopPropagation()}
    >
      <h3>Delete “{deleteTarget.name}”?</h3>
      <p>
        This campaign owns a collection holding its notes and entities. Choose what happens to
        that collection.
      </p>
      <div class="modal-actions">
        <button class="m-btn danger" onclick={() => confirmDelete('delete')}>
          Delete campaign and its notes
        </button>
        <button class="m-btn" onclick={() => confirmDelete('convert_to_regular')}>
          Keep notes as a regular collection
        </button>
        <button class="m-btn" onclick={() => (deleteTarget = null)}>Cancel</button>
      </div>
    </div>
  </div>
{/if}
```

Styles — append to the component `<style>` block, matching existing tokens (`m-btn`/`danger` classes already exist):

```css
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: grid;
  place-items: center;
  z-index: 50;
}
.modal {
  background: var(--bg-raised, #1d1a17);
  border: 1px solid var(--border, #3a352f);
  border-radius: 8px;
  padding: 1.25rem;
  max-width: 26rem;
}
.modal-actions {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-top: 1rem;
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
pnpm -C apps/desktop test:run -- CampaignView && pnpm -C apps/desktop typecheck
```

Expected: PASS (including the previously-failing typecheck from Task 7).

- [ ] **Step 5: Commit (includes Task 7's `commands.ts` change)**

```bash
git add apps/desktop/src/lib/commands.ts apps/desktop/src/views/CampaignView.svelte apps/desktop/src/views/CampaignView.test.ts
git commit -m "feat(ui): ask cascade-vs-convert when deleting a campaign"
```

### Task 9: A1b acceptance feature

**Files:**
- Create: `apps/desktop/tests/e2e/features/campaign-delete.feature`
- Create: `apps/desktop/tests/e2e/backend/steps/campaign.steps.ts`

**Interfaces:**
- Consumes: `Given/When/Then` from `steps/fixtures.ts` (Task 3); `window.__ipcCalls` recording (Task 2); the dialog labels from Task 8.

- [ ] **Step 1: Write the feature**

```gherkin
Feature: Campaign deletion modes
  Deleting a campaign must never silently destroy its owned collection.
  The GM chooses: cascade-delete the notes, or keep them as a regular
  collection (ADR-010).

  Background:
    Given the app is running with a seeded campaign "Test Campaign"
    When the GM opens the app

  Scenario: The GM must choose what happens to the campaign's notes
    When the GM opens the campaign manager
    And the GM clicks delete on the campaign "Test Campaign"
    Then a dialog offers "Delete campaign and its notes" and "Keep notes as a regular collection"

  Scenario: Cancelling the dialog deletes nothing
    When the GM opens the campaign manager
    And the GM clicks delete on the campaign "Test Campaign"
    And the GM cancels the dialog
    Then no delete command was sent to the backend
```

- [ ] **Step 2: Run to verify missing steps**

```bash
pnpm -C apps/desktop run e2e:backend
```

Expected: `bddgen` FAILS listing 4 undefined steps.

- [ ] **Step 3: Write `steps/campaign.steps.ts`**

```ts
import { expect } from '@playwright/test';
import { When, Then } from './fixtures';

When('the GM opens the campaign manager', async ({ page }) => {
  // Rail button → campaign view, then expand the manage list.
  await page.locator('button[title="Manage campaign and source collections"]').click();
  await page.getByText('Manage campaigns').click();
});

When('the GM clicks delete on the campaign {string}', async ({ page }, name: string) => {
  const row = page.locator('.manage-row', { hasText: name });
  await row.locator('button[title="Delete"]').click();
});

Then(
  'a dialog offers {string} and {string}',
  async ({ page }, optionA: string, optionB: string) => {
    const dialog = page.getByRole('dialog', { name: 'Delete campaign' });
    await expect(dialog.getByText(optionA)).toBeVisible();
    await expect(dialog.getByText(optionB)).toBeVisible();
  },
);

When('the GM cancels the dialog', async ({ page }) => {
  await page.getByRole('dialog', { name: 'Delete campaign' }).getByText('Cancel').click();
});

Then('no delete command was sent to the backend', async ({ page }) => {
  const calls = await page.evaluate(
    () => (window as unknown as { __ipcCalls: Array<{ cmd: string }> }).__ipcCalls,
  );
  expect(calls.some((c) => c.cmd === 'delete_campaign')).toBe(false);
});
```

- [ ] **Step 4: Run to verify the scenarios pass**

```bash
pnpm -C apps/desktop run e2e:backend
```

Expected: PASS (3 BDD scenarios total incl. smoke).

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/tests/e2e/features/campaign-delete.feature apps/desktop/tests/e2e/backend/steps/campaign.steps.ts
git commit -m "test(e2e): acceptance feature for two-mode campaign delete"
```

### Task 10: ADR-010 status note; verify, push, PR (A1b)

**Files:**
- Modify: `docs/architecture.md` ADR-010 section (~line 1143-1146)

- [ ] **Step 1: Update the ADR paragraph**

Replace:

> The Tauri command layer surfaces this as an `on_owned_collection` parameter on `delete_campaign`. In PR-A1a the parameter is temporarily optional and defaults to `"delete"` so the pre-A1a frontend keeps working; PR-A1b makes it required once the two-mode confirmation UI lands.

with:

> The Tauri command layer surfaces this as a **required** `on_owned_collection` parameter on `delete_campaign` (made required in PR-A1b, 2026-07, together with the two-mode confirmation dialog). Callers must pass `"delete"` or `"convert_to_regular"`; omitting it is a command error.

- [ ] **Step 2: Full verification**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
```

Expected: all PASS.

- [ ] **Step 3: Commit, push, PR**

```bash
git add docs/architecture.md
git commit -m "docs(adr-010): on_owned_collection now required (A1b)"
git push -u origin feat/a1b-two-mode-delete-ui
gh pr create --title "feat: two-mode campaign delete dialog (ADR-010, PR-A1b)" --body "$(cat <<'EOF'
## What
Cascade-vs-convert dialog in CampaignView; delete_campaign's on_owned_collection is now required; acceptance feature added.

## Why
Finishes ADR-010 (A1b). Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md.

## Tested
cargo test --workspace; Vitest CampaignView suite; pnpm run e2e:backend (BDD scenarios for the dialog).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR A2a — `feat/a2a-codex-schema`

Additive Codex schema + ADR-009. Branch from main after A1b merges (or from main directly — no code dependency on A1b).

### Task 11: Schema tests first (TDD)

**Files:**
- Create: `apps/desktop/src-tauri/tests/schema_wiki_layer_a2_test.rs`

**Interfaces:**
- Consumes: `chronacle_db::run_migrations` (existing).
- Produces: the executable definition of the A2a schema surface. Backend-only BDD scenarios "migrations run twice lose nothing" and "rule_entry rejects unknown categories" live here per the `features/README.md` convention.

- [ ] **Step 1: Create the branch**

```bash
git checkout --no-track -b feat/a2a-codex-schema main
```

- [ ] **Step 2: Write the failing tests**

```rust
//! Schema-level tests for the Codex slice of `002_wiki_layer.surql` (A2a).
//!
//! Mirrors the BDD scenarios in the codex spec that have no UI surface
//! (see apps/desktop/tests/e2e/features/README.md for the convention).

use serde::Deserialize;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;

async fn setup_db() -> Surreal<Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[derive(Deserialize)]
struct CountRow {
    count: i64,
}

async fn count(db: &Surreal<Db>, query: &str) -> i64 {
    let mut resp = db.query(query).await.unwrap();
    let rows: Vec<CountRow> = resp.take(0).unwrap();
    rows.first().map(|r| r.count).unwrap_or(0)
}

#[tokio::test]
async fn codex_fields_default_on_entity_tables() {
    let db = setup_db().await;
    db.query("CREATE npc SET name = 'Mira'").await.unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count() FROM npc WHERE codex_stale = false \
               AND codex_article = NONE AND codex_sources = [] GROUP ALL",
        )
        .await,
        1,
        "codex fields must default to not-stale / no article / empty provenance"
    );
}

#[tokio::test]
async fn rule_entry_accepts_all_seven_categories() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    for cat in [
        "mechanic",
        "ability",
        "state",
        "procedure",
        "resource",
        "statistic",
        "entry",
    ] {
        db.query(
            "CREATE rule_entry SET collection = collection:`c1`, name = $name, \
             category = $cat, body = 'b', compiled_at = time::now()",
        )
        .bind(("name", format!("rule-{cat}")))
        .bind(("cat", cat.to_owned()))
        .await
        .unwrap()
        .check()
        .unwrap_or_else(|e| panic!("category {cat} must be accepted: {e}"));
    }
    assert_eq!(count(&db, "SELECT count() FROM rule_entry GROUP ALL").await, 7);
}

#[tokio::test]
async fn rule_entry_rejects_unknown_category() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    let res = db
        .query(
            "CREATE rule_entry SET collection = collection:`c1`, name = 'bad', \
             category = 'vibe', body = 'b', compiled_at = time::now()",
        )
        .await
        .unwrap()
        .check();
    assert!(res.is_err(), "unknown category must be rejected by ASSERT");
}

#[tokio::test]
async fn rule_entry_name_unique_per_collection() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    let create = "CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
                  category = 'mechanic', body = 'b', compiled_at = time::now()";
    db.query(create).await.unwrap().check().unwrap();
    let dup = db.query(create).await.unwrap().check();
    assert!(dup.is_err(), "(collection, name) must be UNIQUE");
}

#[tokio::test]
async fn codex_proposal_defaults_to_pending() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Notes', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    db.query(
        "CREATE codex_proposal SET kind = 'entity_article_update', \
         target = npc:`n1`, collection = collection:`c1`, \
         payload = { proposed_text: 'x', rationale: 'y' }, \
         origin = { kind: 'manual' }",
    )
    .await
    .unwrap()
    .check()
    .expect("codex_proposal must accept a minimal row");
    assert_eq!(
        count(
            &db,
            "SELECT count() FROM codex_proposal WHERE status = 'pending' \
               AND resolved_at = NONE GROUP ALL",
        )
        .await,
        1
    );
}

#[tokio::test]
async fn migration_002_a2_is_idempotent_and_preserves_rule_entries() {
    let db = setup_db().await;
    db.query("CREATE collection SET id = 'c1', name = 'Rules', description = NULL, created_at = time::now(), updated_at = time::now()")
        .await
        .unwrap();
    db.query(
        "CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
         category = 'mechanic', body = 'b', compiled_at = time::now()",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    assert_eq!(count(&db, "SELECT count() FROM rule_entry GROUP ALL").await, 1);
}
```

- [ ] **Step 3: Run to verify they fail**

```bash
cargo test -p Chronacle --test schema_wiki_layer_a2_test
```

(`Chronacle` — capital C — is the src-tauri package name.)
Expected: FAIL — `codex_stale`, `rule_entry`, `codex_proposal` don't exist yet.

- [ ] **Step 4: Commit the failing tests**

```bash
git add apps/desktop/src-tauri/tests/schema_wiki_layer_a2_test.rs
git commit -m "test(schema): failing coverage for codex A2a schema surface"
```

### Task 12: Extend `002_wiki_layer.surql`

**Files:**
- Modify: `crates/chronacle-db/src/schema/002_wiki_layer.surql` (append)

**Interfaces:**
- Produces: `codex_article`, `codex_compiled_at`, `codex_stale`, `codex_sources` on all 8 entity tables; `rule_entry`; `codex_proposal`. All `DEFINE … OVERWRITE`, additive only.

- [ ] **Step 1: Append the A2a slice**

Append this block. The entity-field block below is written out once per table for **all eight** tables: `npc`, `location`, `faction`, `creature`, `item`, `event`, `player_character`, `misc` (32 `DEFINE FIELD` statements total — no loops exist in `.surql`):

```sql
-- ═══ A2a slice — Codex compiled world model (ADR-009) ═══════════════════════

-- ── Entity codex-article fields (machine-owned; user summary/notes untouched) ─
-- Repeated identically for: npc, location, faction, creature, item, event,
-- player_character, misc.
-- codex_sources entry shapes by kind:
--   { kind: "chunk", source, page_start, page_end }
--   { kind: "session", session }
--   { kind: "proposal", proposal }

DEFINE FIELD OVERWRITE codex_article     ON TABLE npc TYPE option<string> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_compiled_at ON TABLE npc TYPE option<datetime> DEFAULT NONE;
DEFINE FIELD OVERWRITE codex_stale       ON TABLE npc TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE codex_sources     ON TABLE npc TYPE array<object> DEFAULT [];

-- (…same four statements for location, faction, creature, item, event,
--  player_character, misc — write them out in full…)

-- ── Rule entries (compiled rules layer; ADR-009) ─────────────────────────────
-- body/category/page_refs/sources are compiler-owned; notes is GM-owned and
-- survives recompiles. GM corrections go through "redo with objections",
-- recorded in sources as { kind: "objection", text, at }.

DEFINE TABLE OVERWRITE rule_entry SCHEMAFULL;
DEFINE FIELD OVERWRITE collection  ON TABLE rule_entry TYPE record<collection>;
DEFINE FIELD OVERWRITE name        ON TABLE rule_entry TYPE string;
DEFINE FIELD OVERWRITE category    ON TABLE rule_entry TYPE string
    ASSERT $value IN ['mechanic', 'ability', 'state', 'procedure', 'resource', 'statistic', 'entry'];
DEFINE FIELD OVERWRITE body        ON TABLE rule_entry TYPE string;
DEFINE FIELD OVERWRITE notes       ON TABLE rule_entry TYPE string | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE page_refs   ON TABLE rule_entry TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE sources     ON TABLE rule_entry TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE compiled_at ON TABLE rule_entry TYPE datetime;
DEFINE FIELD OVERWRITE stale       ON TABLE rule_entry TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE embedding   ON TABLE rule_entry TYPE array<float> | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE embed_model ON TABLE rule_entry TYPE string | NULL DEFAULT NULL;
DEFINE FIELD OVERWRITE created_at  ON TABLE rule_entry TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at  ON TABLE rule_entry TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE rule_entry_collection_idx ON TABLE rule_entry COLUMNS collection;
DEFINE INDEX OVERWRITE rule_entry_name_idx       ON TABLE rule_entry COLUMNS collection, name UNIQUE;
DEFINE INDEX OVERWRITE rule_entry_embedding_idx  ON TABLE rule_entry FIELDS embedding MTREE DIMENSION 768 DIST COSINE;

-- ── Codex proposals (write-back review queue; ADR-009) ───────────────────────
-- payload: { proposed_text, rationale } for *_update kinds; full draft object
-- for new_* kinds. origin: { kind: "chat", message } | { kind: "session",
-- session } | { kind: "manual" }. target unset for new_* kinds.

DEFINE TABLE OVERWRITE codex_proposal SCHEMAFULL;
DEFINE FIELD OVERWRITE kind        ON TABLE codex_proposal TYPE string
    ASSERT $value IN ['entity_article_update', 'entity_notes_update',
                      'rule_entry_update', 'new_entity', 'new_rule_entry'];
DEFINE FIELD OVERWRITE target      ON TABLE codex_proposal TYPE option<record> DEFAULT NONE;
DEFINE FIELD OVERWRITE collection  ON TABLE codex_proposal TYPE record<collection>;
DEFINE FIELD OVERWRITE campaign    ON TABLE codex_proposal TYPE option<record<campaign>> DEFAULT NONE;
DEFINE FIELD OVERWRITE payload     ON TABLE codex_proposal TYPE object;
DEFINE FIELD OVERWRITE origin      ON TABLE codex_proposal TYPE object;
DEFINE FIELD OVERWRITE status      ON TABLE codex_proposal TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'accepted', 'rejected'];
DEFINE FIELD OVERWRITE created_at  ON TABLE codex_proposal TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE resolved_at ON TABLE codex_proposal TYPE option<datetime> DEFAULT NONE;
DEFINE INDEX OVERWRITE codex_proposal_status_idx ON TABLE codex_proposal COLUMNS status;

-- ── lint_finding: kinds added from A2b onward ────────────────────────────────
-- (no schema change — kind vocabulary documented here)
--   * "scope_violation"  — payload = { edge: option, from, to,
--                                       from_collection, to_collection }
--   * "broken_wikilink"  — payload = { entity, link_text }          (C2)
--   * "stale_article"    — payload = { entity, reason }             (C2)
--   * "duplicate_entity" — payload = { a, b, similarity }           (C2)
```

- [ ] **Step 2: Run the schema tests**

```bash
cargo test -p Chronacle --test schema_wiki_layer_a2_test
```

Expected: all 6 PASS. Also run `cargo test --workspace` — existing tests must stay green (fields are additive with defaults).

- [ ] **Step 3: Commit**

```bash
git add crates/chronacle-db/src/schema/002_wiki_layer.surql
git commit -m "feat(schema): codex fields, rule_entry, codex_proposal (A2a)"
```

### Task 13: ADR-009, docs; verify, push, PR (A2a)

**Files:**
- Modify: `docs/architecture.md` — insert ADR-009 **before** the ADR-010 section (~line 1087) to keep numeric order.

- [ ] **Step 1: Insert ADR-009**

```markdown
## ADR-009: Compiled World Model — The Codex

**Status:** Accepted (2026-07-03). Schema landed in PR-A2a; behaviour lands
across the A2b–C2 series. Full design:
`docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md`.

### Context

Chronacle answered every question by re-deriving from raw chunks plus thin
entity summaries: no durable compiled knowledge, no write-back of durable
results, no linting, and no compiled rules layer (the LLM Wiki gap).

### Decision

A compiled layer — the **Codex** — sits between extraction and answering:

- **Setting articles live on the entity tables** as machine-owned fields
  (`codex_article`, `codex_compiled_at`, `codex_stale`, `codex_sources`).
  User `summary`/`notes` are never machine-overwritten.
- **Rules are a separate aggregate**, `rule_entry`, collection-scoped, with
  a closed category enum (`mechanic`, `ability`, `state`, `procedure`,
  `resource`, `statistic`, `entry`), compiler-owned body/page-refs and a
  GM-owned `notes` field. Corrections go through "redo with objections".
- **Write-back is a review queue** (`codex_proposal`): chat answers and
  session notes propose changes; nothing mutates the compiled layer
  without explicit accept.
- **Compilation is manual with staleness markers** — never automatic.
- **Reference rules are enforced**: content of a campaign-bound collection
  may reference collections its owner campaign subscribes to; content of a
  regular collection may reference only that same collection. Enforced at
  relation write time, at compile-provenance time, and by lint pass.
- Retrieval consumes compiled layers first: RULES → CODEX → ENTITIES →
  CHUNKS block ordering (lands in PR-B3).
- Everything compiler-owned is derived state, recompilable from chunks +
  accepted proposals + stored objections — the layer's core safety
  property.

### Consequences

- Positive: durable, citable, incrementally-maintained knowledge; rules
  and setting stay separate in the domain model, retrieval, and UX.
- Negative: LLM compile cost (mitigated: manual trigger + staleness
  increments); entity embeddings change semantics once articles are
  folded in (accepted — strictly richer signal, same embed model).
- The `codex_service` lives in `chronacle-extraction` (same dependency
  shape as extraction); extracting a dedicated crate later is mechanical.
```

- [ ] **Step 2: Verification, push, PR**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace
git add docs/architecture.md
git commit -m "docs(adr-009): compiled world model (Codex) accepted"
git push -u origin feat/a2a-codex-schema
gh pr create --title "feat: Codex schema — entity fields, rule_entry, proposals (A2a)" --body "$(cat <<'EOF'
## What
Additive 002_wiki_layer.surql slice: codex fields on all 8 entity tables, rule_entry (7-category closed enum), codex_proposal review queue; ADR-009; schema tests.

## Why
Codex spec PR-A2a. Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md.

## Tested
cargo test --workspace incl. new schema_wiki_layer_a2_test (idempotency, category ASSERT, UNIQUE (collection,name), defaults).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR A2b — `feat/a2b-staleness-scope`

Staleness producers + reference-scope validation. Depends on A2a (schema fields must exist). Branch after A2a merges: `git checkout --no-track -b feat/a2b-staleness-scope main`.

### Task 14: `codex_service` skeleton

**Files:**
- Create: `crates/chronacle-extraction/src/codex_service/mod.rs`
- Modify: `crates/chronacle-extraction/src/lib.rs` (add `pub mod codex_service;`)

**Interfaces:**
- Produces:
  - `codex_service::mark_entity_stale(db, table: &str, id: &str) -> Result<(), String>`
  - `codex_service::record_lint(db, kind: &str, payload: serde_json::Value) -> Result<(), String>`
  - Used by Tasks 16 and 18; later PRs (B1, C2) extend this module.

- [ ] **Step 1: Write the failing tests + implementation in one module**

`crates/chronacle-extraction/src/codex_service/mod.rs`:

```rust
//! Codex service — the compiled-world-model layer (ADR-009).
//!
//! A2b skeleton: staleness marking and lint recording. Compilation (B1),
//! rules (B2), and proposals (C1) extend this module in later PRs.

use surrealdb::Connection;

/// Mark one entity's codex article as stale (needs recompilation).
///
/// Producers: extraction touching an entity, user edits to summary/notes,
/// session-note mentions (C1). Cleared by the compiler (B1).
pub async fn mark_entity_stale<C: Connection>(
    db: &surrealdb::Surreal<C>,
    table: &str,
    id: &str,
) -> Result<(), String> {
    db.query("UPDATE type::thing($table, $id) SET codex_stale = true")
        .bind(("table", table.to_owned()))
        .bind(("id", id.to_owned()))
        .await
        .map_err(|e| format!("Failed to mark entity stale: {e}"))?;
    Ok(())
}

/// Record a lint finding for the maintenance inbox (C2 adds the UI).
///
/// `payload` shape depends on `kind`; shapes are documented in
/// `002_wiki_layer.surql`.
pub async fn record_lint<C: Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    db.query("CREATE lint_finding SET kind = $kind, payload = $payload")
        .bind(("kind", kind.to_owned()))
        .bind(("payload", payload))
        .await
        .map_err(|e| format!("Failed to record lint finding: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use surrealdb::engine::local::{Db, Mem};
    use surrealdb::Surreal;

    async fn setup_db() -> Surreal<Db> {
        let db = Surreal::new::<Mem>(()).await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        chronacle_db::run_migrations(&db).await.unwrap();
        db
    }

    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }

    async fn count(db: &Surreal<Db>, q: &str) -> i64 {
        let mut resp = db.query(q).await.unwrap();
        let rows: Vec<CountRow> = resp.take(0).unwrap();
        rows.first().map(|r| r.count).unwrap_or(0)
    }

    #[tokio::test]
    async fn mark_entity_stale_sets_the_flag() {
        let db = setup_db().await;
        db.query("CREATE npc:`n1` SET name = 'Mira'").await.unwrap();
        mark_entity_stale(&db, "npc", "n1").await.unwrap();
        assert_eq!(
            count(&db, "SELECT count() FROM npc WHERE codex_stale = true GROUP ALL").await,
            1
        );
    }

    #[tokio::test]
    async fn record_lint_creates_unresolved_finding() {
        let db = setup_db().await;
        record_lint(
            &db,
            "scope_violation",
            serde_json::json!({ "from": "npc:a", "to": "npc:b" }),
        )
        .await
        .unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT count() FROM lint_finding WHERE kind = 'scope_violation' \
                   AND resolved_at = NONE GROUP ALL"
            )
            .await,
            1
        );
    }
}
```

- [ ] **Step 2: Register the module** — in `crates/chronacle-extraction/src/lib.rs` add `pub mod codex_service;` alongside the existing modules.

- [ ] **Step 3: Run**

```bash
cargo test -p chronacle-extraction codex_service
```

Expected: 2 PASS. (`chronacle-db` is already a dev-dependency of this crate; `serde_json` is already a regular dependency.)

- [ ] **Step 4: Commit**

```bash
git add crates/chronacle-extraction/src/codex_service crates/chronacle-extraction/src/lib.rs
git commit -m "feat(codex): service skeleton — staleness + lint recording"
```

### Task 15: Ingestion staleness producer

**Files:**
- Modify: `crates/chronacle-ingestion/src/ingestion_service/mod.rs` (new helper + call after the `index_status = 'done'` update at ~line 164)
- Test: `crates/chronacle-ingestion/src/ingestion_service/tests.rs`

**Interfaces:**
- Produces: `pub(crate) async fn mark_codex_stale_for_source(db, source_id: &str) -> Result<(), IngestionError>`, called at the end of `ingest_source` (covers both upload and reindex command paths). Lives in the ingestion crate because `chronacle-ingestion` and `chronacle-extraction` are sibling crates with no dependency edge; the one duplicated query is documented.

- [ ] **Step 1: Write the failing test** (append in `tests.rs` — this file currently holds only pure unit tests with no DB setup, so add the helpers alongside the new test; `chronacle-db` is already a dev-dependency of this crate):

```rust
async fn setup_db() -> surrealdb::Surreal<surrealdb::engine::local::Db> {
    let db = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(())
        .await
        .unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

#[derive(serde::Deserialize)]
struct CountRow {
    count: i64,
}


#[tokio::test]
async fn ingest_completion_marks_collection_entities_and_rules_stale() {
    let db = setup_db().await; // existing helper: mem db + run_migrations
    db.query(
        "CREATE collection:`c1` SET name = 'Rules', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         CREATE source:`s1` SET collection = collection:`c1`, campaign = NULL, \
             filename = 'f.pdf', display_name = 'F', source_type = 'rules', \
             page_count = 0, indexed_at = time::now(), index_status = 'done', \
             embed_model = 'test';
         CREATE npc:`n1` SET name = 'Mira';
         RELATE collection:`c1`->in_collection->npc:`n1` SET created_at = time::now();
         CREATE rule_entry SET collection = collection:`c1`, name = 'Initiative', \
             category = 'mechanic', body = 'b', compiled_at = time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();

    super::mark_codex_stale_for_source(&db, "s1").await.unwrap();

    let mut resp = db
        .query("SELECT count() FROM npc WHERE codex_stale = true GROUP ALL")
        .await
        .unwrap();
    let rows: Vec<CountRow> = resp.take(0).unwrap();
    assert_eq!(rows.first().map(|r| r.count).unwrap_or(0), 1);

    let mut resp2 = db
        .query("SELECT count() FROM rule_entry WHERE stale = true GROUP ALL")
        .await
        .unwrap();
    let rows2: Vec<CountRow> = resp2.take(0).unwrap();
    assert_eq!(rows2.first().map(|r| r.count).unwrap_or(0), 1);
}
```


- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p chronacle-ingestion ingest_completion_marks
```

Expected: FAIL — function not defined.

- [ ] **Step 3: Implement in `ingestion_service/mod.rs`**

```rust
/// Mark the Codex stale for everything in this source's collection (ADR-009).
///
/// Entities get `codex_stale = true`; existing rule entries are marked stale
/// when the source can contain rules (`rules` / `supplement`). This lives in
/// the ingestion crate (not `codex_service`) because ingestion and extraction
/// are sibling crates with no dependency edge; the query is small and its
/// shape is documented in the codex spec.
pub(crate) async fn mark_codex_stale_for_source<C: Connection>(
    db: &surrealdb::Surreal<C>,
    source_id: &str,
) -> Result<(), IngestionError> {
    db.query(
        "LET $src = type::thing('source', $id);
         LET $col = array::first((SELECT VALUE collection FROM source WHERE id = $src));
         LET $stype = array::first((SELECT VALUE source_type FROM source WHERE id = $src));
         LET $ents = (SELECT VALUE out FROM in_collection WHERE in = $col);
         UPDATE $ents SET codex_stale = true;
         IF $stype IN ['rules', 'supplement'] THEN
             (UPDATE rule_entry SET stale = true WHERE collection = $col)
         END;",
    )
    .bind(("id", source_id.to_owned()))
    .await
    .map_err(|e| IngestionError::Db(format!("Failed to mark codex stale: {e}")))?;
    Ok(())
}
```

Then, in `ingest_source`, immediately after the `index_status = 'done'` update (~line 164-167), add:

```rust
    mark_codex_stale_for_source(db, source_id).await?;
```

(match the actual variable names in scope at that point).

- [ ] **Step 4: Run to verify it passes**

```bash
cargo test -p chronacle-ingestion
```

Expected: PASS, including pre-existing ingestion tests.

- [ ] **Step 5: Commit**

```bash
git add crates/chronacle-ingestion
git commit -m "feat(ingestion): mark codex stale when a source finishes indexing"
```

### Task 16: Reference-scope validation on relation writes

**Files:**
- Create: `crates/chronacle-extraction/src/entity_service/relations/scope.rs`
- Modify: `crates/chronacle-extraction/src/entity_service/relations/mod.rs` (add `mod scope;`)
- Modify: `crates/chronacle-extraction/src/entity_service/relations/edge.rs:19-67` (`relate` calls the check)
- Modify: `crates/chronacle-extraction/src/entity_service/types.rs` (new `EntityError::ScopeViolation` variant — the enum lives here; adjust if it is defined in `mod.rs` instead)
- Test: `crates/chronacle-extraction/src/entity_service/relations/scope_tests.rs` (new; add `#[cfg(test)] mod scope_tests;` in `relations/mod.rs`)

**Interfaces:**
- Consumes: `is_safe_record_id` (edge.rs), `EntityError`.
- Produces: `EntityError::ScopeViolation { from: String, to: String }`; `scope::check_scope(db, from_kind, from_id, to_kind, to_id) -> Result<(), EntityError>` invoked by `relate()` (and therefore by `relate_collapsing` and every service path). Task 18 matches on the new variant.

- [ ] **Step 1: Add the error variant** (in the `EntityError` definition):

```rust
    #[error("Scope violation: {from} may not reference {to} (see reference rules, ADR-009)")]
    ScopeViolation { from: String, to: String },
```

- [ ] **Step 2: Write the failing tests** (`relations/scope_tests.rs`; use the crate's existing test setup idiom — mem DB + `chronacle_db::run_migrations`):

```rust
//! Reference-rule matrix (ADR-009). Mirrors the spec's A2 BDD scenarios
//! (backend-only; see apps/desktop/tests/e2e/features/README.md).

use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

use crate::entity_service::{relate, EntityError};

async fn setup_db() -> Surreal<Db> {
    let db = Surreal::new::<Mem>(()).await.unwrap();
    db.use_ns("test").use_db("test").await.unwrap();
    chronacle_db::run_migrations(&db).await.unwrap();
    db
}

/// Seed: campaign `cam1` owns collection `owned1` and subscribes to regular
/// `reg1`; regular `reg2` is unrelated. One npc in each collection.
async fn seed(db: &Surreal<Db>) {
    db.query(
        "CREATE campaign:`cam1` SET name = 'C', system = 'x', \
             created_at = time::now(), updated_at = time::now();
         CREATE collection:`owned1` SET name = 'Own', description = NULL, \
             owner_campaign = campaign:`cam1`, created_at = time::now(), updated_at = time::now();
         CREATE collection:`reg1` SET name = 'R1', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         CREATE collection:`reg2` SET name = 'R2', description = NULL, \
             created_at = time::now(), updated_at = time::now();
         RELATE campaign:`cam1`->subscribes_to->collection:`owned1` SET created_at = time::now();
         RELATE campaign:`cam1`->subscribes_to->collection:`reg1` SET created_at = time::now();
         CREATE npc:`own_a` SET name = 'OwnA';
         CREATE npc:`own_b` SET name = 'OwnB';
         CREATE npc:`r1_a` SET name = 'R1A';
         CREATE npc:`r2_a` SET name = 'R2A';
         RELATE collection:`owned1`->in_collection->npc:`own_a` SET created_at = time::now();
         RELATE collection:`owned1`->in_collection->npc:`own_b` SET created_at = time::now();
         RELATE collection:`reg1`->in_collection->npc:`r1_a` SET created_at = time::now();
         RELATE collection:`reg2`->in_collection->npc:`r2_a` SET created_at = time::now();",
    )
    .await
    .unwrap()
    .check()
    .unwrap();
}

#[tokio::test]
async fn same_collection_relation_is_allowed() {
    let db = setup_db().await;
    seed(&db).await;
    relate(&db, "own_a", "npc", "own_b", "npc", "allied_with", None)
        .await
        .expect("same-collection edges are always legal");
}

#[tokio::test]
async fn campaign_bound_to_subscribed_regular_is_allowed_both_directions() {
    let db = setup_db().await;
    seed(&db).await;
    relate(&db, "own_a", "npc", "r1_a", "npc", "knows", None)
        .await
        .expect("campaign-bound → subscribed regular is legal");
    relate(&db, "r1_a", "npc", "own_a", "npc", "knows", None)
        .await
        .expect("the check is symmetric on the pair (ADR-010 cross-edges)");
}

#[tokio::test]
async fn campaign_bound_to_unsubscribed_regular_is_rejected() {
    let db = setup_db().await;
    seed(&db).await;
    let err = relate(&db, "own_a", "npc", "r2_a", "npc", "knows", None)
        .await
        .expect_err("cam1 does not subscribe to reg2");
    assert!(matches!(err, EntityError::ScopeViolation { .. }));
}

#[tokio::test]
async fn relation_between_two_regular_collections_is_rejected() {
    let db = setup_db().await;
    seed(&db).await;
    let err = relate(&db, "r1_a", "npc", "r2_a", "npc", "knows", None)
        .await
        .expect_err("regular collections may only self-reference");
    assert!(matches!(err, EntityError::ScopeViolation { .. }));
}

#[tokio::test]
async fn unscoped_legacy_entities_are_not_blocked() {
    let db = setup_db().await;
    seed(&db).await;
    db.query("CREATE npc:`floating` SET name = 'Ghost'")
        .await
        .unwrap();
    relate(&db, "floating", "npc", "own_a", "npc", "knows", None)
        .await
        .expect("entities without scope edges (legacy/tests) must not be blocked");
}
```

- [ ] **Step 3: Run to verify failures**

```bash
cargo test -p chronacle-extraction scope_tests
```

Expected: rejection tests FAIL (edges are currently created unconditionally).

- [ ] **Step 4: Implement `relations/scope.rs`**

```rust
//! Reference-scope validation for `relates_to` edges (ADR-009).
//!
//! Rules (symmetric on the unordered pair — ADR-010 already treats
//! owned↔subscribed cross-edges as legitimate in either direction):
//! * same collection → allowed;
//! * a pair {campaign-governed content, collection X} → allowed iff that
//!   campaign subscribes to X;
//! * two different regular collections → violation;
//! * an endpoint with no scope edges at all (legacy/test data) → allowed —
//!   we cannot judge it, and blocking would break pre-scope data.

use serde::Deserialize;
use surrealdb::sql::Thing;

use super::super::EntityError;

#[derive(Debug, Deserialize)]
struct EndpointScope {
    collection: Option<Thing>,
    campaign: Option<Thing>,
}

impl EndpointScope {
    fn unscoped(&self) -> bool {
        self.collection.is_none() && self.campaign.is_none()
    }
}

/// Resolve where an entity lives: its collection (`in_collection` edge) and
/// the campaign that governs it (`in_campaign` edge, or the collection's
/// `owner_campaign` when the collection is campaign-bound).
async fn endpoint_scope<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    kind: &str,
    id: &str,
) -> Result<EndpointScope, EntityError> {
    // kind/id are validated by the callers in `edge.rs` (is_safe_record_id).
    let q = format!(
        "LET $col = array::first((SELECT VALUE in FROM in_collection WHERE out = {kind}:{id}));
         LET $cam = array::first((SELECT VALUE in FROM in_campaign WHERE out = {kind}:{id}));
         LET $owner = IF $col IS NOT NONE THEN $col.owner_campaign ELSE NONE END;
         RETURN {{ collection: $col, campaign: $cam ?? $owner }};"
    );
    let mut resp = db.query(q).await.map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    // Statements 0-2 are LETs; the RETURN is index 3.
    let scope: Option<EndpointScope> = resp.take(3).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(scope.unwrap_or(EndpointScope {
        collection: None,
        campaign: None,
    }))
}

async fn is_subscribed<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    campaign: &Thing,
    collection: &Thing,
) -> Result<bool, EntityError> {
    #[derive(Deserialize)]
    struct CountRow {
        count: i64,
    }
    let mut resp = db
        .query("SELECT count() FROM subscribes_to WHERE in = $cam AND out = $col GROUP ALL")
        .bind(("cam", campaign.clone()))
        .bind(("col", collection.clone()))
        .await
        .map_err(|e| EntityError::Database {
            message: e.to_string(),
        })?;
    let rows: Vec<CountRow> = resp.take(0).map_err(|e| EntityError::Database {
        message: e.to_string(),
    })?;
    Ok(rows.first().map(|r| r.count).unwrap_or(0) > 0)
}

/// Enforce the reference rules for a prospective `relates_to` edge.
pub(super) async fn check_scope<C: surrealdb::Connection>(
    db: &surrealdb::Surreal<C>,
    from_kind: &str,
    from_id: &str,
    to_kind: &str,
    to_id: &str,
) -> Result<(), EntityError> {
    let a = endpoint_scope(db, from_kind, from_id).await?;
    let b = endpoint_scope(db, to_kind, to_id).await?;

    if a.unscoped() || b.unscoped() {
        return Ok(());
    }
    if a.collection.is_some() && a.collection == b.collection {
        return Ok(());
    }
    if a.campaign.is_some() && a.campaign == b.campaign {
        return Ok(());
    }
    // One side campaign-governed, other side in a collection that campaign
    // subscribes to (checked both ways — the rule is pair-symmetric).
    for (gov, other) in [(&a, &b), (&b, &a)] {
        if let (Some(cam), Some(col)) = (&gov.campaign, &other.collection) {
            if other.campaign.is_none() && is_subscribed(db, cam, col).await? {
                return Ok(());
            }
        }
    }

    Err(EntityError::ScopeViolation {
        from: format!("{from_kind}:{from_id}"),
        to: format!("{to_kind}:{to_id}"),
    })
}
```

Register in `relations/mod.rs`: `mod scope;` and `#[cfg(test)] mod scope_tests;`.

- [ ] **Step 5: Wire into `relate()`** — in `edge.rs`, after the two `is_safe_record_id` checks (line ~39), add:

```rust
    scope::check_scope(db, from_kind, from_id, to_kind, to_id).await?;
```

with `use super::scope;` at the top.

- [ ] **Step 6: Run the full crate + workspace tests**

```bash
cargo test -p chronacle-extraction && cargo test --workspace
```

Expected: scope tests PASS; all pre-existing relation/extraction/campaign tests PASS (test entities created without scope edges pass via the `unscoped` escape; service-created entities always share a collection in existing tests). If a pre-existing test now fails with `ScopeViolation`, that test was creating a genuinely cross-scope edge — fix the test's seeding to subscribe the campaign, not the production code.

- [ ] **Step 7: Commit**

```bash
git add crates/chronacle-extraction/src/entity_service
git commit -m "feat(entity): enforce reference-scope rules on relates_to writes"
```

### Task 17: Entity-edit staleness producer

**Files:**
- Modify: `crates/chronacle-extraction/src/entity_service/crud/update.rs:31-49`
- Test: `crates/chronacle-extraction/src/entity_service/crud/crud_tests_update.rs`

- [ ] **Step 1: Write the failing test** (append to `crud_tests_update.rs`; `setup_db`, `create`, `update` and the `EntityInput { …, ..Default::default() }` idiom already exist in this file):

```rust
#[tokio::test]
async fn update_marks_codex_stale() {
    let db = setup_db().await;
    let node = create(
        &db,
        Some("camp1"),
        None,
        EntityKind::Npc,
        EntityInput {
            name: "Mira".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    update(
        &db,
        &node.id,
        EntityKind::Npc,
        EntityInput {
            name: "Mira".to_string(),
            notes: Some("She now runs the Gilded Flagon.".to_string()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    #[derive(serde::Deserialize)]
    struct Row {
        codex_stale: bool,
    }
    let mut resp = db
        .query("SELECT codex_stale FROM type::thing('npc', $id)")
        .bind(("id", node.id.clone()))
        .await
        .unwrap();
    let rows: Vec<Row> = resp.take(0).unwrap();
    assert!(rows[0].codex_stale, "user edits must mark the article stale");
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p chronacle-extraction update_marks_codex_stale
```

Expected: FAIL — `codex_stale` still false.

- [ ] **Step 3: Implement** — in the `UPDATE` statement in `update.rs`, add one line to the `SET` list (before `updated_at`):

```
            codex_stale    = true,
```

- [ ] **Step 4: Run to verify it passes, then the crate suite**

```bash
cargo test -p chronacle-extraction
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/chronacle-extraction/src/entity_service/crud
git commit -m "feat(entity): user edits mark codex article stale"
```

### Task 18: Extraction staleness + scope-violation lint fallback

**Files:**
- Modify: `crates/chronacle-extraction/src/extraction_service/persist.rs`
- Test: `crates/chronacle-extraction/src/extraction_service/collection_tests.rs`

**Interfaces:**
- Consumes: `codex_service::{mark_entity_stale, record_lint}` (Task 14); `EntityError::ScopeViolation` (Task 16); `setup_db_with_collection` + `MockLlm` + `MockEmbeddingProvider` from `extraction_service::test_support`.
- Produces: `persist::handle_relate_error(db, collection_id, origin, target, err)` — the lint-not-fail policy for extraction bulk paths, reused by seed extraction if it ever produces cross-scope edges.

Context: `extract_from_collection` looks relation targets up **within the swept collection**, so a `ScopeViolation` cannot arise from that path today — the fallback is defensive (seed/enrich paths span collections). It is therefore extracted into a helper and unit-tested directly rather than through a contorted end-to-end arrangement.

- [ ] **Step 1: Write the failing tests** (append to `collection_tests.rs`):

```rust
#[tokio::test]
async fn extraction_marks_touched_entities_stale() {
    let (db, col_id) = setup_db_with_collection().await;
    // Pre-existing entity → the mock LLM re-emitting it takes the dedup path.
    entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Faction,
        EntityInput {
            name: "The Iron Fist".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let llm: Arc<dyn LlmProvider> = Arc::new(MockLlm {
        response: r#"{
            "entities": [{
                "name": "The Iron Fist",
                "kind": "faction",
                "summary": "Militant faction.",
                "notes": null,
                "relations": []
            }]
        }"#
        .to_string(),
    });
    let embed: Arc<dyn EmbeddingProvider> = Arc::new(MockEmbeddingProvider::new(768));
    extract_from_collection(&db, &llm, &embed, &col_id, |_| {})
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let mut resp = db
        .query("SELECT count() FROM faction WHERE codex_stale = true GROUP ALL")
        .await
        .unwrap();
    let counts: Vec<C> = resp.take(0).unwrap();
    assert_eq!(
        counts.first().map(|c| c.count).unwrap_or(0),
        1,
        "extraction touching an entity must mark its article stale"
    );
}

#[tokio::test]
async fn scope_violation_during_extraction_is_linted_not_fatal() {
    let (db, col_id) = setup_db_with_collection().await;
    let a = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "A".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let b = entity_service::create(
        &db,
        None,
        Some(&col_id),
        EntityKind::Npc,
        EntityInput {
            name: "B".to_string(),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    crate::extraction_service::persist::handle_relate_error(
        &db,
        &col_id,
        &a,
        &b,
        entity_service::EntityError::ScopeViolation {
            from: format!("npc:{}", a.id),
            to: format!("npc:{}", b.id),
        },
    )
    .await;

    #[derive(serde::Deserialize)]
    struct C {
        count: i64,
    }
    let mut resp = db
        .query(
            "SELECT count() FROM lint_finding WHERE kind = 'scope_violation' \
               AND resolved_at = NONE GROUP ALL",
        )
        .await
        .unwrap();
    let counts: Vec<C> = resp.take(0).unwrap();
    assert_eq!(counts.first().map(|c| c.count).unwrap_or(0), 1);
}
```

Adjust the `use` lines at the top of `collection_tests.rs` to also import `EntityKind` (already imported) and the `persist` path if module resolution differs (`persist` is a private sibling module — reachable from within the `extraction_service` module tree).

- [ ] **Step 2: Run to verify failures**

```bash
cargo test -p chronacle-extraction collection_tests
```

Expected: both FAIL (`codex_stale` never set; `handle_relate_error` not defined).

- [ ] **Step 3: Implement in `persist.rs`**

(a) Staleness — in `persist_batch`, after `origin_node` is resolved (after the `if let Some… else …` at line ~88) and after `rel_node` is resolved (line ~138), add:

```rust
        if let Err(e) =
            crate::codex_service::mark_entity_stale(db, &origin_node.kind, &origin_node.id).await
        {
            eprintln!("extraction: failed to mark {} stale: {e}", origin_node.name);
        }
```

(and the same for `rel_node`).

(b) Lint fallback — add the helper and route the existing error arm through it. Replace the `Err(e) => eprintln!(…)` arm of the `match result` on `relate_collapsing` (line ~182-191) with:

```rust
                Err(e) => handle_relate_error(db, collection_id, &origin_node, &rel_node, e).await,
```

and add to `persist.rs` (with `use crate::entity_service::EntityError;` in the imports):

```rust
/// Handle a failed relate on an extraction bulk path.
///
/// Scope violations are recorded as `scope_violation` lint findings and the
/// edge is skipped — extraction must never fail the whole run over one edge
/// (ADR-009). All other errors are logged, matching prior behaviour.
pub(super) async fn handle_relate_error<C: Connection>(
    db: &surrealdb::Surreal<C>,
    collection_id: &str,
    origin: &GraphNode,
    target: &GraphNode,
    err: EntityError,
) {
    match err {
        EntityError::ScopeViolation { from, to } => {
            let payload = serde_json::json!({
                "edge": serde_json::Value::Null,
                "from": from,
                "to": to,
                "from_collection": collection_id,
                "to_collection": serde_json::Value::Null,
            });
            if let Err(e) =
                crate::codex_service::record_lint(db, "scope_violation", payload).await
            {
                eprintln!("extraction: failed to record scope_violation lint: {e}");
            }
        }
        e => eprintln!(
            "extraction: failed to relate {} -> {}: {e}",
            origin.name, target.name
        ),
    }
}
```

- [ ] **Step 4: Run the crate + workspace suites**

```bash
cargo test -p chronacle-extraction && cargo test --workspace
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/chronacle-extraction/src/extraction_service
git commit -m "feat(extraction): staleness marking + scope-violation linting"
```

### Task 19: Verify, push, PR (A2b)

- [ ] **Step 1: Full verification**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace && pnpm -C apps/desktop typecheck && pnpm -C apps/desktop lint && pnpm -C apps/desktop test:run && pnpm -C apps/desktop run e2e:backend
```

Expected: all PASS.

- [ ] **Step 2: Push and open PR**

```bash
git push -u origin feat/a2b-staleness-scope
gh pr create --title "feat: codex staleness producers + reference-scope rules (A2b)" --body "$(cat <<'EOF'
## What
codex_service skeleton (mark_entity_stale, record_lint); staleness marking on ingest-done, entity edits, and extraction touches; reference-scope validation on every relates_to write with lint-not-fail fallback on extraction bulk paths.

## Why
Codex spec PR-A2b (ADR-009 reference rules). Spec: docs/superpowers/specs/2026-07-03-codex-compiled-world-model-design.md.

## Tested
cargo test --workspace (scope matrix, staleness producers, extraction lint fallback — backend-only BDD scenarios per features/README.md convention).

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Execution notes

- PR order: A0 → A1b → (A2a → A2b). A1b needs A0 merged (its `.feature` runs on A0's tooling). A2a/A2b are independent of A1b.
- Review checkpoint after every task; per-PR code review (`code-reviewer` agent) before push.
- If SurrealDB syntax in Tasks 15/16 misbehaves (e.g. `RETURN` object deserialization or `UPDATE $ents`), the fallback is splitting into separate `.query()` calls with `resp.take::<Vec<Thing>>(…)` — keep the same function signatures and tests; the tests are the contract.
