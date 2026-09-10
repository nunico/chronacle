import { expect, type Locator, type Page } from '@playwright/test';
import { Given, Then, When } from './fixtures';
import {
  activeWrites,
  composer,
  hasCampaign,
  hold,
  observations,
  openCampaign,
  openRailView,
  rejectNext,
  requestApplicationExit,
  resolveNext,
} from './draft-reliability-support';

const CAMPAIGN_A_ENTITY_DRAFT = 'Campaign A entity draft retained for deletion.';
const CAMPAIGN_A_SESSION_DRAFT = 'Campaign A failed session draft';
const CAMPAIGN_B_ORACLE_DRAFT = 'Campaign B unrelated retained question';

function campaignDeleteDialog(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Delete campaign' });
}

async function openCampaignDeletion(page: Page, campaignName: string): Promise<void> {
  await openRailView(page, 'Campaign & sources');
  const manage = page.getByRole('button', { name: /Manage campaigns/ });
  if (!(await page.locator('.manage-body').isVisible())) await manage.click();
  const row = page.locator('.manage-row').filter({ hasText: campaignName });
  await row.getByRole('button', { name: 'Delete' }).click();
  await expect(campaignDeleteDialog(page)).toBeVisible();
}

Given(
  'campaign A contains an unsaved entity draft and a failed session draft',
  async ({ page }) => {
    await openRailView(page, 'NPCs');
    await page.getByRole('button', { name: 'Mira', exact: true }).click();
    await page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true })
      .fill(CAMPAIGN_A_ENTITY_DRAFT);

    await openRailView(page, 'Sessions');
    await page.getByRole('button', { name: /Ashes at Dawn/ }).click();
    const title = page.getByRole('textbox', { name: 'Name', exact: true });
    await title.fill(CAMPAIGN_A_SESSION_DRAFT);
    await rejectNext(page, 'update_session', {
      code: 'DATABASE',
      message: 'Campaign A session save failed.',
    });
    await title.blur();
    await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  },
);

Given('campaign B contains an unrelated Oracle draft', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  await openRailView(page, 'Oracle');
  await composer(page).fill(CAMPAIGN_B_ORACLE_DRAFT);
  await openCampaign(page, 'Campaign A');
});

When('I open deletion confirmation for campaign A', async ({ page }) => {
  await openCampaignDeletion(page, 'Campaign A');
});

Then('the campaign deletion discloses both at-risk drafts', async ({ page }) => {
  await expect(campaignDeleteDialog(page)).toContainText(
    'Deleting this campaign will discard 2 retained draft(s).',
  );
});

When('campaign A deletion fails', async ({ page }) => {
  await rejectNext(page, 'delete_campaign', 'Campaign deletion failed.');
  await campaignDeleteDialog(page)
    .getByRole('button', { name: 'Delete campaign and its notes' })
    .click();
  await expect(campaignDeleteDialog(page).getByRole('alert')).toContainText(
    'Campaign deletion failed.',
  );
});

Then('campaign A remains available', async ({ page }) => {
  expect(await hasCampaign(page, 'camp-a')).toBe(true);
  await expect(campaignDeleteDialog(page)).toBeVisible();
});

Then("campaign A's exact entity and session drafts remain available", async ({ page }) => {
  await campaignDeleteDialog(page).getByRole('button', { name: 'Cancel' }).click();
  await openRailView(page, 'NPCs');
  await page.getByRole('button', { name: 'Mira', exact: true }).click();
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true }),
  ).toHaveValue(CAMPAIGN_A_ENTITY_DRAFT);
  await openRailView(page, 'Sessions');
  await page.getByRole('button', { name: new RegExp(CAMPAIGN_A_SESSION_DRAFT) }).click();
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CAMPAIGN_A_SESSION_DRAFT,
  );
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

Then("campaign B's Oracle draft remains unchanged", async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  await openRailView(page, 'Oracle');
  await expect(composer(page)).toHaveValue(CAMPAIGN_B_ORACLE_DRAFT);
});

Given('campaign A has an active session save', async ({ page }) => {
  await openRailView(page, 'Sessions');
  await page.getByRole('button', { name: /Ashes at Dawn/ }).click();
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  await hold(page, 'update_session');
  await title.fill('Campaign A session save in progress');
  await title.blur();
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(1);
});

Then('both campaign deletion choices are unavailable', async ({ page }) => {
  const dialog = campaignDeleteDialog(page);
  await expect(
    dialog.getByRole('button', { name: 'Delete campaign and its notes' }),
  ).toBeDisabled();
  await expect(dialog.getByRole('button', { name: 'Keep notes' })).toBeDisabled();
});

Then('I am told to wait for campaign saves to finish', async ({ page }) => {
  await expect(campaignDeleteDialog(page)).toContainText(
    'Wait for campaign saves to finish before deleting.',
  );
});

When('the campaign session save succeeds', async ({ page }) => {
  await resolveNext(page, 'update_session');
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(0);
});

Then('both campaign deletion choices become available', async ({ page }) => {
  const dialog = campaignDeleteDialog(page);
  await expect(dialog.getByRole('button', { name: 'Delete campaign and its notes' })).toBeEnabled();
  await expect(dialog.getByRole('button', { name: 'Keep notes' })).toBeEnabled();
});

When('I confirm deleting campaign A and its notes', async ({ page }) => {
  await campaignDeleteDialog(page)
    .getByRole('button', { name: 'Delete campaign and its notes' })
    .click();
});

Then('campaign A is deleted', async ({ page }) => {
  await expect.poll(() => hasCampaign(page, 'camp-a')).toBe(false);
  await expect(campaignDeleteDialog(page)).toHaveCount(0);
});

Then("campaign A's retained drafts are removed", async ({ page }) => {
  const deletes = await observations(page, 'delete_campaign');
  expect(deletes).toEqual([{ id: 'camp-a', onOwnedCollection: 'delete' }]);
});

Then('campaign B remains available with its Oracle draft unchanged', async ({ page }) => {
  expect(await hasCampaign(page, 'camp-b')).toBe(true);
  await openCampaign(page, 'Campaign B');
  await openRailView(page, 'Oracle');
  await expect(composer(page)).toHaveValue(CAMPAIGN_B_ORACLE_DRAFT);
});

Then('no removed campaign draft blocks a later close', async ({ page }) => {
  await composer(page).fill('');
  await requestApplicationExit(page, 84);
  await expect(page.getByRole('dialog', { name: 'Unsaved changes' })).toHaveCount(0);
});
