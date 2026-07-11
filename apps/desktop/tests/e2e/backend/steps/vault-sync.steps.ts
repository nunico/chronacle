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

Given('a sync will report {int} exported and {int} failed', async ({ page }, exported: number, failed: number) => {
  await installIpcMock(page, {
    get_vault_path: '/Users/gm/Vault',
    vault_sync_now: {
      exported,
      unchanged: 0,
      adopted: 0,
      deferred_apply: 0,
      deferred_conflict: 0,
      deferred_delete: 0,
      failed,
    },
  });
});

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
