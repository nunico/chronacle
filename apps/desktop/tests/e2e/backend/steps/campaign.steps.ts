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
