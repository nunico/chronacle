import { expect, type Page } from '@playwright/test';
import { Given, When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

// ── Backend-only invariants ──────────────────────────────────────────────
// The mocked-IPC backend suite has no real filesystem or database, so a GM
// edit "in the vault" cannot actually happen here. Each step below arranges
// the mock to reflect what a real `vault_sync_now` would report and what a
// subsequent refetch would read back, so these scenarios prove the UI's half
// of the contract only: it dispatches the sync, and it renders whatever the
// backend returns. See the `# backend:` comments in vault-inbound.feature for
// the Rust tests that prove the deep half (the actual file write, fence
// revert, sidecar bytes, and DB round-trip).
//
// NOTE on causality: unlike vault-sync.steps.ts (D4b), which asserts the
// *arguments* of a dispatched IPC call — a check that is causal by
// construction — several scenarios here can only assert *rendered mock
// echo* (what get_entities / list_vault_conflicts return on a later read).
// A naive single `installIpcMock` call bakes that "post-sync" data in at
// the very first `page.goto('/')`, before the sync ever runs, which makes
// the assertion pass even with the sync button wired to a no-op. Where that
// applies, the `When` step below passes a `postSyncOverrides` argument to
// `installIpcMock`: the mock only starts serving that data once it has
// itself observed a `vault_sync_now` dispatch, so the `Then` step's
// rendered-echo check is causally gated on the click, not merely coincident
// with it.

const VAULT_PATH = '/Users/gm/Vault';
const ORIGINAL_NOTES = 'Original notes.';
const EDITED_NOTES = 'Edited notes from Obsidian.';
const VAULT_VERSION_NOTES = 'Vault-side edit, kept after resolution.';
const NEW_VAULT_PATH = '/Users/gm/NewVault';

interface IpcCall {
  cmd: string;
  args?: Record<string, unknown>;
}

async function getIpcCalls(page: Page): Promise<IpcCall[]> {
  return page.evaluate(() => (window as unknown as { __ipcCalls: IpcCall[] }).__ipcCalls);
}

/** A minimal `GraphNode` for the entity list mock, matching vault-sync.steps.ts. */
function entityNode(name: string, notes: string): Record<string, unknown> {
  return {
    id: 'npc1',
    kind: 'npc',
    campaign_id: 'camp1',
    name,
    summary: null,
    notes,
    created_at: null,
    updated_at: null,
    date_start: null,
    date_end: null,
    is_ongoing: null,
    sequence_index: null,
    era: null,
    duration_label: null,
    session_id: null,
    player_name: null,
    character_class: null,
    character_level: null,
    status: null,
    codex_article: null,
    codex_stale: null,
    codex_compiled_at: null,
  };
}

function conflictEntry(name: string): Record<string, unknown> {
  return {
    id: 'npc1',
    kind: 'npc',
    name,
    key: 'campaigns/sov/entities/npc/seraphina-aldric.md',
    sidecarKey: 'campaigns/sov/entities/npc/seraphina-aldric.conflict.md',
  };
}

function defaultReport(): Record<string, number> {
  return {
    exported: 0,
    unchanged: 0,
    adopted: 0,
    applied: 0,
    conflicts: 0,
    resolved: 0,
    soft_deleted: 0,
    swept: 0,
    invalid: 0,
    failed: 0,
  };
}

/** Remembers the entity name a parameterless step needs, across steps. */
const names = new WeakMap<Page, string>();

/** Open the NPCs manager and edit the one seeded entity. */
async function openEntityEditForm(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'NPCs' }).click();
  await page.locator('button.entity-name').first().click();
  await expect(page.getByTestId('entity-form')).toBeVisible();
}

Given('a synced vault with an entity {string}', async ({ page }, name: string) => {
  names.set(page, name);
  await installIpcMock(page, {
    get_vault_path: VAULT_PATH,
    get_entities: [entityNode(name, ORIGINAL_NOTES)],
    get_entity_counts: { npc: 1 },
    get_entity_relations: [],
    list_vault_conflicts: [],
    vault_sync_now: defaultReport(),
  });
});

Given('an entity {string} frozen in conflict', async ({ page }, name: string) => {
  names.set(page, name);
  await installIpcMock(page, {
    get_vault_path: VAULT_PATH,
    get_entities: [entityNode(name, ORIGINAL_NOTES)],
    get_entity_counts: { npc: 1 },
    get_entity_relations: [],
    list_vault_conflicts: [conflictEntry(name)],
    vault_sync_now: defaultReport(),
  });
});

When('the GM edits the notes of {string} in the vault', async ({ page }, name: string) => {
  // External vault edit: the mock still serves the ORIGINAL notes until it
  // observes vault_sync_now, so "Then the entity has the edited notes" fails
  // if the sync click is removed (see the module-level causality note).
  await installIpcMock(
    page,
    {
      get_vault_path: VAULT_PATH,
      get_entities: [entityNode(name, ORIGINAL_NOTES)],
      get_entity_counts: { npc: 1 },
      get_entity_relations: [],
      list_vault_conflicts: [],
      vault_sync_now: { ...defaultReport(), applied: 1 },
    },
    {
      get_entities: [entityNode(name, EDITED_NOTES)],
    },
  );
});

When('the GM edits inside the compiled block of {string}', async ({ page }, name: string) => {
  // A pure fence edit changes no GM-owned field, so a clean reconcile settles
  // as unchanged — no conflict, no failure.
  await installIpcMock(page, {
    get_vault_path: VAULT_PATH,
    get_entities: [entityNode(name, ORIGINAL_NOTES)],
    get_entity_counts: { npc: 1 },
    get_entity_relations: [],
    list_vault_conflicts: [],
    vault_sync_now: { ...defaultReport(), unchanged: 1 },
  });
});

When(
  'both Chronacle and the vault file of {string} are edited differently',
  async ({ page }, name: string) => {
    // No conflict is listed until the mock observes vault_sync_now, so "Then
    // ... list X as a conflict" fails if the sync click is removed — that
    // conflict row is otherwise loaded by a mount-time effect independent of
    // the click (see the module-level causality note).
    await installIpcMock(
      page,
      {
        get_vault_path: VAULT_PATH,
        get_entities: [entityNode(name, ORIGINAL_NOTES)],
        get_entity_counts: { npc: 1 },
        get_entity_relations: [],
        list_vault_conflicts: [],
        vault_sync_now: { ...defaultReport(), conflicts: 1 },
      },
      {
        list_vault_conflicts: [conflictEntry(name)],
      },
    );
  },
);

When('the GM deletes the conflict sidecar', async ({ page }) => {
  const name = names.get(page) ?? 'Seraphina Aldric';
  // The GM's resolution signal. Deleting the sidecar changes nothing on its
  // own: until a sync runs, the record is still frozen in conflict and still
  // carries Chronacle's version. Only once the mock observes vault_sync_now
  // does it serve the resolved state (conflict cleared, vault's version
  // adopted) — so both `Then` steps fail if the sync click is removed (see the
  // module-level causality note).
  await installIpcMock(
    page,
    {
      get_vault_path: VAULT_PATH,
      get_entities: [entityNode(name, ORIGINAL_NOTES)],
      get_entity_counts: { npc: 1 },
      get_entity_relations: [],
      list_vault_conflicts: [conflictEntry(name)],
      vault_sync_now: { ...defaultReport(), resolved: 1 },
    },
    {
      get_entities: [entityNode(name, VAULT_VERSION_NOTES)],
      list_vault_conflicts: [],
    },
  );
});

When('the GM deletes the vault file of {string}', async ({ page }, name: string) => {
  names.set(page, name);
  // Soft-deleted entities are hidden from every read path (E5). The mock
  // still lists the entity until it observes vault_sync_now, so "Then X is
  // no longer visible" fails if the sync click is removed — get_entities is
  // otherwise loaded by a mount-time effect independent of the click (see
  // the module-level causality note).
  await installIpcMock(
    page,
    {
      get_vault_path: VAULT_PATH,
      get_entities: [entityNode(name, ORIGINAL_NOTES)],
      get_entity_counts: { npc: 1 },
      get_entity_relations: [],
      list_vault_conflicts: [],
      vault_sync_now: { ...defaultReport(), soft_deleted: 1 },
    },
    {
      get_entities: [],
      get_entity_counts: { npc: 0 },
    },
  );
});

When('a sync runs', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('button', { name: 'Sync now', exact: true }).click();
  // Wait for the settled report — proves vault_sync_now was dispatched.
  await expect(page.locator('.reindex-success, .reindex-error')).toBeVisible();
});

When('the vault path is changed to a new empty folder', async ({ page }) => {
  const name = names.get(page) ?? 'Seraphina Aldric';
  await installIpcMock(page, {
    get_vault_path: VAULT_PATH,
    get_entities: [entityNode(name, ORIGINAL_NOTES)],
    get_entity_counts: { npc: 1 },
    get_entity_relations: [],
    list_vault_conflicts: [],
    vault_sync_now: defaultReport(),
    'plugin:dialog|open': NEW_VAULT_PATH,
    set_vault_path: null,
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('button', { name: 'Choose folder…', exact: true }).click();
  await expect(page.getByText(NEW_VAULT_PATH, { exact: true })).toBeVisible();
});

Then('the entity {string} has the edited notes in Chronacle', async ({ page }, _name: string) => {
  await openEntityEditForm(page);
  await expect(page.locator('#ef-notes')).toHaveValue(EDITED_NOTES);
});

Then(
  'the vault file of {string} shows the compiled text again',
  async ({ page }, _name: string) => {
    // Only the sync trigger and a clean (non-conflict, non-failure) report
    // are observable via mocked IPC; the byte-for-byte fence revert on disk
    // is proven in Rust (see the feature file's backend comment).
    const calls = await getIpcCalls(page);
    expect(calls.some((c) => c.cmd === 'vault_sync_now')).toBe(true);
    await expect(page.getByText('1 unchanged', { exact: false })).toBeVisible();
    expect(await page.locator('.conflicts-section').count()).toBe(0);
  },
);

Then('a conflict sidecar exists for {string}', async ({ page }, name: string) => {
  // The sidecar file itself is filesystem-only; the reconcile report and the
  // conflict row's sidecarKey are the proxy the mocked suite can check. The
  // sidecar's bytes on disk are proven in Rust (see the feature file).
  await expect(page.getByText('1 conflicts', { exact: false })).toBeVisible();
  const row = page.locator('.conflict-row', { hasText: name });
  await expect(row).toBeVisible();
  await expect(row.getByText('.conflict.md', { exact: false })).toBeVisible();
});

Then('the vault sync settings list {string} as a conflict', async ({ page }, name: string) => {
  await expect(page.locator('.conflict-row', { hasText: name })).toBeVisible();
});

Then('the entity {string} has the vault version in Chronacle', async ({ page }, _name: string) => {
  await openEntityEditForm(page);
  await expect(page.locator('#ef-notes')).toHaveValue(VAULT_VERSION_NOTES);
});

Then('no conflict is listed for {string}', async ({ page }, _name: string) => {
  // The previous `Then` left the entity form open in the manager view, where a
  // conflicts list could never render anyway — so assert against the vault
  // panel itself: close the form, go back to Settings, and require the freshly
  // re-fetched conflict list to be empty. Before the sync, that same panel
  // still lists the frozen record, so this fails if the sync never ran.
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.locator('form[aria-label="entity form"]')).toBeHidden();
  await page.getByRole('button', { name: 'Settings' }).click();
  await expect(page.getByRole('heading', { name: 'Markdown vault' })).toBeVisible();
  await expect(page.locator('.conflicts-section')).toHaveCount(0);
});

Then('{string} is no longer visible in Chronacle', async ({ page }, name: string) => {
  await page.getByRole('button', { name: 'NPCs' }).click();
  await expect(page.getByText(name, { exact: true })).not.toBeVisible();
});

Then('{string} is still visible in Chronacle', async ({ page }, name: string) => {
  // Switching folders must never delete a record — proven directly by the
  // absence of any delete-shaped IPC call.
  const calls = await getIpcCalls(page);
  expect(calls.some((c) => c.cmd === 'soft_delete_entity')).toBe(false);
  await page.getByRole('button', { name: 'NPCs' }).click();
  await expect(page.getByText(name, { exact: true })).toBeVisible();
});

Then('the new folder contains a file for {string}', async ({ page }, _name: string) => {
  // Filesystem-only: the new folder's file layout is proven in Rust (see the
  // feature file's backend comment). The mocked suite can only prove the UI
  // dispatched the new path.
  const calls = await getIpcCalls(page);
  const call = calls.find((c) => c.cmd === 'set_vault_path');
  expect(call?.args?.vaultPath).toBe(NEW_VAULT_PATH);
});
