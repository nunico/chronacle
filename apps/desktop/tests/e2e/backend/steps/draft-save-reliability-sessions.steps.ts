import { expect, type Locator, type Page } from '@playwright/test';
import { Given, Then, When, test } from './fixtures';
import {
  CAMPAIGN_A_QUESTION,
  SESSION_REVISION_TWO_TITLE,
  activeWrites,
  commitPending,
  composer,
  dispatchBlurWithoutMovingFocus,
  hold,
  holdNextSessionList,
  maxConcurrentWrites,
  observations,
  openCampaign,
  openRailView,
  pendingSessionLists,
  pendingWrites,
  persisted,
  pressChord,
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
const CREATED_SESSION_TITLE = 'Session 2';
const MISSING_SESSION_TITLE = 'Ashes retained after deletion';
const MISSING_SESSION_NOTES = 'Exact notes retained after deletion.';
const KEYBOARD_EDITED_SESSION_TITLE = `${CHANGED_SESSION_TITLE}go`;
const UNRELATED_SESSION_DRAFT_TITLE = 'Lanterns retained separately';

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

Given('creating a new session for campaign A is in progress', async ({ page }) => {
  await openRailView(page, 'Sessions');
  await hold(page, 'create_session');
  await page.getByRole('button', { name: /New session/i }).click();
  await expect.poll(() => activeWrites(page, 'create_session')).toBe(1);
  const writes = await observations(page, 'create_session');
  expect(writes).toHaveLength(1);
  expect(writes[0].campaignId).toBe('camp-a');
});

When('I switch to campaign B before session creation completes', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  await openRailView(page, 'Sessions');
  await expect(page.getByRole('button', { name: /Lanterns in Rain/ })).toBeVisible();
});

When('the delayed session creation succeeds', async ({ page }) => {
  await resolveNext(page, 'create_session');
  await expect.poll(() => activeWrites(page, 'create_session')).toBe(0);
});

Then('the created session is not shown in campaign B', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(CREATED_SESSION_TITLE) })).toHaveCount(
    0,
  );
});

When('I return to campaign A sessions', async ({ page }) => {
  await openCampaign(page, 'Campaign A');
  await openRailView(page, 'Sessions');
});

Then('the created session is shown once in campaign A', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(CREATED_SESSION_TITLE) })).toHaveCount(
    1,
  );
});

Then('the created session belongs to campaign A', async ({ page }) => {
  const created = await persisted<{ campaign_id: string; title: string }>(
    page,
    'update_session',
    'created-session-3',
  );
  expect(created).toMatchObject({ campaign_id: 'camp-a', title: CREATED_SESSION_TITLE });
});

When('I return to campaign A while its older session list is held', async ({ page }) => {
  await holdNextSessionList(page, 'camp-a');
  await openCampaign(page, 'Campaign A');
  await openRailView(page, 'Sessions');
  await expect.poll(() => pendingSessionLists(page)).toBe(1);
});

When(
  'the older Campaign A session list completes without the created session',
  async ({ page }) => {
    await resolveNextSessionList(page);
    await expect.poll(() => pendingSessionLists(page)).toBe(0);
  },
);

When('the backend commits the session without acknowledging creation', async ({ page }) => {
  await commitPending(page, 'create_session');
});

When('the delayed session creation acknowledgment arrives', async ({ page }) => {
  await resolveNext(page, 'create_session');
  await expect.poll(() => activeWrites(page, 'create_session')).toBe(0);
});

Then('the created session is still shown once in campaign A', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(CREATED_SESSION_TITLE) })).toHaveCount(
    1,
  );
});

Given('creating a default new session for campaign A fails', async ({ page }) => {
  await openRailView(page, 'Sessions');
  await rejectNext(page, 'create_session', {
    code: 'DATABASE',
    message: 'Session create failed.',
  });
  await page.getByRole('button', { name: /New session/i }).click();
});

Then('the session view keeps an actionable creation failure', async ({ page }) => {
  const failure = page.getByRole('alert');
  await expect(failure).toContainText("Couldn't create session.");
  await expect(failure).toContainText('Session create failed.');
  await expect(failure.getByRole('button', { name: 'Retry' })).toBeVisible();
  await expect(failure.getByRole('button', { name: 'Discard changes' })).toBeVisible();
});

When('I switch to campaign B and return to campaign A sessions', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  await openRailView(page, 'Sessions');
  await expect(page.getByRole('alert')).toHaveCount(0);
  await openCampaign(page, 'Campaign A');
  await openRailView(page, 'Sessions');
});

Then('the failed creation is restored only in campaign A', async ({ page }) => {
  await expect(page.getByRole('alert')).toContainText("Couldn't create session.");
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

When(
  'I retry the failed creation twice with the keyboard while it is pending',
  async ({ page }) => {
    await hold(page, 'create_session');
    const retry = page.getByRole('button', { name: 'Retry' });
    await retry.focus();
    await retry.press('Enter');
    await retry.press('Enter');
    await expect.poll(() => activeWrites(page, 'create_session')).toBe(1);
  },
);

Then('only one retry uses the original Campaign A session details', async ({ page }) => {
  const writes = await observations(page, 'create_session');
  expect(writes).toHaveLength(2);
  expect(writes[0]).toEqual(writes[1]);
  expect(writes[1]).toMatchObject({
    campaignId: 'camp-a',
    input: {
      sessionNumber: 2,
      title: CREATED_SESSION_TITLE,
      notes: '',
    },
  });
});

When('the retried session creation succeeds', async ({ page }) => {
  await resolveNext(page, 'create_session');
  await expect.poll(() => activeWrites(page, 'create_session')).toBe(0);
});

Then('the session is created once in campaign A', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(CREATED_SESSION_TITLE) })).toHaveCount(
    1,
  );
  const created = await persisted<{ campaign_id: string; title: string }>(
    page,
    'update_session',
    'created-session-3',
  );
  expect(created).toMatchObject({ campaign_id: 'camp-a', title: CREATED_SESSION_TITLE });
});

Then('the creation failure is cleared', async ({ page }) => {
  await expect(page.getByRole('alert')).toHaveCount(0);
});

Given('saving a changed session failed before its target was deleted', async ({ page }) => {
  const { title } = await openSession(page);
  await title.fill(MISSING_SESSION_TITLE);
  await page.getByRole('textbox', { name: 'Notes' }).evaluate((notes, value) => {
    const field = notes as HTMLTextAreaElement;
    field.value = value;
    field.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'insertText' }));
  }, MISSING_SESSION_NOTES);
  await rejectNext(page, 'update_session', { code: 'DATABASE', message: 'Session write failed.' });
  await title.blur();
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

Given('that session target is now deleted', async ({ page }) => {
  await removeSession(page, 'session-a');
});

Given('an unrelated session draft is retained', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  await openRailView(page, 'Sessions');
  await page.getByRole('button', { name: /Lanterns in Rain/ }).click();
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  await title.fill(UNRELATED_SESSION_DRAFT_TITLE);
  await rejectNext(page, 'update_session', {
    code: 'DATABASE',
    message: 'Unrelated session write failed.',
  });
  await title.blur();
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();

  await openCampaign(page, 'Campaign A');
  await openRailView(page, 'Sessions');
  await page.getByRole('button', { name: new RegExp(MISSING_SESSION_TITLE) }).click();
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    MISSING_SESSION_TITLE,
  );
});

When('I retry the failed session and it reports unavailable', async ({ page }) => {
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect(page.getByText('This record is no longer available.')).toBeVisible();
});

When('I navigate away and return to Sessions', async ({ page }) => {
  await openRailView(page, 'Oracle');
  await openRailView(page, 'Sessions');
});

Then('the changed session remains available as unavailable', async ({ page }) => {
  const recovery = page.getByRole('button', { name: new RegExp(MISSING_SESSION_TITLE) });
  await expect(recovery).toBeVisible();
  await recovery.click();
  await expect(page.getByText('This record is no longer available.')).toBeVisible();
});

Then('its exact title and notes are retained', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    MISSING_SESSION_TITLE,
  );
  await expect(page.getByRole('textbox', { name: 'Notes' })).toHaveValue(MISSING_SESSION_NOTES);
});

When('I retry the omitted session with the keyboard', async ({ page }) => {
  const retry = page.getByRole('button', { name: 'Retry' });
  await retry.focus();
  await retry.press('Enter');
});

Then('Retry still targets the omitted session', async ({ page }) => {
  await expect
    .poll(() =>
      observations(page, 'update_session').then(
        (writes) => writes.filter((write) => write.id === 'session-a').length,
      ),
    )
    .toBe(3);
  const writes = await observations(page, 'update_session');
  const omittedSessionWrites = writes.filter((write) => write.id === 'session-a');
  expect(omittedSessionWrites).toHaveLength(3);
  const retriedInput = omittedSessionWrites.at(-1)?.input as Record<string, unknown> | undefined;
  expect(retriedInput).toMatchObject({
    title: MISSING_SESSION_TITLE,
    notes: MISSING_SESSION_NOTES,
  });
});

Then('the unavailable failure remains actionable', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  await expect(page.getByText('This record is no longer available.')).toBeVisible();
});

When('I discard the omitted session with the keyboard', async ({ page }) => {
  const discard = page.getByRole('button', { name: 'Discard changes' });
  await discard.focus();
  await discard.press('Enter');
});

Then('only the omitted session recovery row disappears', async ({ page }) => {
  await expect(page.getByRole('button', { name: new RegExp(MISSING_SESSION_TITLE) })).toHaveCount(
    0,
  );
  await expect(page.getByRole('button', { name: /New session/i })).toBeVisible();
});

Then('focus moves to the stable session control', async ({ page }) => {
  await expect(page.getByRole('button', { name: /New session/i })).toBeFocused();
});

Then('the unrelated session draft remains intact', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  await openRailView(page, 'Sessions');
  const recovery = page.getByRole('button', { name: new RegExp(UNRELATED_SESSION_DRAFT_TITLE) });
  await expect(recovery).toBeVisible();
  await recovery.click();
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    UNRELATED_SESSION_DRAFT_TITLE,
  );
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

When('I navigate to Oracle with the g chord', async ({ page }) => {
  await pressChord(page, 'o');
  await expect(composer(page)).toBeVisible();
});

When('I return to Sessions with the g chord', async ({ page }) => {
  await page.getByRole('button', { name: 'Oracle', exact: true }).focus();
  await pressChord(page, 's');
  await expect(page.getByRole('heading', { name: 'Sessions' })).toBeVisible();
});

When('I press the Oracle g chord in the focused session field', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toBeFocused();
  await pressChord(page, 'o');
});

Then('I remain in the session editor and the typed keys remain unsaved', async ({ page }) => {
  await expect(page.getByRole('heading', { name: 'Sessions' })).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    KEYBOARD_EDITED_SESSION_TITLE,
  );
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

When('I move focus to the session notes field', async ({ page }) => {
  await page.getByRole('textbox', { name: 'Notes' }).focus();
});

Then('the ordinary session blur is saved', async ({ page }) => {
  await expect
    .poll(async () => {
      const session = await persisted<{ title: string }>(page, 'update_session', 'session-a');
      return session?.title;
    })
    .toBe(KEYBOARD_EDITED_SESSION_TITLE);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Delete' }).focus();
});

Then('the exact keyboard-edited session title is restored', async ({ page }) => {
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  if ((await title.count()) === 0) {
    await page.getByRole('button', { name: new RegExp(KEYBOARD_EDITED_SESSION_TITLE) }).click();
  }
  await expect(title).toHaveValue(KEYBOARD_EDITED_SESSION_TITLE);
});
