import { expect } from '@playwright/test';
import { Given, When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

interface IpcCall {
  cmd: string;
  args?: Record<string, unknown>;
}

async function getIpcCalls(page: import('@playwright/test').Page): Promise<IpcCall[]> {
  return page.evaluate(() => (window as unknown as { __ipcCalls: IpcCall[] }).__ipcCalls);
}

Given('no vault is configured', async ({ page }) => {
  await installIpcMock(page, { get_vault_path: null });
});

Given('a vault is configured at {string}', async ({ page }, path: string) => {
  await installIpcMock(page, { get_vault_path: path });
});

Given(
  'a sync will report {int} exported and {int} failed',
  async ({ page }, exported: number, failed: number) => {
    await installIpcMock(page, {
      get_vault_path: '/Users/gm/Vault',
      vault_sync_now: {
        exported,
        unchanged: 0,
        adopted: 0,
        applied: 0,
        conflicts: 0,
        resolved: 0,
        soft_deleted: 0,
        swept: 0,
        invalid: 0,
        failed,
      },
    });
  },
);

When('the GM opens Settings', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings' }).click();
});

When('the GM clicks {string}', async ({ page }, label: string) => {
  await page.getByRole('button', { name: label, exact: true }).click();
});

Then('the settings page shows {string}', async ({ page }, text: string) => {
  await expect(page.getByText(text, { exact: false }).first()).toBeVisible();
});

Then('the settings page shows the vault path {string}', async ({ page }, path: string) => {
  await expect(page.getByText(path, { exact: true })).toBeVisible();
});

Then('the {string} button is disabled', async ({ page }, label: string) => {
  await expect(page.getByRole('button', { name: label, exact: true })).toBeDisabled();
});

Then('the {string} button is enabled', async ({ page }, label: string) => {
  await expect(page.getByRole('button', { name: label, exact: true })).toBeEnabled();
});

Then('the set vault path command was sent with null', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const call = calls.find((c) => c.cmd === 'set_vault_path');
  expect(call?.args?.vaultPath).toBeNull();
});

// ── D4b: producer-dispatch scenarios ─────────────────────────────────────────

/** A minimal `GraphNode` for the entity list mock. */
function entityNode(name: string): Record<string, unknown> {
  return {
    id: 'npc1',
    kind: 'npc',
    campaign_id: 'camp1',
    name,
    summary: null,
    notes: 'Original notes.',
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

Given(
  'a vault is configured at {string} and an entity {string}',
  async ({ page }, _path: string, name: string) => {
    const node = entityNode(name);
    await installIpcMock(page, {
      get_vault_path: '/Users/gm/Vault',
      get_entities: [node],
      get_entity_counts: { npc: 1 },
      get_entity_relations: [],
      // Echo the edited input straight back so the UI settles after save.
      update_entity: node,
    });
  },
);

/** Open the NPCs manager and edit the one seeded entity. */
async function openEntityEditForm(page: import('@playwright/test').Page): Promise<void> {
  await page.goto('/');
  // The rail button's accessible name includes its icon and count, so match on
  // the "NPCs" label as a substring.
  await page.getByRole('button', { name: 'NPCs' }).click();
  await page.locator('button.entity-name').first().click();
  await expect(page.getByTestId('entity-form')).toBeVisible();
}

When("the GM edits that entity's notes to {string}", async ({ page }, notes: string) => {
  await openEntityEditForm(page);
  await page.locator('#ef-notes').fill(notes);
  await page.getByTestId('entity-form-submit').click();
});

When('the GM renames that entity to {string}', async ({ page }, newName: string) => {
  await openEntityEditForm(page);
  await page.getByTestId('entity-form-name').fill(newName);
  await page.getByTestId('entity-form-submit').click();
});

Then('an update entity command was sent with notes {string}', async ({ page }, notes: string) => {
  const calls = await getIpcCalls(page);
  const call = calls.find((c) => c.cmd === 'update_entity');
  expect(call, 'update_entity must be dispatched').toBeDefined();
  const input = call?.args?.input as Record<string, unknown> | undefined;
  expect(input?.notes).toBe(notes);
});

Then('an update entity command was sent with name {string}', async ({ page }, name: string) => {
  const calls = await getIpcCalls(page);
  const call = calls.find((c) => c.cmd === 'update_entity');
  expect(call, 'update_entity must be dispatched').toBeDefined();
  const input = call?.args?.input as Record<string, unknown> | undefined;
  expect(input?.name).toBe(name);
});

Given(
  'a vault is configured at {string} and a compiled collection {string}',
  async ({ page }, _path: string, name: string) => {
    await installIpcMock(page, {
      get_vault_path: '/Users/gm/Vault',
      get_collections: [{ id: 'col1', name, description: null }],
      get_campaign_collections: [{ id: 'col1', name, description: null }],
      get_codex_status: { stale_entities: 3, total_entities: 10, rules_stale: 0, rule_entries: 0 },
      compile_collection: { articles_compiled: 3, remaining_stale: 0 },
    });
  },
);

When('the GM recompiles the collection', async ({ page }) => {
  await page.goto('/');
  await page.locator('button[title="Manage campaign and source collections"]').click();
  await page.getByText('Manage campaigns').click();
  const coll = page.locator('.coll', { hasText: 'World Guide' });
  await coll.locator('button[aria-label*="Compile"]').click();
});

Then('exactly one compile collection command was sent', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const compiles = calls.filter((c) => c.cmd === 'compile_collection');
  expect(compiles).toHaveLength(1);
});
