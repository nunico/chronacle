import { expect } from '@playwright/test';
import { When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

interface IpcCall {
  cmd: string;
  args?: Record<string, unknown>;
}

Then('the collection {string} shows the codex badge {string}', async ({ page }, name: string, badge: string) => {
  // Re-install mock with collection data for the codex scenario
  await installIpcMock(page, {
    get_collections: [{ id: 'col1', name: 'World Guide', description: null }],
    get_campaign_collections: [{ id: 'col1', name: 'World Guide', description: null }],
    get_codex_status: {
      stale_entities: 12,
      total_entities: 40,
      rules_stale: 0,
      rule_entries: 0,
    },
  });

  // Navigate to apply the new mock overrides
  await page.goto('/');
  // Re-open campaign manager with the new mock data
  await page.locator('button[title="Manage campaign and source collections"]').click();
  await page.getByText('Manage campaigns').click();

  // Assert the badge is visible
  const coll = page.locator('.coll', { hasText: name });
  await expect(coll.locator('.codex-badge')).toHaveText(badge);
});

When('the GM clicks compile on the collection {string}', async ({ page }, name: string) => {
  const coll = page.locator('.coll', { hasText: name });
  await coll.locator('button[aria-label*="Compile"]').click();
});

Then('the compile command is sent for the collection {string}', async ({ page }, _name: string) => {
  const calls = await page.evaluate(
    () => (window as unknown as { __ipcCalls: Array<{ cmd: string; args?: Record<string, unknown> }> }).__ipcCalls,
  );
  const compileCall = calls.find((c) => c.cmd === 'compile_collection' && c.args?.collectionId === 'col1');
  expect(compileCall).toBeDefined();
});

When('the GM opens the rules tab of collection {string}', async ({ page }, name: string) => {
  // Re-install mock with collection data so the manager renders "World Guide".
  await installIpcMock(page, {
    get_collections: [{ id: 'col1', name, description: null }],
    get_campaign_collections: [{ id: 'col1', name, description: null }],
  });

  await page.goto('/');
  await page.locator('button[title="Manage campaign and source collections"]').click();
  await page.getByText('Manage campaigns').click();

  const coll = page.locator('.coll', { hasText: name });
  await coll.locator('.coll-head').click();
  await coll.getByRole('tab', { name: 'Rules' }).click();
});

Then(
  'the rules list shows {string} under the {string} category',
  async ({ page }, entryName: string, category: string) => {
    const heading = page.getByRole('heading', { name: category, exact: false });
    await expect(heading).toBeVisible();
    const group = heading.locator('xpath=..');
    await expect(group.getByText(entryName)).toBeVisible();
  },
);

Then('the entry {string} cites {string}', async ({ page }, entryName: string, citation: string) => {
  const item = page.locator('.entry-item', { hasText: entryName });
  await item.locator('.entry-name').click();
  await expect(item.locator('.page-refs')).toContainText(citation);
});

When('the GM opens the rule entry {string}', async ({ page }, entryName: string) => {
  const item = page.locator('.entry-item', { hasText: entryName });
  await item.locator('.entry-name').click();
});

When('the GM submits the objection {string}', async ({ page }, objection: string) => {
  await page.getByText(/Redo with objections/).click();
  await page.getByLabel('Objection').fill(objection);
  await page.getByRole('button', { name: 'Submit', exact: true }).click();
});

// Entry names in the Gherkin map to the ipc-mock fixture ids.
const RULE_ENTRY_IDS: Record<string, string> = { Initiative: 'rule1' };

Then('a redo command is sent for the entry {string}', async ({ page }, entryName: string) => {
  const calls = await page.evaluate(() => (window as unknown as { __ipcCalls: IpcCall[] }).__ipcCalls);
  const redoCall = calls.find(
    (c) => c.cmd === 'redo_rule_entry' && c.args?.id === RULE_ENTRY_IDS[entryName],
  );
  expect(redoCall).toBeDefined();
});
