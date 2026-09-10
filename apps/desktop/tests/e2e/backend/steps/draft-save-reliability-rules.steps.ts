import { expect, type Locator, type Page } from '@playwright/test';
import { Given, Then, When } from './fixtures';
import {
  acknowledgeNextRuleAs,
  activeWrites,
  hold,
  maxConcurrentWrites,
  observations,
  openCampaign,
  openRailView,
  pendingWrites,
  persisted,
  rejectNext,
  removeRule,
  resolveNext,
} from './draft-reliability-support';

const RULE_DRAFT = 'Roll initiative once for each side.';
const EARLIER_RULE_DRAFT = 'Earlier initiative ruling.';
const NEWER_RULE_DRAFT = 'Newer initiative ruling that remains local.';
const CAMPAIGN_B_RULE_DRAFT = 'Campaign B queued initiative ruling.';
const CAMPAIGN_A_LATEST_RULE_DRAFT = 'Campaign A newest initiative ruling.';
const FOCUSED_RULE_DRAFT = 'Unsaved note';
const NO_CAMPAIGN_RULE_DRAFT = 'No-campaign rule note';
const WORLD_GUIDE_RULE_DRAFT = 'World Guide retained initiative ruling.';
const RAPID_RULE_NOTES = ['First rapid ruling.', 'Second rapid ruling.', 'Newest rapid ruling.'];
const KEYBOARD_EDITED_RULE_DRAFT = `${FOCUSED_RULE_DRAFT}go`;
const ADVENTURER_INITIATIVE_SAVED_NOTE = 'Adventurer initiative saved note.';

When('I return to the Initiative rule with the keyboard', async ({ page }) => {
  const campaign = page.getByRole('button', { name: 'Campaign & sources' });
  await campaign.focus();
  await campaign.press('Enter');
  const collection = page.locator('.coll').filter({
    has: page.getByText('World Guide', { exact: true }),
  });
  const rulesTab = collection.getByRole('tab', { name: 'Rules' });
  if (!(await rulesTab.isVisible())) {
    const header = collection.locator('.coll-head');
    await header.focus();
    await header.press('Enter');
  }
  if ((await rulesTab.getAttribute('aria-selected')) !== 'true') {
    await rulesTab.focus();
    await rulesTab.press('Enter');
  }
  const initiative = collection.getByRole('button', { name: 'Initiative', exact: true });
  if (!(await collection.getByRole('textbox', { name: 'Table notes' }).isVisible())) {
    await initiative.focus();
    await initiative.press('Enter');
  }
  await expect(collection.getByRole('textbox', { name: 'Table notes' })).toBeVisible();
});

When('I press the Oracle g chord in the focused rule-note field', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toBeFocused();
  await page.keyboard.press('g');
  await page.keyboard.press('o');
});

Then('I remain in the Initiative rule and the typed keys remain unsaved', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
    KEYBOARD_EDITED_RULE_DRAFT,
  );
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

When('I move focus to the Initiative redo control', async ({ page }) => {
  await page.getByRole('button', { name: /Redo with objections/ }).focus();
});

Then('the ordinary rule-note blur is saved', async ({ page }) => {
  await expect
    .poll(async () => {
      const rule = await persisted<{ notes: string }>(page, 'update_rule_notes', 'initiative');
      return rule?.notes;
    })
    .toBe(KEYBOARD_EDITED_RULE_DRAFT);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Then('the rule note is shown as saved', async ({ page }) => {
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  const rule = await persisted<{ notes: string }>(page, 'update_rule_notes', 'initiative');
  expect(rule?.notes).toBe(KEYBOARD_EDITED_RULE_DRAFT);
});

Then('the exact keyboard-edited rule note is restored', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
    KEYBOARD_EDITED_RULE_DRAFT,
  );
});

async function openRule(page: Page, name: string): Promise<Locator> {
  return openRuleInCollection(page, 'World Guide', name);
}

async function openRuleInCollection(
  page: Page,
  collectionName: string,
  ruleName: string,
): Promise<Locator> {
  await openRailView(page, 'Campaign & sources');
  const collection = page.locator('.coll').filter({
    has: page.getByText(collectionName, { exact: true }),
  });
  await expect(collection).toBeVisible();
  const rulesTab = collection.getByRole('tab', { name: 'Rules' });
  if (!(await rulesTab.isVisible())) {
    await collection.getByText(collectionName, { exact: true }).click();
  }
  await expect(rulesTab).toBeVisible();
  if ((await rulesTab.getAttribute('aria-selected')) !== 'true') await rulesTab.click();
  const notes = collection.getByRole('textbox', { name: 'Table notes' });
  if (!(await notes.isVisible())) {
    await collection.getByRole('button', { name: ruleName, exact: true }).click();
  }
  await expect(notes).toBeVisible();
  return notes;
}

Given('I have changed the table notes for rule {string}', async ({ page }, name: string) => {
  const notes = await openRule(page, name);
  await notes.fill(RULE_DRAFT);
});

When('the rule-note save fails', async ({ page }) => {
  await rejectNext(page, 'update_rule_notes', {
    code: 'DATABASE',
    message: 'Rule-note write failed.',
  });
  await page.getByRole('textbox', { name: 'Table notes' }).blur();
});

Then('the changed rule note remains available', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
});

Then('the rule note shows an actionable save failure', async ({ page }) => {
  await expect(page.getByRole('alert')).toContainText(/couldn't save|rule-note write failed/i);
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

When('I retry the rule-note save and it succeeds', async ({ page }) => {
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect
    .poll(async () => {
      const rule = await persisted<{ notes: string }>(page, 'update_rule_notes', 'initiative');
      return rule?.notes;
    })
    .toBe(RULE_DRAFT);
});

Then('the changed rule note is shown as saved', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Then('the rule-note failure indication is cleared', async ({ page }) => {
  await expect(page.getByRole('alert')).toHaveCount(0);
});

Given('a save of my changed rule note is in progress', async ({ page }) => {
  const notes = await openRule(page, 'Initiative');
  await hold(page, 'update_rule_notes');
  await notes.fill(RULE_DRAFT);
  await notes.blur();
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(1);
});

When('I navigate to Oracle before the rule-note save completes', async ({ page }) => {
  await openRailView(page, 'Oracle');
  expect(await activeWrites(page, 'update_rule_notes')).toBe(1);
});

When('I return to the Initiative rule', async ({ page }) => {
  await openRule(page, 'Initiative');
});

Then('the changed rule note is preserved and shown as saving', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
  await expect(page.getByText('Saving…', { exact: true })).toBeVisible();
});

Given(
  'I have entered {string} in the focused Initiative table notes',
  async ({ page }, note: string) => {
    expect([FOCUSED_RULE_DRAFT, NO_CAMPAIGN_RULE_DRAFT]).toContain(note);
    const notes = await openRule(page, 'Initiative');
    await notes.fill(note);
    await expect(notes).toBeFocused();
  },
);

When('I navigate to Oracle while the rule note is still focused', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toBeFocused();
  await openRailView(page, 'Oracle');
});

Then('no rule-note save has been sent', async ({ page }) => {
  expect(await observations(page, 'update_rule_notes')).toHaveLength(0);
});

Then('the exact rule note {string} is restored', async ({ page }, note: string) => {
  expect([FOCUSED_RULE_DRAFT, NO_CAMPAIGN_RULE_DRAFT]).toContain(note);
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(note);
});

Then('the rule note is shown as unsaved', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
  await expect(page.getByText('Saved', { exact: true })).toHaveCount(0);
});

When('I restore the saved Initiative table note', async ({ page }) => {
  await page.getByRole('textbox', { name: 'Table notes' }).fill('Initiative saved note.');
});

Then('the rule note no longer indicates unsaved changes', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toHaveCount(0);
});

Given('a World Guide Initiative rule-note save is in progress', async ({ page }) => {
  const notes = await openRuleInCollection(page, 'World Guide', 'Initiative');
  await hold(page, 'update_rule_notes');
  await notes.fill(WORLD_GUIDE_RULE_DRAFT);
  await notes.blur();
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(1);
});

When('I navigate to Oracle before the collection rule-note save completes', async ({ page }) => {
  await openRailView(page, 'Oracle');
  expect(await activeWrites(page, 'update_rule_notes')).toBe(1);
});

When('I open Initiative in the Adventurer Guide', async ({ page }) => {
  await openRuleInCollection(page, 'Adventurer Guide', 'Initiative');
});

Then('the World Guide rule-note draft is not shown in the Adventurer Guide', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).not.toHaveValue(
    WORLD_GUIDE_RULE_DRAFT,
  );
});

Then('the Adventurer Guide saved rule note is shown', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
    'Adventurer initiative saved note.',
  );
});

When('I return to Initiative in the World Guide', async ({ page }) => {
  await openRuleInCollection(page, 'World Guide', 'Initiative');
});

Then('the World Guide rule-note draft is preserved and shown as saving', async ({ page }) => {
  const worldGuide = page.locator('.coll').filter({
    has: page.getByText('World Guide', { exact: true }),
  });
  await expect(worldGuide.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
    WORLD_GUIDE_RULE_DRAFT,
  );
  await expect(worldGuide.getByText('Saving…', { exact: true })).toBeVisible();
});

When('I create campaign {string}', async ({ page }, campaignName: string) => {
  await openRailView(page, 'Campaign & sources');
  await page.getByRole('button', { name: /Manage campaigns/ }).click();
  await page.getByPlaceholder('New campaign name').fill(campaignName);
  await page.getByRole('button', { name: /Create/ }).click();
  await expect(page.getByRole('button', { name: 'Switch campaign' })).toContainText(campaignName);
});

When('I open Initiative in the World Guide', async ({ page }) => {
  await openRuleInCollection(page, 'World Guide', 'Initiative');
});

Then('the no-campaign rule-note draft is not shown in campaign A', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).not.toHaveValue(
    NO_CAMPAIGN_RULE_DRAFT,
  );
});

Then('the saved Initiative table note is shown', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
    'Initiative saved note.',
  );
});

When(
  'I delete campaign {string} and return to Initiative',
  async ({ page }, campaignName: string) => {
    const manage = page.getByRole('button', { name: /Manage campaigns/ });
    const campaignRow = page.locator('.manage-row').filter({ hasText: campaignName });
    if (!(await campaignRow.isVisible())) await manage.click();
    await campaignRow.getByRole('button', { name: 'Delete' }).click();
    const dialog = page.getByRole('dialog', { name: 'Delete campaign' });
    await dialog.getByRole('button', { name: 'Keep notes as a regular collection' }).click();
    await expect(page.getByRole('button', { name: 'Switch campaign' })).toContainText(
      'No campaign',
    );
    await openRuleInCollection(page, 'World Guide', 'Initiative');
  },
);

Then("campaign A's changed rule note is not shown in campaign B", async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
    'Initiative saved note.',
  );
  await expect(page.getByText(RULE_DRAFT, { exact: true })).toHaveCount(0);
});

Given('a save of an earlier rule-note revision is in progress', async ({ page }) => {
  const notes = await openRule(page, 'Initiative');
  await hold(page, 'update_rule_notes');
  await notes.fill(EARLIER_RULE_DRAFT);
  await notes.blur();
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(1);
});

When('I make a newer edit to the rule note', async ({ page }) => {
  await page.getByRole('textbox', { name: 'Table notes' }).fill(NEWER_RULE_DRAFT);
});

When('the earlier rule-note save completes', async ({ page }) => {
  await resolveNext(page, 'update_rule_notes');
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(0);
});

Then('my newer rule-note edit remains intact', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(NEWER_RULE_DRAFT);
});

Then('the rule note is not incorrectly marked as saved', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
  await expect(page.getByText('Saved', { exact: true })).toHaveCount(0);
});

Given('saving my changed Initiative note is in progress', async ({ page }) => {
  const notes = await openRule(page, 'Initiative');
  await hold(page, 'update_rule_notes');
  await notes.fill(RULE_DRAFT);
  await notes.blur();
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(1);
});

When('that save is acknowledged as the Adventurer Guide Initiative rule', async ({ page }) => {
  await acknowledgeNextRuleAs(page, 'adventurer-initiative');
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(0);
});

Then('my changed Initiative note remains available and needs attention', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
  await expect(page.getByRole('alert')).toContainText(/couldn't save/i);
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  await expect(page.getByText('Saved', { exact: true })).toHaveCount(0);
});

Then('neither rule is overwritten by the wrong acknowledgment', async ({ page }) => {
  const initiative = await persisted<{ notes: string }>(page, 'update_rule_notes', 'initiative');
  const adventurerInitiative = await persisted<{ notes: string }>(
    page,
    'update_rule_notes',
    'adventurer-initiative',
  );
  expect(initiative?.notes).toBe('Initiative saved note.');
  expect(adventurerInitiative?.notes).toBe(ADVENTURER_INITIATIVE_SAVED_NOTE);
  const writes = await observations(page, 'update_rule_notes');
  expect(writes).toEqual([{ id: 'initiative', notes: RULE_DRAFT }]);
});

When('I retry the Initiative note and its acknowledgment succeeds', async ({ page }) => {
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect.poll(() => pendingWrites(page, 'update_rule_notes')).toBe(1);
  await resolveNext(page, 'update_rule_notes');
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(0);
});

Then('the changed Initiative note is saved to Initiative only', async ({ page }) => {
  const initiative = await persisted<{ notes: string }>(page, 'update_rule_notes', 'initiative');
  const adventurerInitiative = await persisted<{ notes: string }>(
    page,
    'update_rule_notes',
    'adventurer-initiative',
  );
  expect(initiative?.notes).toBe(RULE_DRAFT);
  expect(adventurerInitiative?.notes).toBe(ADVENTURER_INITIATIVE_SAVED_NOTE);
  const writes = await observations(page, 'update_rule_notes');
  expect(writes).toEqual([
    { id: 'initiative', notes: RULE_DRAFT },
    { id: 'initiative', notes: RULE_DRAFT },
  ]);
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

Given('rule-note saves are being held open', async ({ page }) => {
  await openRule(page, 'Initiative');
  await hold(page, 'update_rule_notes');
});

When('I request rapid saves for three different rule notes', async ({ page }) => {
  const notes = page.getByRole('textbox', { name: 'Table notes' });
  for (const value of RAPID_RULE_NOTES) {
    await notes.fill(value);
    await notes.blur();
  }
  await expect(notes).toHaveValue(RAPID_RULE_NOTES[2]);
});

Then('the rule-note save attempts do not overlap', async ({ page }) => {
  expect(await maxConcurrentWrites(page, 'update_rule_notes')).toBe(1);
});

When('the pending rule-note saves are acknowledged', async ({ page }) => {
  await resolveNext(page, 'update_rule_notes');
  await expect
    .poll(() => observations(page, 'update_rule_notes').then((writes) => writes.length))
    .toBe(2);
  await expect.poll(() => pendingWrites(page, 'update_rule_notes')).toBe(1);
  await resolveNext(page, 'update_rule_notes');
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(0);
});

Then('the newest rule note is preserved', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RAPID_RULE_NOTES[2]);
});

Then('only the newest rule-note revision is shown as saved', async ({ page }) => {
  const rule = await persisted<{ notes: string }>(page, 'update_rule_notes', 'initiative');
  expect(rule?.notes).toBe(RAPID_RULE_NOTES[2]);
  const writes = await observations(page, 'update_rule_notes');
  expect(writes).toHaveLength(2);
  expect(writes[1]?.notes).toBe(RAPID_RULE_NOTES[2]);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

When('saving reports that rule {string} is no longer available', async ({ page }, name) => {
  expect(name).toBe('Initiative');
  await removeRule(page, 'initiative');
  await page.getByRole('textbox', { name: 'Table notes' }).blur();
});

Then('the rule note shows an actionable unavailable-target failure', async ({ page }) => {
  const alert = page.getByRole('alert');
  await expect(alert).toContainText('This record is no longer available.');
  await expect(alert).not.toContainText('Rule no longer available.');
  await expect(alert.getByRole('button', { name: 'Retry' })).toBeVisible();
});

When('I retry the unavailable rule-note save with the keyboard', async ({ page }) => {
  const retry = page.getByRole('button', { name: 'Retry' });
  await retry.focus();
  await retry.press('Enter');
  await expect
    .poll(() => observations(page, 'update_rule_notes').then((writes) => writes.length))
    .toBe(2);
});

Then('Retry still targets rule {string}', async ({ page }, name) => {
  expect(name).toBe('Initiative');
  const writes = await observations(page, 'update_rule_notes');
  expect(writes).toHaveLength(2);
  expect(writes.every((write) => write.id === 'initiative')).toBe(true);
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
});

Then('focus remains on the rule-note Retry action', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Retry' })).toBeFocused();
});

When('I navigate away and reload the rule list', async ({ page }) => {
  await openRailView(page, 'Oracle');
  await openRailView(page, 'Campaign & sources');
  const worldGuide = page.locator('.coll').filter({
    has: page.getByText('World Guide', { exact: true }),
  });
  await worldGuide.getByText('World Guide', { exact: true }).click();
  await worldGuide.getByRole('tab', { name: 'Rules' }).click();
});

Then('the omitted Initiative draft remains available as recovery-only', async ({ page }) => {
  const worldGuide = page.locator('.coll').filter({
    has: page.getByText('World Guide', { exact: true }),
  });
  const initiative = worldGuide.getByRole('button', { name: 'Initiative', exact: true });
  await expect(initiative).toBeVisible();
  await initiative.click();
  await expect(worldGuide.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
  await expect(worldGuide.getByRole('alert')).toBeVisible();
});

Then('the unavailable rule-note recovery is localized and actionable', async ({ page }) => {
  const alert = page.getByRole('alert');
  await expect(alert).toContainText('This record is no longer available.');
  await expect(alert.getByRole('button', { name: 'Retry' })).toBeVisible();
  await expect(alert.getByRole('button', { name: 'Discard changes' })).toBeVisible();
});

Then('the raw rule backend detail is not shown', async ({ page }) => {
  await expect(page.getByRole('alert')).not.toContainText('Rule no longer available.');
});

When('I retry the omitted rule-note save with the keyboard', async ({ page }) => {
  const retry = page.getByRole('button', { name: 'Retry' });
  await retry.focus();
  await retry.press('Enter');
  await expect
    .poll(() => observations(page, 'update_rule_notes').then((writes) => writes.length))
    .toBe(2);
});

Then('Retry uses the original Initiative target and content', async ({ page }) => {
  const writes = await observations(page, 'update_rule_notes');
  expect(writes).toHaveLength(2);
  expect(writes.every((write) => write.id === 'initiative')).toBe(true);
  expect(writes.every((write) => write.notes === RULE_DRAFT)).toBe(true);
  await expect(page.getByRole('textbox', { name: 'Table notes' })).toHaveValue(RULE_DRAFT);
});

When('I discard the omitted rule-note draft with the keyboard', async ({ page }) => {
  const discard = page.getByRole('button', { name: 'Discard changes' });
  await discard.focus();
  await discard.press('Enter');
});

Then('only the omitted Initiative draft is removed', async ({ page }) => {
  const worldGuide = page.locator('.coll').filter({
    has: page.getByText('World Guide', { exact: true }),
  });
  await expect(worldGuide.getByRole('button', { name: 'Initiative', exact: true })).toHaveCount(0);
  await expect(page.getByText('Adventurer Guide', { exact: true })).toBeVisible();
  expect(await observations(page, 'update_rule_notes')).toHaveLength(2);
});

Then('focus moves to the stable Rules tab', async ({ page }) => {
  const worldGuide = page.locator('.coll').filter({
    has: page.getByText('World Guide', { exact: true }),
  });
  await expect(worldGuide.getByRole('tab', { name: 'Rules' })).toBeFocused();
});

Given('a Campaign A Initiative rule-note save is in progress', async ({ page }) => {
  const notes = await openRule(page, 'Initiative');
  await hold(page, 'update_rule_notes');
  await notes.fill(EARLIER_RULE_DRAFT);
  await notes.blur();
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(1);
});

When('Campaign B requests its Initiative rule-note save', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  const notes = await openRule(page, 'Initiative');
  await notes.fill(CAMPAIGN_B_RULE_DRAFT);
  await notes.blur();
});

When('Campaign A requests a newer Initiative rule-note save', async ({ page }) => {
  await openCampaign(page, 'Campaign A');
  const notes = await openRule(page, 'Initiative');
  await notes.fill(CAMPAIGN_A_LATEST_RULE_DRAFT);
  await notes.blur();
});

Then('Initiative rule-note writes do not overlap', async ({ page }) => {
  expect(await maxConcurrentWrites(page, 'update_rule_notes')).toBe(1);
});

When('all three Initiative rule-note writes are acknowledged', async ({ page }) => {
  await resolveNext(page, 'update_rule_notes');
  await expect
    .poll(() => observations(page, 'update_rule_notes').then((writes) => writes.length))
    .toBe(2);
  await resolveNext(page, 'update_rule_notes');
  await expect
    .poll(() => observations(page, 'update_rule_notes').then((writes) => writes.length))
    .toBe(3);
  await resolveNext(page, 'update_rule_notes');
  await expect.poll(() => activeWrites(page, 'update_rule_notes')).toBe(0);
});

Then("each campaign's requested rule-note write was preserved in order", async ({ page }) => {
  const writes = await observations(page, 'update_rule_notes');
  expect(writes.map((write) => write.notes)).toEqual([
    EARLIER_RULE_DRAFT,
    CAMPAIGN_B_RULE_DRAFT,
    CAMPAIGN_A_LATEST_RULE_DRAFT,
  ]);
});

Then('the newest Campaign A rule note is persisted as saved', async ({ page }) => {
  const rule = await persisted<{ notes: string }>(page, 'update_rule_notes', 'initiative');
  expect(rule?.notes).toBe(CAMPAIGN_A_LATEST_RULE_DRAFT);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});
