import { expect } from '@playwright/test';
import { resetDraftReliabilityIpcMock } from '../ipc-mock';
import { Given, Then, When } from './fixtures';
import {
  CAMPAIGN_A_QUESTION,
  type DraftWindow,
  composer,
  openCampaign,
  openRailView,
  submissions,
} from './draft-reliability-support';

const QUESTION = 'Where is the Moon Gate?';
const CAMPAIGN_B_QUESTION = 'Campaign B separate question';
const NO_CAMPAIGN_QUESTION = 'A question without a campaign';

Given('I have entered an Oracle question without sending it', async ({ page }) => {
  await composer(page).fill(QUESTION);
});

When('I open the campaign notebook', async ({ page }) => {
  await openRailView(page, 'Campaign & sources');
});

When('I return to Oracle', async ({ page }) => {
  await openRailView(page, 'Oracle');
});

Then('my question is still in the composer', async ({ page }) => {
  await expect(composer(page)).toHaveValue(QUESTION);
});

Then('it has not been submitted', async ({ page }) => {
  expect(await submissions(page)).toHaveLength(0);
});

Given('I have an unsent question in campaign A', async ({ page }) => {
  await composer(page).fill(CAMPAIGN_A_QUESTION);
});

When('I switch to campaign B', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
});

Then("campaign A's question is not shown in campaign B", async ({ page }) => {
  await expect(composer(page)).not.toHaveValue(CAMPAIGN_A_QUESTION);
});

When('I enter a different unsent question in campaign B', async ({ page }) => {
  await composer(page).fill(CAMPAIGN_B_QUESTION);
});

When('I return to campaign A', async ({ page }) => {
  await openCampaign(page, 'Campaign A');
});

Then('its unsent question is restored', async ({ page }) => {
  await expect(composer(page)).toHaveValue(CAMPAIGN_A_QUESTION);
});

Then('neither question has been submitted', async ({ page }) => {
  expect(await submissions(page)).toHaveLength(0);
});

Given('no campaign is available', async ({ page }) => {
  await resetDraftReliabilityIpcMock(page, { noCampaign: true });
  expect(
    await page.evaluate(() => (window as unknown as DraftWindow).__draftReliability.seedMode()),
  ).toBe('no-campaign');
  await expect(page.getByRole('button', { name: 'Switch campaign' })).toContainText('No campaign');
  await openRailView(page, 'Oracle');
});

Given('I enter an Oracle question without sending it', async ({ page }) => {
  await composer(page).fill(NO_CAMPAIGN_QUESTION);
});

When('I open Settings', async ({ page }) => {
  await openRailView(page, 'Settings');
});

Then('my no-campaign question is still in the composer', async ({ page }) => {
  await expect(composer(page)).toHaveValue(NO_CAMPAIGN_QUESTION);
});
