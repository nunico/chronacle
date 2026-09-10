import { expect, type Locator, type Page } from '@playwright/test';
import { Given, Then, When, test } from './fixtures';
import {
  CAMPAIGN_A_QUESTION,
  SESSION_REVISION_TWO_TITLE,
  activeWrites,
  composer,
  dispatchBlurWithoutMovingFocus,
  hold,
  holdNextSessionList,
  maxConcurrentWrites,
  observations,
  openRailView,
  pendingSessionLists,
  pendingWrites,
  persisted,
  rejectNext,
  removeSession,
  resolveNext,
  resolveNextSessionList,
  resolveNextWithCanonicalInput,
  setSessionCanonical,
  skipNativeCloseContract,
} from './draft-reliability-support';

const SESSION_TITLE = 'Ashes at Dawn';
const CHANGED_SESSION_TITLE = 'Ashes after the Storm';
const EARLIER_SESSION_TITLE = 'Earlier session revision';
const NEWER_SESSION_TITLE = 'Newer unsaved session revision';
const SESSION_REVISION_ONE_TITLE = 'Session draft revision one';
const LATER_SESSION_TITLE = 'Later authoritative session title';
const RAPID_TITLES = ['First rapid title', 'Second rapid title', 'Newest rapid title'];

async function openSession(page: Page): Promise<{ title: Locator }> {
  await openRailView(page, 'Sessions');
  const header = page.getByRole('button', { name: new RegExp(SESSION_TITLE) });
  await header.click();
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  await expect(title).toHaveValue(SESSION_TITLE);
  return { title };
}

Given('a save of an earlier session draft revision is in progress', async ({ page }) => {
  if (test.info().tags.includes('@native-close-contract')) {
    skipNativeCloseContract();
    return;
  }
  const { title } = await openSession(page);
  await hold(page, 'update_session');
  await title.fill(EARLIER_SESSION_TITLE);
  await title.blur();
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(1);
});

When('I make another edit to the session', async ({ page }) => {
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill(NEWER_SESSION_TITLE);
});

When('the earlier save completes', async ({ page }) => {
  await resolveNext(page, 'update_session');
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(0);
});

Then('my newer session edit remains intact', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    NEWER_SESSION_TITLE,
  );
});

Then('it is not incorrectly marked as saved', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
  await expect(page.getByText('Saved', { exact: true })).toHaveCount(0);
});

Given('a save of session draft revision 1 is in progress', async ({ page }) => {
  const { title } = await openSession(page);
  await hold(page, 'update_session');
  await title.fill(SESSION_REVISION_ONE_TITLE);
  await title.blur();
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(1);
});

When('I make session draft revision 2', async ({ page }) => {
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill(SESSION_REVISION_TWO_TITLE);
});

When('revision 1 is acknowledged with canonical content equal to revision 2', async ({ page }) => {
  await resolveNextWithCanonicalInput(page, 'update_session', {
    title: SESSION_REVISION_TWO_TITLE,
  });
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(0);
});

When('I save revision 2 and its acknowledgment completes', async ({ page }) => {
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  await title.blur();
  await expect.poll(() => pendingWrites(page, 'update_session')).toBe(1);
  await resolveNext(page, 'update_session');
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(0);
});

Then('revision 2 is shown as saved', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    SESSION_REVISION_TWO_TITLE,
  );
  const session = await persisted<{ title: string }>(page, 'update_session', 'session-a');
  expect(session?.title).toBe(SESSION_REVISION_TWO_TITLE);
  const writes = await observations(page, 'update_session');
  expect(writes).toHaveLength(2);
  expect((writes[1].input as { title: string }).title).toBe(SESSION_REVISION_TWO_TITLE);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Given('I have changed a session title', async ({ page }) => {
  const { title } = await openSession(page);
  await title.fill(CHANGED_SESSION_TITLE);
});

When('the session save fails', async ({ page }) => {
  await rejectNext(page, 'update_session', { code: 'DATABASE', message: 'Session write failed.' });
  await page.getByRole('textbox', { name: 'Name', exact: true }).blur();
});

Then('the changed session title remains available', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CHANGED_SESSION_TITLE,
  );
});

Then('the session shows an actionable save failure', async ({ page }) => {
  await expect(page.getByRole('alert')).toContainText(/couldn't save|session write failed/i);
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

When('I retry the session save and it succeeds', async ({ page }) => {
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect
    .poll(async () => {
      const session = await persisted<{ title: string }>(page, 'update_session', 'session-a');
      return session?.title;
    })
    .toBe(CHANGED_SESSION_TITLE);
});

Then('the changed session title is shown as saved', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CHANGED_SESSION_TITLE,
  );
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Then('the session failure indication is cleared', async ({ page }) => {
  await expect(page.getByRole('alert')).toHaveCount(0);
});

Then("campaign A's session draft is not shown in campaign B", async ({ page }) => {
  await openRailView(page, 'Sessions');
  await expect(page.getByRole('button', { name: /Lanterns in Rain/ })).toBeVisible();
  await expect(page.getByText(CHANGED_SESSION_TITLE, { exact: true })).toHaveCount(0);
});

Then('its save is still in progress', async ({ page }) => {
  expect(await activeWrites(page, 'update_session')).toBe(1);
  await expect(page.getByText('Saving…', { exact: true })).toBeVisible();
});

Given('I have changed a session title without blurring it', async ({ page }) => {
  const { title } = await openSession(page);
  await title.fill(CHANGED_SESSION_TITLE);
  await expect(title).toBeFocused();
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

When('I restore the original session title', async ({ page }) => {
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill(SESSION_TITLE);
});

Then('the session no longer indicates unsaved changes', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toHaveCount(0);
});

Then('no session save has been sent', async ({ page }) => {
  expect(await observations(page, 'update_session')).toHaveLength(0);
});

When('saving reports that the session is no longer available', async ({ page }) => {
  await removeSession(page, 'session-a');
  await page.getByRole('textbox', { name: 'Name', exact: true }).blur();
});

Then('the session shows the localized unavailable-target recovery', async ({ page }) => {
  const alert = page.getByRole('alert');
  await expect(alert).toContainText('This record is no longer available.');
  await expect(alert.getByRole('button', { name: 'Retry' })).toBeVisible();
});

Then('the raw session backend detail is not shown', async ({ page }) => {
  await expect(page.getByRole('alert')).not.toContainText('Session no longer available.');
});

When('I retry the unavailable session save', async ({ page }) => {
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect
    .poll(() => observations(page, 'update_session').then((writes) => writes.length))
    .toBe(2);
});

Then('Retry still targets the same session', async ({ page }) => {
  const writes = await observations(page, 'update_session');
  expect(writes).toHaveLength(2);
  expect(writes.every((write) => write.id === 'session-a')).toBe(true);
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CHANGED_SESSION_TITLE,
  );
});

Given(
  'the next Campaign A session list completes with the earlier saved title',
  async ({ page }) => {
    await holdNextSessionList(page, 'camp-a');
  },
);

When('I return to Sessions before the held list completes', async ({ page }) => {
  await openRailView(page, 'Sessions');
  await expect.poll(() => pendingSessionLists(page)).toBe(1);
});

When('the held session list completes', async ({ page }) => {
  await resolveNextSessionList(page);
});

When('a later session list loads newer canonical content', async ({ page }) => {
  await setSessionCanonical(page, 'session-a', { title: LATER_SESSION_TITLE });
  await openRailView(page, 'Oracle');
  await openRailView(page, 'Sessions');
});

Then('the newer canonical session title is shown', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(LATER_SESSION_TITLE) })).toBeVisible();
});

Given('the next Campaign A session list is held with obsolete content', async ({ page }) => {
  await holdNextSessionList(page, 'camp-a');
});

When('I open Sessions and leave before that list completes', async ({ page }) => {
  await openRailView(page, 'Sessions');
  await expect.poll(() => pendingSessionLists(page)).toBe(1);
  await openRailView(page, 'Oracle');
});

When('the backend session gains newer canonical content', async ({ page }) => {
  await setSessionCanonical(page, 'session-a', { title: LATER_SESSION_TITLE });
});

When('the obsolete session list completes', async ({ page }) => {
  await resolveNextSessionList(page);
});

Then('the newer canonical session title is still shown', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(LATER_SESSION_TITLE) })).toBeVisible();
  await expect(page.getByText(SESSION_TITLE, { exact: true })).toHaveCount(0);
});

Then('Delete is unavailable for that session', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Delete' })).toBeDisabled();
});

Then('I am told to wait for the session save to finish', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Delete' })).toHaveAccessibleDescription(
    /wait for saving to finish/i,
  );
});

When('I activate Delete and explicitly confirm deletion', async ({ page }) => {
  page.once('dialog', (dialog) => dialog.accept());
  await page.getByRole('button', { name: 'Delete' }).click();
});

Then('the session and only its retained draft are removed', async ({ page }) => {
  await expect(page.getByRole('button', { name: /Ashes/ })).toHaveCount(0);
  expect(await persisted(page, 'update_session', 'session-a')).toBeUndefined();
  await openRailView(page, 'Oracle');
  await expect(composer(page)).toHaveValue(CAMPAIGN_A_QUESTION);
});

When('I confirm deleting the session and deletion remains in progress', async ({ page }) => {
  await hold(page, 'delete_session');
  page.once('dialog', (dialog) => dialog.accept());
  await page.getByRole('button', { name: 'Delete' }).click();
  await expect.poll(() => activeWrites(page, 'delete_session')).toBe(1);
});

Then(
  'the session fields and its Delete, Retry, and Discard actions are unavailable',
  async ({ page }) => {
    await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toBeDisabled();
    await expect(page.getByLabel('Date played')).toBeDisabled();
    await expect(page.getByRole('textbox', { name: 'Notes' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Delete' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Retry' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Discard changes' })).toBeDisabled();
  },
);

When('I attempt to retry while the session deletion is pending', async ({ page }) => {
  await page
    .getByRole('button', { name: 'Retry' })
    .evaluate((button: HTMLButtonElement) => button.click());
  await dispatchBlurWithoutMovingFocus(page.getByRole('textbox', { name: 'Name', exact: true }));
});

Then('no session save starts behind deletion', async ({ page }) => {
  expect(await observations(page, 'update_session')).toHaveLength(1);
  expect(await activeWrites(page, 'update_session')).toBe(0);
  expect(await activeWrites(page, 'delete_session')).toBe(1);
});

When('the pending session deletion completes', async ({ page }) => {
  await resolveNext(page, 'delete_session');
  await expect.poll(() => activeWrites(page, 'delete_session')).toBe(0);
});

When('I activate the session Discard action with the keyboard', async ({ page }) => {
  const discard = page.getByRole('button', { name: 'Discard changes' });
  await discard.focus();
  await page.keyboard.press('Enter');
});

Then('the saved session title is restored', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(SESSION_TITLE);
});

Then('focus moves to the stable session header', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(SESSION_TITLE) })).toBeFocused();
});

Given('a save of my changed session title is in progress', async ({ page }) => {
  const { title } = await openSession(page);
  await hold(page, 'update_session');
  await title.fill(CHANGED_SESSION_TITLE);
  await title.blur();
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(1);
});

When('I navigate to Oracle before the save completes', async ({ page }) => {
  await openRailView(page, 'Oracle');
  expect(await activeWrites(page, 'update_session')).toBe(1);
});

When('the pending session save completes', async ({ page }) => {
  await resolveNext(page, 'update_session');
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(0);
});

When('I return to Sessions', async ({ page }) => {
  await openRailView(page, 'Sessions');
  await expect(page.getByRole('heading', { name: 'Sessions' })).toBeVisible();
});

Then('the changed session title is preserved', async ({ page }) => {
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  if ((await title.count()) === 0) {
    await page
      .getByRole('button', { name: new RegExp(`${SESSION_TITLE}|${CHANGED_SESSION_TITLE}`) })
      .click();
  }
  await expect(title).toHaveValue(CHANGED_SESSION_TITLE);
});

Then('it is shown as saved', async ({ page }) => {
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Then('it remains shown as unsaved', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

Given('session saves are being held open', async ({ page }) => {
  await openSession(page);
  await hold(page, 'update_session');
});

When('I request rapid saves for three different session titles', async ({ page }) => {
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  for (const value of RAPID_TITLES) {
    await title.fill(value);
    await title.blur();
  }
  await expect(title).toHaveValue(RAPID_TITLES[2]);
});

Then('the session save attempts do not overlap', async ({ page }) => {
  expect(await maxConcurrentWrites(page, 'update_session')).toBe(1);
});

When('the pending session saves are acknowledged', async ({ page }) => {
  await resolveNext(page, 'update_session');
  await expect
    .poll(() => observations(page, 'update_session').then((calls) => calls.length))
    .toBe(2);
  await expect.poll(() => pendingWrites(page, 'update_session')).toBe(1);
  await resolveNext(page, 'update_session');
  await expect.poll(() => activeWrites(page, 'update_session')).toBe(0);
});

Then('the newest session title is preserved', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    RAPID_TITLES[2],
  );
});

Then('only the newest revision is shown as saved', async ({ page }) => {
  const session = await persisted<{ title: string }>(page, 'update_session', 'session-a');
  expect(session?.title).toBe(RAPID_TITLES[2]);
  const writes = await observations(page, 'update_session');
  expect(writes).toHaveLength(2);
  expect((writes[1].input as { title: string }).title).toBe(RAPID_TITLES[2]);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Given('a session save has failed while its title field is focused', async ({ page }) => {
  const { title } = await openSession(page);
  await title.fill(CHANGED_SESSION_TITLE);
  await rejectNext(page, 'update_session', { code: 'DATABASE', message: 'Session write failed.' });
  await dispatchBlurWithoutMovingFocus(title);
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  await expect(title).toBeFocused();
});

When('I move to Retry and press Enter', async ({ page }) => {
  const retry = page.getByRole('button', { name: 'Retry' });
  await retry.focus();
  await retry.press('Enter');
});

Then('the session save is retried', async ({ page }) => {
  await expect
    .poll(() => observations(page, 'update_session').then((calls) => calls.length))
    .toBe(2);
  const session = await persisted<{ title: string }>(page, 'update_session', 'session-a');
  expect(session?.title).toBe(CHANGED_SESSION_TITLE);
});

Then('focus returns to the session title', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toBeFocused();
});
