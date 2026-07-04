import { expect } from '@playwright/test';
import { When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

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
