import { expect } from '@playwright/test';
import { Given } from './fixtures';
import { resetDraftReliabilityIpcMock } from '../ipc-mock';
import { type DraftWindow } from './draft-reliability-support';

Given('draft reliability test data is available', async ({ page }) => {
  await resetDraftReliabilityIpcMock(page);
});

Given('I have opened Chronacle in campaign {string}', async ({ page }, campaignName: string) => {
  expect(
    await page.evaluate(() => (window as unknown as DraftWindow).__draftReliability.seedMode()),
  ).toBe('campaigns');
  await expect(page.getByRole('button', { name: 'Switch campaign' })).toContainText(campaignName);
});
