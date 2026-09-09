import { expect, type Locator, type Page } from '@playwright/test';
import { Given, Then, When, test } from './fixtures';
import {
  resetDraftReliabilityIpcMock,
  type DraftReliabilityControls,
  type DraftWriteCommand,
} from '../ipc-mock';

interface DraftWindow extends Window {
  __draftReliability: DraftReliabilityControls;
  __ipcCalls: Array<{ cmd: string; args?: Record<string, unknown> }>;
}

const QUESTION = 'Where is the Moon Gate?';
const CAMPAIGN_A_QUESTION = 'Campaign A secret question';
const CAMPAIGN_B_QUESTION = 'Campaign B separate question';
const NO_CAMPAIGN_QUESTION = 'A question without a campaign';
const MIRA_SAVED_NOTES = 'Mira saved notes.';
const MIRA_DRAFT_NOTES = 'Mira draft notes that must survive.';
const MIRA_CANONICAL_NAME = 'Mira Moonshadow';
const MIRA_CANONICAL_NOTES = 'Mira canonical notes acknowledged by the save.';
const MIRA_LATER_CANONICAL_NOTES = 'Mira canonical notes from a later list load.';
const EARLIER_MIRA_NOTES = 'Mira notes acknowledged by the earlier save.';
const NEWER_MIRA_NOTES = 'Mira newer notes that remain unsaved.';
const TORVIN_SAVED_NOTES = 'Torvin saved notes.';
const NEW_ENTITY_NAME = 'Elowen';
const NEW_ENTITY_NOTES = 'A new NPC draft, not a saved record.';
const CREATE_REVISION_ONE_NAME = 'Nyra';
const CREATE_REVISION_ONE_NOTES = 'Nyra first creation revision.';
const CREATE_REVISION_TWO_NAME = 'Nyra the Unwritten';
const CREATE_REVISION_TWO_NOTES = 'Nyra second revision entered during creation.';
const CONVERGED_DESTINATION_NAME = 'Nyra of the North';
const CONVERGED_DESTINATION_NOTES = 'Nyra destination notes saved after the record appeared.';
const ASSIGNED_DESTINATION_DRAFT_NOTES = 'Unsaved notes entered on the assigned NPC row.';
const POST_PROMOTION_NOTES = 'Nyra notes saved after local promotion recovery.';
const SESSION_TITLE = 'Ashes at Dawn';
const CHANGED_SESSION_TITLE = 'Ashes after the Storm';
const EARLIER_SESSION_TITLE = 'Earlier session revision';
const NEWER_SESSION_TITLE = 'Newer unsaved session revision';
const SESSION_REVISION_ONE_TITLE = 'Session draft revision one';
const SESSION_REVISION_TWO_TITLE = 'Session draft revision two';
const LATER_SESSION_TITLE = 'Later authoritative session title';
const RULE_DRAFT = 'Roll initiative once for each side.';
const EARLIER_RULE_DRAFT = 'Earlier initiative ruling.';
const NEWER_RULE_DRAFT = 'Newer initiative ruling that remains local.';
const CAMPAIGN_B_RULE_DRAFT = 'Campaign B queued initiative ruling.';
const CAMPAIGN_A_LATEST_RULE_DRAFT = 'Campaign A newest initiative ruling.';
const FOCUSED_RULE_DRAFT = 'Unsaved note';
const NO_CAMPAIGN_RULE_DRAFT = 'No-campaign rule note';
const WORLD_GUIDE_RULE_DRAFT = 'World Guide retained initiative ruling.';
const RAPID_TITLES = ['First rapid title', 'Second rapid title', 'Newest rapid title'];
const RAPID_RULE_NOTES = ['First rapid ruling.', 'Second rapid ruling.', 'Newest rapid ruling.'];
const pagesWithPreAcknowledgmentEntityList = new WeakSet<Page>();

async function hold(page: Page, command: DraftWriteCommand): Promise<void> {
  await page.evaluate((commandName) => {
    (window as unknown as DraftWindow).__draftReliability.hold(commandName);
  }, command);
}

async function commitPending(
  page: Page,
  command: DraftWriteCommand,
  canonicalInput?: Record<string, unknown>,
): Promise<void> {
  await page.evaluate(
    ({ commandName, input }) => {
      (window as unknown as DraftWindow).__draftReliability.commitPending(commandName, input);
    },
    { commandName: command, input: canonicalInput },
  );
}

async function rejectNext(page: Page, command: string, error: unknown): Promise<void> {
  await page.evaluate(
    ({ commandName, rejection }) => {
      (window as unknown as DraftWindow).__draftReliability.rejectNext(commandName, rejection);
    },
    { commandName: command, rejection: error },
  );
}

async function rejectPending(
  page: Page,
  command: DraftWriteCommand,
  error: unknown,
): Promise<void> {
  await page.evaluate(
    ({ commandName, rejection }) => {
      (window as unknown as DraftWindow).__draftReliability.rejectPending(commandName, rejection);
    },
    { commandName: command, rejection: error },
  );
}

async function removeEntity(page: Page, id: string): Promise<void> {
  await page.evaluate((recordId) => {
    (window as unknown as DraftWindow).__draftReliability.removeEntity(recordId);
  }, id);
}

async function removeSession(page: Page, id: string): Promise<void> {
  await page.evaluate((recordId) => {
    (window as unknown as DraftWindow).__draftReliability.removeSession(recordId);
  }, id);
}

async function removeRule(page: Page, id: string): Promise<void> {
  await page.evaluate((recordId) => {
    (window as unknown as DraftWindow).__draftReliability.removeRule(recordId);
  }, id);
}

async function setEntityCanonical(
  page: Page,
  id: string,
  changes: Record<string, unknown>,
): Promise<void> {
  await page.evaluate(
    ({ recordId, canonicalChanges }) => {
      (window as unknown as DraftWindow).__draftReliability.setEntityCanonical(
        recordId,
        canonicalChanges,
      );
    },
    { recordId: id, canonicalChanges: changes },
  );
}

async function holdNextEntityList(page: Page, campaignId: string, kind: string): Promise<void> {
  await page.evaluate(
    ({ heldCampaignId, heldKind }) => {
      (window as unknown as DraftWindow).__draftReliability.holdNextEntityList(
        heldCampaignId,
        heldKind,
      );
    },
    { heldCampaignId: campaignId, heldKind: kind },
  );
}

async function resolveNextEntityList(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as unknown as DraftWindow).__draftReliability.resolveNextEntityList();
  });
}

async function holdNextSessionList(page: Page, campaignId: string): Promise<void> {
  await page.evaluate((heldCampaignId) => {
    (window as unknown as DraftWindow).__draftReliability.holdNextSessionList(heldCampaignId);
  }, campaignId);
}

async function resolveNextSessionList(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as unknown as DraftWindow).__draftReliability.resolveNextSessionList();
  });
}

async function pendingSessionLists(page: Page): Promise<number> {
  return page.evaluate(() =>
    (window as unknown as DraftWindow).__draftReliability.pendingSessionLists(),
  );
}

async function setSessionCanonical(
  page: Page,
  id: string,
  changes: Record<string, unknown>,
): Promise<void> {
  await page.evaluate(
    ({ recordId, canonicalChanges }) => {
      (window as unknown as DraftWindow).__draftReliability.setSessionCanonical(
        recordId,
        canonicalChanges,
      );
    },
    { recordId: id, canonicalChanges: changes },
  );
}

async function pendingEntityLists(page: Page): Promise<number> {
  return page.evaluate(() =>
    (window as unknown as DraftWindow).__draftReliability.pendingEntityLists(),
  );
}

async function resolveNext(page: Page, command: string): Promise<void> {
  await page.evaluate((commandName) => {
    (window as unknown as DraftWindow).__draftReliability.resolveNext(commandName);
  }, command);
}

async function resolveNextWithCanonicalInput(
  page: Page,
  command: string,
  canonicalInput: Record<string, unknown>,
): Promise<void> {
  await page.evaluate(
    ({ commandName, input }) => {
      (window as unknown as DraftWindow).__draftReliability.resolveNext(commandName, input);
    },
    { commandName: command, input: canonicalInput },
  );
}

async function activeWrites(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.activeWrites(commandName),
    command,
  );
}

async function maxConcurrentWrites(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.maxConcurrentWrites(commandName),
    command,
  );
}

async function pendingWrites(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.pendingWrites(commandName),
    command,
  );
}

async function persisted<T>(page: Page, command: string, id: string): Promise<T | undefined> {
  return page.evaluate(
    ({ commandName, recordId }) =>
      (window as unknown as DraftWindow).__draftReliability.persisted(commandName, recordId) as
        | T
        | undefined,
    { commandName: command, recordId: id },
  );
}

async function observations(page: Page, command: string): Promise<Array<Record<string, unknown>>> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.observations(commandName),
    command,
  );
}

async function submissions(page: Page): Promise<Array<Record<string, unknown>>> {
  return page.evaluate(() => (window as unknown as DraftWindow).__draftReliability.submissions());
}

function composer(page: Page): Locator {
  return page.getByRole('textbox', { name: 'Ask a rule, a name, a place…' });
}

async function openRailView(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name }).click();
}

async function openCampaign(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name: 'Switch campaign' }).click();
  const switcher = page.getByRole('dialog', { name: 'Switch campaign' });
  await switcher.getByRole('button', { name }).click();
}

async function openEntity(page: Page, name: string): Promise<Locator> {
  await openRailView(page, 'NPCs');
  await page.getByRole('button', { name, exact: true }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form).toBeVisible();
  return form;
}

async function openSession(page: Page): Promise<{ title: Locator }> {
  await openRailView(page, 'Sessions');
  const header = page.getByRole('button', { name: new RegExp(SESSION_TITLE) });
  await header.click();
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  await expect(title).toHaveValue(SESSION_TITLE);
  return { title };
}

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

async function dispatchBlurWithoutMovingFocus(locator: Locator): Promise<void> {
  await locator.evaluate((element) => element.dispatchEvent(new FocusEvent('blur')));
}

function unsavedChangesDialog(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Unsaved changes' });
}

function entityRow(page: Page, name: string): Locator {
  return page.locator('li.entity-row').filter({
    has: page.getByRole('button', { name, exact: true }),
  });
}

async function openBlockedCreatePromotion(page: Page): Promise<Locator> {
  await openRailView(page, 'NPCs');
  await page.getByRole('button', { name: 'New NPC' }).click();
  const createForm = page.getByRole('form', { name: 'Entity form' });
  await createForm
    .getByRole('textbox', { name: 'Name', exact: true })
    .fill(CREATE_REVISION_ONE_NAME);
  await createForm
    .getByRole('textbox', { name: 'Notes', exact: true })
    .fill(CREATE_REVISION_ONE_NOTES);
  await hold(page, 'create_entity');
  await createForm.getByRole('button', { name: 'Create', exact: true }).click();
  await expect.poll(() => activeWrites(page, 'create_entity')).toBe(1);
  await commitPending(page, 'create_entity');

  await openRailView(page, 'Campaign & sources');
  await openRailView(page, 'NPCs');
  const assignedRow = entityRow(page, CREATE_REVISION_ONE_NAME);
  await expect(assignedRow).toHaveCount(1);
  await assignedRow.getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true }).click();
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('textbox', { name: 'Notes', exact: true })
    .fill(ASSIGNED_DESTINATION_DRAFT_NOTES);

  await resolveNext(page, 'create_entity');
  await expect.poll(() => activeWrites(page, 'create_entity')).toBe(0);
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(2);
  await entityRow(page, CREATE_REVISION_ONE_NAME)
    .last()
    .getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true })
    .click();
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  await expect(alert).toBeVisible();
  await expect(alert.getByRole('button', { name: 'Retry' })).toBeVisible();
  return alert;
}

async function openFailedMiraSave(page: Page): Promise<Locator> {
  const form = await openEntity(page, 'Mira');
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_DRAFT_NOTES);
  await rejectNext(page, 'update_entity', {
    code: 'DATABASE',
    message: 'Database unavailable.',
  });
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  const alert = page.getByRole('alert').filter({ hasText: /couldn't save/i });
  await expect(alert).toBeVisible();
  return alert;
}

function skipNativeCloseContract(): void {
  test.skip(
    true,
    'Native Tauri close requests cannot be exercised by the mocked browser suite; Task 5 binds this contract in tauri-driver.',
  );
}

Given('draft reliability test data is available', async ({ page }) => {
  await resetDraftReliabilityIpcMock(page);
});

Given('I have opened Chronacle in campaign {string}', async ({ page }, campaignName: string) => {
  expect(
    await page.evaluate(() => (window as unknown as DraftWindow).__draftReliability.seedMode()),
  ).toBe('campaigns');
  await expect(page.getByRole('button', { name: 'Switch campaign' })).toContainText(campaignName);
});

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

Given('I have changed the notes for entity {string} without saving', async ({ page }, name) => {
  const form = await openEntity(page, name);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_DRAFT_NOTES);
});

When('I navigate to another view', async ({ page }) => {
  await openRailView(page, 'Oracle');
});

When('I reopen entity {string}', async ({ page }, name: string) => {
  await openEntity(page, name);
});

Then('my entity edits are preserved', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    MIRA_DRAFT_NOTES,
  );
});

Then('the interface indicates that they are not yet saved', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true }).first()).toBeVisible();
});

Given("the next NPC list load is held with Mira's earlier saved content", async ({ page }) => {
  await openRailView(page, 'Campaign & sources');
  await holdNextEntityList(page, 'camp-a', 'npc');
});

When('I open NPCs and leave before that list completes', async ({ page }) => {
  await openRailView(page, 'NPCs');
  await expect.poll(() => pendingEntityLists(page)).toBe(1);
  await openRailView(page, 'Oracle');
});

When('I return to NPCs and save canonical changes to Mira', async ({ page }) => {
  const form = await openEntity(page, 'Mira');
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill(MIRA_CANONICAL_NAME);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_CANONICAL_NOTES);
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  await expect
    .poll(async () => {
      const mira = await persisted<{ name: string; notes: string }>(page, 'update_entity', 'mira');
      return mira ? { name: mira.name, notes: mira.notes } : null;
    })
    .toEqual({ name: MIRA_CANONICAL_NAME, notes: MIRA_CANONICAL_NOTES });
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

When('the old NPC list completes after that save acknowledgment', async ({ page }) => {
  await resolveNextEntityList(page);
  await expect.poll(() => pendingEntityLists(page)).toBe(0);
  await page.evaluate(
    () =>
      new Promise<void>((resolve) => {
        requestAnimationFrame(() => resolve());
      }),
  );
});

Given('saving my changed entity {string} is held in progress', async ({ page }, name: string) => {
  const form = await openEntity(page, name);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_DRAFT_NOTES);
  await hold(page, 'update_entity');
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  await expect.poll(() => activeWrites(page, 'update_entity')).toBe(1);
});

When("the entity list completes with Mira's earlier saved content", async ({ page }) => {
  await expect(entityRow(page, 'Mira')).toBeVisible();
  const mira = await persisted<{ notes: string }>(page, 'update_entity', 'mira');
  expect(mira?.notes).toBe(MIRA_SAVED_NOTES);
  expect(await activeWrites(page, 'update_entity')).toBe(1);
});

When('the held save completes with its canonical content', async ({ page }) => {
  await resolveNextWithCanonicalInput(page, 'update_entity', {
    name: MIRA_CANONICAL_NAME,
    notes: MIRA_CANONICAL_NOTES,
  });
  await expect.poll(() => activeWrites(page, 'update_entity')).toBe(0);
  const mira = await persisted<{ notes: string }>(page, 'update_entity', 'mira');
  expect(mira?.notes).toBe(MIRA_CANONICAL_NOTES);
});

When('I open entity {string} from that already-rendered list', async ({ page }, name: string) => {
  await entityRow(page, name).getByRole('button', { name, exact: true }).click();
});

Then(
  'the acknowledged canonical name and notes are shown in the row, preview, and editor',
  async ({ page }) => {
    const form = page.getByRole('form', { name: 'Entity form' });
    await expect.soft(entityRow(page, 'Mira')).toHaveCount(0);
    await expect.soft(entityRow(page, MIRA_CANONICAL_NAME)).toHaveCount(1);
    await expect
      .soft(form.getByRole('textbox', { name: 'Name', exact: true }))
      .toHaveValue(MIRA_CANONICAL_NAME);
    await expect
      .soft(form.getByRole('textbox', { name: 'Notes', exact: true }))
      .toHaveValue(MIRA_CANONICAL_NOTES);
    await expect.soft(page.getByText(MIRA_CANONICAL_NOTES, { exact: true })).toBeVisible();
  },
);

Then('the acknowledged entity is shown as saved', async ({ page }) => {
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  await expect(page.getByText('Unsaved changes', { exact: true })).toHaveCount(0);
});

When('a later entity list completes with newer canonical content', async ({ page }) => {
  await setEntityCanonical(page, 'mira', { notes: MIRA_LATER_CANONICAL_NOTES });
  await openRailView(page, 'Oracle');
  await openRailView(page, 'NPCs');
  await expect(entityRow(page, MIRA_CANONICAL_NAME)).toBeVisible();
});

Then('the newer canonical content is shown', async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true }),
  ).toHaveValue(MIRA_LATER_CANONICAL_NOTES);
  await expect(page.getByText(MIRA_LATER_CANONICAL_NOTES, { exact: true })).toBeVisible();
});

Given('I have started creating an NPC', async ({ page }) => {
  await openRailView(page, 'NPCs');
  await page.getByRole('button', { name: 'New NPC' }).click();
});

Given('I have entered its name and notes', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill(NEW_ENTITY_NAME);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(NEW_ENTITY_NOTES);
});

When('I navigate away and return to NPCs', async ({ page }) => {
  await openRailView(page, 'Oracle');
  await openRailView(page, 'NPCs');
});

Then('my new entity draft is restored', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    NEW_ENTITY_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    NEW_ENTITY_NOTES,
  );
});

Then('navigation has not created a saved entity', async ({ page }) => {
  expect(await observations(page, 'create_entity')).toHaveLength(0);
  const mira = await persisted<{ name: string }>(page, 'update_entity', 'mira');
  const torvin = await persisted<{ name: string }>(page, 'update_entity', 'torvin');
  expect([mira?.name, torvin?.name]).not.toContain(NEW_ENTITY_NAME);
});

Given('creation of revision 1 for a new NPC is in progress', async ({ page }) => {
  await openRailView(page, 'NPCs');
  await page.getByRole('button', { name: 'New NPC' }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill(CREATE_REVISION_ONE_NAME);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(CREATE_REVISION_ONE_NOTES);
  await hold(page, 'create_entity');
  await form.getByRole('button', { name: 'Create', exact: true }).click();
  await expect.poll(() => activeWrites(page, 'create_entity')).toBe(1);
});

When('I enter revision 2 before creation completes', async ({ page }) => {
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('textbox', { name: 'Notes', exact: true })
    .fill(CREATE_REVISION_TWO_NOTES);
});

When('revision 2 also renames the NPC', async ({ page }) => {
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('textbox', { name: 'Name', exact: true })
    .fill(CREATE_REVISION_TWO_NAME);
});

Then('the Create action is unavailable', async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('button', { name: 'Create', exact: true }),
  ).toBeDisabled();
});

When('creation completes and assigns the NPC an ID', async ({ page }) => {
  await resolveNext(page, 'create_entity');
  await expect.poll(() => activeWrites(page, 'create_entity')).toBe(0);
  await expect
    .poll(async () => {
      const created = await persisted<{ name: string }>(page, 'create_entity', 'created-3');
      return created?.name;
    })
    .toBe(CREATE_REVISION_ONE_NAME);
});

Then('revision 2 remains visible as unsaved', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  if (await form.isVisible()) {
    await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
      CREATE_REVISION_TWO_NOTES,
    );
  } else {
    await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
      SESSION_REVISION_TWO_TITLE,
    );
    await expect(page.getByText('Saved', { exact: true })).toHaveCount(0);
  }
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

Then('the explicit action is now Save', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('button', { name: 'Save', exact: true })).toBeVisible();
  await expect(form.getByRole('button', { name: 'Create', exact: true })).toHaveCount(0);
});

When('I save revision 2', async ({ page }) => {
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('button', { name: 'Save', exact: true })
    .click();
  await expect
    .poll(async () => {
      const created = await persisted<{ notes: string }>(page, 'update_entity', 'created-3');
      return created?.notes;
    })
    .toBe(CREATE_REVISION_TWO_NOTES);
});

Then('revision 2 updates the NPC with the assigned ID', async ({ page }) => {
  const created = await persisted<{ name: string; notes: string }>(
    page,
    'update_entity',
    'created-3',
  );
  expect(created).toMatchObject({
    name: CREATE_REVISION_ONE_NAME,
    notes: CREATE_REVISION_TWO_NOTES,
  });
  const updates = await observations(page, 'update_entity');
  expect(updates.at(-1)?.id).toBe('created-3');
});

Then('only one NPC has been created', async ({ page }) => {
  expect(await observations(page, 'create_entity')).toHaveLength(1);
});

When('the backend commits the new NPC but delays its acknowledgment', async ({ page }) => {
  await commitPending(page, 'create_entity');
  await expect
    .poll(async () => {
      const created = await persisted<{ name: string }>(page, 'create_entity', 'created-3');
      return created?.name;
    })
    .toBe(CREATE_REVISION_ONE_NAME);
  expect(await activeWrites(page, 'create_entity')).toBe(1);
  expect(await pendingWrites(page, 'create_entity')).toBe(1);
});

When('I switch to campaign B and return to campaign A on NPCs', async ({ page }) => {
  await openCampaign(page, 'Campaign B');
  await expect(page.getByText('No npcs yet.')).toBeVisible();
  await openCampaign(page, 'Campaign A');
  await expect.poll(() => activeWrites(page, 'create_entity')).toBe(1);
});

When('I return to NPCs before creation is acknowledged', async ({ page }) => {
  await openRailView(page, 'NPCs');
  expect(await activeWrites(page, 'create_entity')).toBe(1);
  expect(await pendingWrites(page, 'create_entity')).toBe(1);
});

Then('the committed NPC appears in the entity list', async ({ page }) => {
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toBeVisible();
  const created = await persisted<{ name: string }>(page, 'create_entity', 'created-3');
  expect(created?.name).toBe(CREATE_REVISION_ONE_NAME);
});

When('the delayed creation acknowledgment arrives', async ({ page }) => {
  await resolveNext(page, 'create_entity');
  await expect.poll(() => activeWrites(page, 'create_entity')).toBe(0);
});

When('I make unsaved changes to the assigned NPC from the list', async ({ page }) => {
  const assignedRows = entityRow(page, CREATE_REVISION_ONE_NAME);
  await expect(assignedRows).toHaveCount(1);
  await assignedRows.getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true }).click();
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('textbox', { name: 'Notes', exact: true })
    .fill(ASSIGNED_DESTINATION_DRAFT_NOTES);
});

When('I update the committed NPC from the entity list', async ({ page }) => {
  const assignedRows = entityRow(page, CREATE_REVISION_ONE_NAME);
  await expect(assignedRows).toHaveCount(1);
  await assignedRows.getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true }).click();
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('textbox', { name: 'Notes', exact: true })
    .fill(ASSIGNED_DESTINATION_DRAFT_NOTES);
});

When('I rename and save the assigned NPC destination', async ({ page }) => {
  const assignedRow = entityRow(page, CREATE_REVISION_ONE_NAME);
  await expect(assignedRow).toHaveCount(1);
  await assignedRow.getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill(CONVERGED_DESTINATION_NAME);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(CONVERGED_DESTINATION_NOTES);
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  await expect
    .poll(async () => {
      const assigned = await persisted<{ name: string; notes: string }>(
        page,
        'update_entity',
        'created-3',
      );
      return assigned ? { name: assigned.name, notes: assigned.notes } : null;
    })
    .toEqual({ name: CONVERGED_DESTINATION_NAME, notes: CONVERGED_DESTINATION_NOTES });
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Then('the converged NPC row keeps the destination name and selection', async ({ page }) => {
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(0);
  const destinationRow = entityRow(page, CONVERGED_DESTINATION_NAME);
  await expect(destinationRow).toHaveCount(1);
  await expect(destinationRow).toHaveClass(/selected/);
});

Then('the converged NPC editor and preview keep the destination content', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CONVERGED_DESTINATION_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    CONVERGED_DESTINATION_NOTES,
  );
  await expect(page.getByText(CONVERGED_DESTINATION_NOTES, { exact: true })).toBeVisible();
});

Then('the converged NPC is shown as saved', async ({ page }) => {
  await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible();
  await expect(page.getByText('Unsaved changes', { exact: true })).toHaveCount(0);
});

Then('the destination content remains persisted for the assigned NPC', async ({ page }) => {
  const assigned = await persisted<{ name: string; notes: string }>(
    page,
    'update_entity',
    'created-3',
  );
  expect(assigned).toMatchObject({
    name: CONVERGED_DESTINATION_NAME,
    notes: CONVERGED_DESTINATION_NOTES,
  });
});

Then('convergence leaves one created NPC without another backend write', async ({ page }) => {
  await expect(entityRow(page, CONVERGED_DESTINATION_NAME)).toHaveCount(1);
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(0);
  expect(await observations(page, 'create_entity')).toHaveLength(1);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

When('I switch from the converged NPC to Mira and back', async ({ page }) => {
  await entityRow(page, 'Mira').getByRole('button', { name: 'Mira', exact: true }).click();
  await entityRow(page, CONVERGED_DESTINATION_NAME)
    .getByRole('button', { name: CONVERGED_DESTINATION_NAME, exact: true })
    .click();
});

Then('the assigned NPC editor remains usable with the destination content', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toBeEnabled();
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CONVERGED_DESTINATION_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    CONVERGED_DESTINATION_NOTES,
  );
  await expect(entityRow(page, CONVERGED_DESTINATION_NAME)).toHaveClass(/selected/);
  await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible();
  expect(await observations(page, 'create_entity')).toHaveLength(1);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

Then('the listed NPC changes remain unsaved', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    ASSIGNED_DESTINATION_DRAFT_NOTES,
  );
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

When('I save the assigned NPC changes before creation is acknowledged', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  await expect
    .poll(async () => {
      const assigned = await persisted<{ notes: string }>(page, 'update_entity', 'created-3');
      return assigned?.notes;
    })
    .toBe(ASSIGNED_DESTINATION_DRAFT_NOTES);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  expect(await activeWrites(page, 'create_entity')).toBe(1);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

When('that destination Update is acknowledged as saved', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  await expect
    .poll(async () => {
      const assigned = await persisted<{ notes: string }>(page, 'update_entity', 'created-3');
      return assigned?.notes;
    })
    .toBe(ASSIGNED_DESTINATION_DRAFT_NOTES);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  expect(await activeWrites(page, 'create_entity')).toBe(1);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

Then('the newer assigned NPC changes remain visible and saved', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    ASSIGNED_DESTINATION_DRAFT_NOTES,
  );
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  await expect(page.getByText('Unsaved changes', { exact: true })).toHaveCount(0);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

Then('the destination Update remains visible as saved', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    ASSIGNED_DESTINATION_DRAFT_NOTES,
  );
  await expect(page.getByRole('status').filter({ hasText: 'Saved' })).toBeVisible();
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

Then('the backend retains the newer assigned NPC changes', async ({ page }) => {
  const assigned = await persisted<{ notes: string }>(page, 'update_entity', 'created-3');
  expect(assigned?.notes).toBe(ASSIGNED_DESTINATION_DRAFT_NOTES);
  const updates = await observations(page, 'update_entity');
  expect(updates).toHaveLength(1);
  expect(updates[0]?.id).toBe('created-3');
});

Then('the destination Update remains persisted for the assigned NPC', async ({ page }) => {
  const assigned = await persisted<{ notes: string }>(page, 'update_entity', 'created-3');
  expect(assigned?.notes).toBe(ASSIGNED_DESTINATION_DRAFT_NOTES);
  const updates = await observations(page, 'update_entity');
  expect(updates).toHaveLength(1);
  expect(updates[0]?.id).toBe('created-3');
});

Then('the original new NPC draft remains available', async ({ page }) => {
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(2);
});

Then('the original revision 2 draft remains available', async ({ page }) => {
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(2);
  const originalDraftRow = entityRow(page, CREATE_REVISION_ONE_NAME).last();
  await originalDraftRow
    .getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true })
    .click();
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true }),
  ).toHaveValue(CREATE_REVISION_TWO_NOTES);
});

Then('the renamed revision 2 draft remains available', async ({ page }) => {
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(1);
  const sourceDraftRow = entityRow(page, CREATE_REVISION_TWO_NAME);
  await expect(sourceDraftRow).toHaveCount(1);
  await sourceDraftRow.getByRole('button', { name: CREATE_REVISION_TWO_NAME, exact: true }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CREATE_REVISION_TWO_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    CREATE_REVISION_TWO_NOTES,
  );
});

Then('I see that the NPC was created but needs attention with Retry', async ({ page }) => {
  const originalDraftRow = entityRow(page, CREATE_REVISION_ONE_NAME).last();
  await originalDraftRow
    .getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true })
    .click();
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  await expect(alert).toBeVisible();
  await expect(alert.getByRole('button', { name: 'Retry' })).toBeVisible();
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true }),
  ).toHaveValue(CREATE_REVISION_ONE_NOTES);
});

Then('I see that the NPC was created but needs attention', async ({ page }) => {
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  await expect(alert).toBeVisible();
  await expect(alert.getByRole('button', { name: 'Keep saved record' })).toBeVisible();
  await expect(alert.getByRole('button', { name: 'Keep my draft' })).toBeVisible();
  await expect(alert.getByRole('button', { name: 'Retry' })).toHaveCount(0);
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true }),
  ).toHaveValue(CREATE_REVISION_TWO_NOTES);
});

When('I keep the saved NPC record', async ({ page }) => {
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  await alert.getByRole('button', { name: 'Keep saved record' }).click();
  await expect(alert).toHaveCount(0);
});

When('I activate Keep saved record with the keyboard', async ({ page }) => {
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  const choice = alert.getByRole('button', { name: 'Keep saved record' });
  await choice.focus();
  await expect(choice).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(alert).toHaveCount(0);
});

Then("focus moves to the saved NPC's Save action", async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('button', { name: 'Save', exact: true }),
  ).toBeFocused();
});

When('I keep my NPC draft', async ({ page }) => {
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  expect(await observations(page, 'create_entity')).toHaveLength(1);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
  await alert.getByRole('button', { name: 'Keep my draft' }).click();
  await expect(alert).toHaveCount(0);
});

When('I activate Keep my draft with the keyboard', async ({ page }) => {
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  const choice = alert.getByRole('button', { name: 'Keep my draft' });
  await choice.focus();
  await expect(choice).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(alert).toHaveCount(0);
});

Then("focus moves to the kept NPC's Save action", async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('button', { name: 'Save', exact: true }),
  ).toBeFocused();
});

Then('the kept revision 2 name, notes, and preview remain visible as unsaved', async ({ page }) => {
  const keptRow = entityRow(page, CREATE_REVISION_TWO_NAME);
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect.soft(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(0);
  await expect.soft(keptRow).toHaveCount(1);
  await expect.soft(keptRow).toHaveClass(/selected/);
  await expect
    .soft(form.getByRole('textbox', { name: 'Name', exact: true }))
    .toHaveValue(CREATE_REVISION_TWO_NAME);
  await expect
    .soft(form.getByRole('textbox', { name: 'Notes', exact: true }))
    .toHaveValue(CREATE_REVISION_TWO_NOTES);
  await expect.soft(page.getByText(CREATE_REVISION_TWO_NOTES, { exact: true })).toBeVisible();
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

Then('the saved destination is unchanged without another backend write', async ({ page }) => {
  const assigned = await persisted<{ name: string; notes: string }>(
    page,
    'update_entity',
    'created-3',
  );
  expect(assigned).toMatchObject({
    name: CREATE_REVISION_ONE_NAME,
    notes: ASSIGNED_DESTINATION_DRAFT_NOTES,
  });
  expect(await observations(page, 'create_entity')).toHaveLength(1);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

When('I save the kept revision 2', async ({ page }) => {
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('button', { name: 'Save', exact: true })
    .click();
});

Then('the kept revision 2 updates the NPC with the assigned ID exactly once', async ({ page }) => {
  await expect
    .poll(async () => {
      const assigned = await persisted<{ name: string; notes: string }>(
        page,
        'update_entity',
        'created-3',
      );
      return assigned ? { name: assigned.name, notes: assigned.notes } : null;
    })
    .toEqual({ name: CREATE_REVISION_TWO_NAME, notes: CREATE_REVISION_TWO_NOTES });
  const updates = await observations(page, 'update_entity');
  expect(updates).toHaveLength(2);
  expect(updates.at(-1)?.id).toBe('created-3');
  expect(updates.at(-1)?.input).toMatchObject({
    name: CREATE_REVISION_TWO_NAME,
    notes: CREATE_REVISION_TWO_NOTES,
  });
});

Then('conflict resolution has not sent another backend write', async ({ page }) => {
  expect(await observations(page, 'create_entity')).toHaveLength(1);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

Then(
  'the original revision 2 draft is discarded without another backend write',
  async ({ page }) => {
    await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(1);
    expect(await observations(page, 'create_entity')).toHaveLength(1);
    expect(await observations(page, 'update_entity')).toHaveLength(1);
  },
);

When('I explicitly discard the listed NPC changes', async ({ page }) => {
  const assignedRow = entityRow(page, CREATE_REVISION_ONE_NAME).first();
  await assignedRow.getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    ASSIGNED_DESTINATION_DRAFT_NOTES,
  );
  await form.getByRole('button', { name: 'Cancel', exact: true }).click();
  const dialog = unsavedChangesDialog(page);
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Discard changes' }).click();
  await expect(dialog).toHaveCount(0);
});

When('I retry finishing the created NPC', async ({ page }) => {
  const originalDraftRow = entityRow(page, CREATE_REVISION_ONE_NAME).last();
  await originalDraftRow
    .getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true })
    .click();
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  await expect(alert).toBeVisible();
  await alert.getByRole('button', { name: 'Retry' }).click();
  await expect(alert).toHaveCount(0);
  expect(await observations(page, 'create_entity')).toHaveLength(1);
  expect(await observations(page, 'update_entity')).toHaveLength(0);
});

Then('the original new NPC draft is promoted to the assigned NPC', async ({ page }) => {
  await expect(entityRow(page, CREATE_REVISION_ONE_NAME)).toHaveCount(1);
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    CREATE_REVISION_ONE_NOTES,
  );
  await expect(form.getByRole('button', { name: 'Save', exact: true })).toBeVisible();
  await expect(page.getByText('Created, but needs attention', { exact: true })).toHaveCount(0);
});

Then('no additional NPC is created', async ({ page }) => {
  expect(await observations(page, 'create_entity')).toHaveLength(1);
  expect(await observations(page, 'update_entity')).toHaveLength(0);
});

Given(
  'a created NPC needs attention because its listed destination had unsaved work',
  async ({ page }) => {
    await openBlockedCreatePromotion(page);
  },
);

Given(
  'a created NPC needs attention because its listed destination still has unsaved work',
  async ({ page }) => {
    await openBlockedCreatePromotion(page);
  },
);

Given('I have resolved the listed destination changes', async ({ page }) => {
  const destinationRow = entityRow(page, CREATE_REVISION_ONE_NAME).first();
  await destinationRow.getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    ASSIGNED_DESTINATION_DRAFT_NOTES,
  );
  await form.getByRole('button', { name: 'Cancel', exact: true }).click();
  const dialog = unsavedChangesDialog(page);
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Discard changes' }).click();
  await expect(dialog).toHaveCount(0);
  await entityRow(page, CREATE_REVISION_ONE_NAME)
    .last()
    .getByRole('button', { name: CREATE_REVISION_ONE_NAME, exact: true })
    .click();
  await expect(
    page
      .getByRole('alert')
      .filter({ hasText: 'Created, but needs attention' })
      .getByRole('button', { name: 'Retry' }),
  ).toBeVisible();
});

Given('the promotion Retry action has focus', async ({ page }) => {
  const retry = page
    .getByRole('alert')
    .filter({ hasText: 'Created, but needs attention' })
    .getByRole('button', { name: 'Retry' });
  await retry.focus();
  await expect(retry).toBeFocused();
});

When('I activate promotion Retry with the keyboard', async ({ page }) => {
  const retry = page
    .getByRole('alert')
    .filter({ hasText: 'Created, but needs attention' })
    .getByRole('button', { name: 'Retry' });
  await retry.press('Enter');
});

Then('focus moves to the promoted NPC editor', async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('button', { name: 'Save', exact: true }),
  ).toBeFocused();
});

Then('the promotion failure remains actionable', async ({ page }) => {
  const alert = page.getByRole('alert').filter({ hasText: 'Created, but needs attention' });
  await expect(alert).toBeVisible();
  await expect(alert.getByRole('button', { name: 'Retry' })).toBeEnabled();
});

Then('focus remains on the promotion Retry action', async ({ page }) => {
  await expect(
    page
      .getByRole('alert')
      .filter({ hasText: 'Created, but needs attention' })
      .getByRole('button', { name: 'Retry' }),
  ).toBeFocused();
});

When('I edit and save the promoted NPC', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(POST_PROMOTION_NOTES);
  await form.getByRole('button', { name: 'Save', exact: true }).click();
  await expect
    .poll(async () => {
      const created = await persisted<{ notes: string }>(page, 'update_entity', 'created-3');
      return created?.notes;
    })
    .toBe(POST_PROMOTION_NOTES);
});

Then('the later edit updates the NPC with the assigned ID', async ({ page }) => {
  const updates = await observations(page, 'update_entity');
  expect(updates).toHaveLength(1);
  expect(updates[0]?.id).toBe('created-3');
  const created = await persisted<{ notes: string }>(page, 'update_entity', 'created-3');
  expect(created?.notes).toBe(POST_PROMOTION_NOTES);
});

When('I navigate to Oracle', async ({ page }) => {
  await openRailView(page, 'Oracle');
});

When('I return to NPCs before creation completes', async ({ page }) => {
  await openRailView(page, 'NPCs');
  expect(await activeWrites(page, 'create_entity')).toBe(1);
});

Then('the new NPC draft is restored with Create unavailable', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CREATE_REVISION_ONE_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    CREATE_REVISION_ONE_NOTES,
  );
  await expect(form.getByRole('button', { name: 'Create', exact: true })).toBeDisabled();
  await expect(page.getByText('Saving…', { exact: true })).toBeVisible();

  const calls = await page.evaluate(() =>
    (window as unknown as DraftWindow).__ipcCalls.map(({ cmd }) => cmd),
  );
  const createIndex = calls.indexOf('create_entity');
  expect(createIndex).toBeGreaterThanOrEqual(0);
  expect(calls.slice(createIndex + 1)).toContain('get_entities');
  expect(await persisted(page, 'create_entity', 'created-3')).toBeUndefined();
  pagesWithPreAcknowledgmentEntityList.add(page);
});

When('the entity list loaded before completion does not contain that ID', async ({ page }) => {
  expect(pagesWithPreAcknowledgmentEntityList.has(page)).toBe(true);
});

Then('the acknowledged NPC remains visible and selected', async ({ page }) => {
  const row = entityRow(page, CREATE_REVISION_ONE_NAME);
  await expect(row).toBeVisible();
  await expect(row).toHaveClass(/selected/);
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    CREATE_REVISION_ONE_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    CREATE_REVISION_ONE_NOTES,
  );
});

Then('the interface indicates that it is saved', async ({ page }) => {
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  await expect(page.getByText('Unsaved changes', { exact: true })).toHaveCount(0);
});

Given('I have a dirty new NPC draft', async ({ page }) => {
  await openRailView(page, 'NPCs');
  await page.getByRole('button', { name: 'New NPC' }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill(NEW_ENTITY_NAME);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(NEW_ENTITY_NOTES);
});

Given('I am viewing the existing NPC {string}', async ({ page }, name: string) => {
  await entityRow(page, name).getByRole('button', { name, exact: true }).click();
  await expect(page.getByRole('form', { name: 'Entity form' })).toBeVisible();
});

When('I request creation of the missing NPC link {string}', async ({ page }, name: string) => {
  await page.getByRole('button', { name: `Create article for ${name}` }).click();
  await page.locator('[data-testid="create-entity-chooser"] [data-entity-kind="npc"]').click();
});

Then('I am asked whether to keep or discard my new NPC draft', async ({ page }) => {
  const dialog = page.getByRole('dialog', { name: /Discard unsaved changes/i });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Keep editing' })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Discard and create' })).toBeVisible();
});

Then(
  'Discard and create is unavailable while the original creation is saving',
  async ({ page }) => {
    const discardAndCreate = page
      .getByRole('dialog', { name: /Discard unsaved changes/i })
      .getByRole('button', { name: 'Discard and create' });
    await expect(discardAndCreate).toBeDisabled();
  },
);

Then('Discard and create is explicitly described by the wait message', async ({ page }) => {
  const discardAndCreate = page
    .getByRole('dialog', { name: /Discard unsaved changes/i })
    .getByRole('button', { name: 'Discard and create' });
  await expect(discardAndCreate).toHaveAttribute('aria-describedby', 'entity-new-discard-blocked');
  await expect(discardAndCreate).toHaveAccessibleDescription(
    'Wait for saving to finish before discarding changes',
  );
});

Then('Discard and create becomes available after creation settles', async ({ page }) => {
  await expect(
    page
      .getByRole('dialog', { name: /Discard unsaved changes/i })
      .getByRole('button', { name: 'Discard and create' }),
  ).toBeEnabled();
});

Then('the completed original NPC remains saved without a duplicate creation', async ({ page }) => {
  const created = await persisted<{ name: string; notes: string }>(
    page,
    'create_entity',
    'created-3',
  );
  expect(created).toMatchObject({
    name: CREATE_REVISION_ONE_NAME,
    notes: CREATE_REVISION_ONE_NOTES,
  });
  expect(await observations(page, 'create_entity')).toHaveLength(1);
});

Then('Keep editing has focus', async ({ page }) => {
  await expect(
    page
      .getByRole('dialog', { name: /Discard unsaved changes/i })
      .getByRole('button', { name: 'Keep editing' }),
  ).toBeFocused();
});

Then('the replacement confirmation closes', async ({ page }) => {
  await expect(page.getByRole('dialog', { name: /Discard unsaved changes/i })).toHaveCount(0);
});

Then('focus returns to the Create control that invoked it', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Create article for Aldric' })).toBeFocused();
});

Then('my original new NPC draft remains unchanged', async ({ page }) => {
  await page.getByRole('button', { name: 'New NPC' }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    NEW_ENTITY_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    NEW_ENTITY_NOTES,
  );
  expect(await observations(page, 'create_entity')).toHaveLength(0);
});

When('I choose to keep editing', async ({ page }) => {
  await page
    .getByRole('dialog', { name: /Discard unsaved changes/i })
    .getByRole('button', { name: 'Keep editing' })
    .click();
});

Then('my original new NPC draft is reopened unchanged', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    NEW_ENTITY_NAME,
  );
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    NEW_ENTITY_NOTES,
  );
});

Then('no NPC named {string} has been created', async ({ page }, name: string) => {
  const creates = await observations(page, 'create_entity');
  expect(creates.some((call) => (call.input as { name?: string } | undefined)?.name === name)).toBe(
    false,
  );
  await expect(entityRow(page, name)).toHaveCount(0);
});

When('I choose to discard the draft and create', async ({ page }) => {
  await page
    .getByRole('dialog', { name: /Discard unsaved changes/i })
    .getByRole('button', { name: 'Discard and create' })
    .click();
});

Then('only my original new NPC draft is discarded', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).not.toHaveValue(
    NEW_ENTITY_NAME,
  );
  const mira = await persisted<{ notes: string }>(page, 'update_entity', 'mira');
  const torvin = await persisted<{ notes: string }>(page, 'update_entity', 'torvin');
  expect(mira?.notes).toBe(MIRA_SAVED_NOTES);
  expect(torvin?.notes).toBe(TORVIN_SAVED_NOTES);
});

Then('a new NPC draft is open with the name {string}', async ({ page }, name: string) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(name);
  await expect(form.getByRole('button', { name: 'Create', exact: true })).toBeVisible();
});

Then('no NPC named {string} has been created yet', async ({ page }, name: string) => {
  const creates = await observations(page, 'create_entity');
  expect(creates.some((call) => (call.input as { name?: string } | undefined)?.name === name)).toBe(
    false,
  );
  await expect(entityRow(page, name)).toHaveCount(0);
});

When('I open entity {string}', async ({ page }, name: string) => {
  await page.getByRole('button', { name, exact: true }).click();
});

Then("Mira's draft is not shown for Torvin", async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    TORVIN_SAVED_NOTES,
  );
});

When('I return to entity {string}', async ({ page }, name: string) => {
  await page.getByRole('button', { name, exact: true }).click();
});

Then("Torvin's saved content is unchanged", async ({ page }) => {
  const torvin = await persisted<{ notes: string }>(page, 'update_entity', 'torvin');
  expect(torvin?.notes).toBe(TORVIN_SAVED_NOTES);
});

Given('I have tried to save entity {string}', async ({ page }, name: string) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  expect(name).toBe('Mira');
});

Then("Mira's name field indicates that it is required", async ({ page }) => {
  await expect(page.getByText('Name is required.', { exact: true })).toBeVisible();
});

Then("Torvin's saved name is shown", async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('Torvin');
});

Then("Mira's required-name message is not shown for Torvin", async ({ page }) => {
  await expect(page.getByText('Name is required.', { exact: true })).toHaveCount(0);
});

Then("Mira's blank-name draft remains available as unsaved", async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('');
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

Then(
  'the frontend required-name message is cleared until I try to save again',
  async ({ page }) => {
    await expect(page.getByText('Name is required.', { exact: true })).toHaveCount(0);
    expect(await observations(page, 'update_entity')).toHaveLength(0);
  },
);

Then("Mira's row indicates unsaved changes", async ({ page }) => {
  await expect(entityRow(page, 'Mira')).toContainText('Unsaved changes');
});

When('I reopen entity {string} and start saving', async ({ page }, name: string) => {
  await hold(page, 'update_entity');
  await entityRow(page, name).getByRole('button', { name, exact: true }).click();
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('button', { name: 'Save', exact: true })
    .click();
  await expect.poll(() => activeWrites(page, 'update_entity')).toBe(1);
});

When('I open entity {string} before the save completes', async ({ page }, name: string) => {
  await entityRow(page, name).getByRole('button', { name, exact: true }).click();
  expect(await activeWrites(page, 'update_entity')).toBe(1);
});

Then("Mira's row indicates saving", async ({ page }) => {
  await expect(entityRow(page, 'Mira')).toContainText('Saving…');
});

When('the save fails', async ({ page }) => {
  await rejectPending(page, 'update_entity', {
    code: 'DATABASE',
    message: 'Entity write failed while the form was closed.',
  });
  await expect.poll(() => activeWrites(page, 'update_entity')).toBe(0);
});

Then("Mira's row indicates that saving failed", async ({ page }) => {
  await expect(entityRow(page, 'Mira')).toContainText(/couldn't save|failed/i);
});

Then('reopening Mira restores the failed draft and Retry action', async ({ page }) => {
  await entityRow(page, 'Mira').getByRole('button', { name: 'Mira', exact: true }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    MIRA_DRAFT_NOTES,
  );
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

When("I restore Mira's notes to the saved value", async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_SAVED_NOTES);
});

Then('the interface no longer indicates unsaved changes', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toHaveCount(0);
});

Then('no save has been sent for Mira', async ({ page }) => {
  expect(await observations(page, 'update_entity')).toHaveLength(0);
});

Given('I have unsaved changes to entity {string}', async ({ page }, name: string) => {
  const form = await openEntity(page, name);
  await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_DRAFT_NOTES);
});

Given('saving my changes to entity {string} has failed', async ({ page }, name: string) => {
  expect(name).toBe('Mira');
  await openFailedMiraSave(page);
});

Given('the inline Retry action has focus', async ({ page }) => {
  const retry = page.getByRole('alert').getByRole('button', { name: 'Retry' });
  await retry.focus();
  await expect(retry).toBeFocused();
});

When('saving the entity fails', async ({ page }) => {
  await rejectNext(page, 'update_entity', { code: 'DATABASE', message: 'Database unavailable.' });
  await page.getByRole('button', { name: 'Save', exact: true }).click();
});

Then('my changes remain available', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    MIRA_DRAFT_NOTES,
  );
});

Then('I see an actionable save failure', async ({ page }) => {
  await expect(page.getByRole('alert')).toContainText(/couldn't save|database unavailable/i);
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

When('I retry and saving succeeds', async ({ page }) => {
  const retry = page.getByRole('button', { name: 'Retry' });
  if (await retry.evaluate((element) => element === document.activeElement)) {
    await retry.press('Enter');
  } else {
    await retry.click();
  }
  await expect
    .poll(async () => {
      const mira = await persisted<{ notes: string }>(page, 'update_entity', 'mira');
      return mira?.notes;
    })
    .toBe(MIRA_DRAFT_NOTES);
});

When('I retry and saving fails again', async ({ page }) => {
  await rejectNext(page, 'update_entity', {
    code: 'DATABASE',
    message: 'Database is still unavailable.',
  });
  const retry = page.getByRole('alert').getByRole('button', { name: 'Retry' });
  await expect(retry).toBeFocused();
  await retry.press('Enter');
  await expect(page.getByRole('alert')).toContainText('Database is still unavailable.');
});

Then('the changes are saved to entity {string}', async ({ page }, name: string) => {
  expect(name).toBe('Mira');
  const mira = await persisted<{ notes: string }>(page, 'update_entity', 'mira');
  expect(mira?.notes).toBe(MIRA_DRAFT_NOTES);
});

Then('the failure indication is cleared', async ({ page }) => {
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
});

Then("focus moves to Mira's stable editor control", async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('button', { name: 'Save', exact: true }),
  ).toBeFocused();
});

Then('the actionable failure remains', async ({ page }) => {
  const alert = page.getByRole('alert');
  await expect(alert).toContainText(/couldn't save|still unavailable/i);
  await expect(alert.getByRole('button', { name: 'Retry' })).toBeEnabled();
});

Then("focus remains on Mira's Retry action", async ({ page }) => {
  await expect(page.getByRole('alert').getByRole('button', { name: 'Retry' })).toBeFocused();
});

Given(
  'I have frontend-valid unsaved changes to entity {string}',
  async ({ page }, name: string) => {
    const form = await openEntity(page, name);
    await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_DRAFT_NOTES);
  },
);

When('the backend rejects the entity save as invalid', async ({ page }) => {
  await rejectNext(page, 'update_entity', {
    code: 'VALIDATION',
    field: 'notes',
    message: 'Notes contain an unsupported construct.',
  });
  await page.getByRole('button', { name: 'Save', exact: true }).click();
});

Given(
  'saving frontend-valid changes to entity {string} is in progress',
  async ({ page }, name: string) => {
    const form = await openEntity(page, name);
    await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_DRAFT_NOTES);
    await hold(page, 'update_entity');
    await form.getByRole('button', { name: 'Save', exact: true }).click();
    await expect.poll(() => activeWrites(page, 'update_entity')).toBe(1);
  },
);

When("the backend rejects Mira's save as invalid", async ({ page }) => {
  await rejectPending(page, 'update_entity', {
    code: 'VALIDATION',
    field: 'name',
    message: "Mira's name is not valid.",
  });
  await expect.poll(() => activeWrites(page, 'update_entity')).toBe(0);
  await expect(entityRow(page, 'Mira')).toContainText(/couldn't save|failed/i);
});

Then("Torvin's saved content remains visible without Mira's error", async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('Torvin');
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    TORVIN_SAVED_NOTES,
  );
  await expect(page.getByText("Mira's name is not valid.", { exact: true })).toHaveCount(0);
});

Then("Mira's changed content remains available", async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true }),
  ).toHaveValue(MIRA_DRAFT_NOTES);
});

Then('Mira shows the save failure and Retry action', async ({ page }) => {
  await expect(page.getByRole('alert')).toContainText(/couldn't save|Mira's name is not valid/i);
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

Then('the same changes are saved to entity {string}', async ({ page }, name: string) => {
  expect(name).toBe('Mira');
  const mira = await persisted<{ notes: string }>(page, 'update_entity', 'mira');
  expect(mira?.notes).toBe(MIRA_DRAFT_NOTES);
});

Given('I have cleared the required name for entity {string}', async ({ page }, name: string) => {
  const form = await openEntity(page, name);
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill('');
});

When('I try to save the entity', async ({ page }) => {
  await page.getByRole('button', { name: 'Save', exact: true }).click();
});

Then('the name field indicates that it is required', async ({ page }) => {
  await expect(page.getByText('Name is required.', { exact: true })).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('');
});

Then('Mira remains marked with unsaved changes', async ({ page }) => {
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
});

Then('no entity save has been sent', async ({ page }) => {
  expect(await observations(page, 'update_entity')).toHaveLength(0);
});

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

When('saving reports that entity {string} is no longer available', async ({ page }, name) => {
  expect(name).toBe('Mira');
  await rejectNext(page, 'update_entity', {
    code: 'NOT_FOUND',
    message: 'Entity Mira is no longer available.',
  });
  await page.getByRole('button', { name: 'Save', exact: true }).click();
});

Then('I see that the target is unavailable', async ({ page }) => {
  await expect(page.getByRole('alert')).toContainText(/no longer available/i);
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

Then('no other entity is changed', async ({ page }) => {
  const torvin = await persisted<{ notes: string }>(page, 'update_entity', 'torvin');
  expect(torvin?.notes).toBe(TORVIN_SAVED_NOTES);
  expect(await observations(page, 'update_entity')).toHaveLength(1);
});

Given(
  'saving my draft for entity {string} reports that the target is unavailable',
  async ({ page }, name: string) => {
    expect(name).toBe('Mira');
    const form = await openEntity(page, name);
    await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(MIRA_DRAFT_NOTES);
    await removeEntity(page, 'mira');
    await form.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(page.getByRole('alert')).toContainText(/no longer available/i);
  },
);

When('I navigate away and the entity list reloads without {string}', async ({ page }, name) => {
  expect(name).toBe('Mira');
  await openRailView(page, 'Oracle');
  expect(await persisted(page, 'update_entity', 'mira')).toBeUndefined();
});

When('I return to NPCs', async ({ page }) => {
  await openRailView(page, 'NPCs');
});

Then(
  'an unavailable row for {string} indicates that saving failed',
  async ({ page }, name: string) => {
    const row = entityRow(page, name);
    await expect(row).toBeVisible();
    await expect(row).toContainText(/couldn't save|failed/i);
    await expect(row).toContainText(/unavailable|no longer available/i);
  },
);

When('I open the unavailable row', async ({ page }) => {
  await entityRow(page, 'Mira').getByRole('button', { name: 'Mira', exact: true }).click();
});

Then('my changes remain available with Retry', async ({ page }) => {
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    MIRA_DRAFT_NOTES,
  );
  await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
});

Then('Retry still targets entity {string}', async ({ page }, name: string) => {
  expect(name).toBe('Mira');
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect
    .poll(() => observations(page, 'update_entity').then((writes) => writes.length))
    .toBe(2);
  const writes = await observations(page, 'update_entity');
  expect(writes.at(-1)?.id).toBe('mira');
  const torvin = await persisted<{ notes: string }>(page, 'update_entity', 'torvin');
  expect(torvin?.notes).toBe(TORVIN_SAVED_NOTES);
});

When("I explicitly discard Mira's changes", async ({ page }) => {
  const dialog = unsavedChangesDialog(page);
  if (!(await dialog.isVisible())) {
    await page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('button', {
        name: 'Cancel',
        exact: true,
      })
      .click();
  }
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Discard changes' }).click();
});

Then("Mira's saved version is restored", async ({ page }) => {
  const form = await openEntity(page, 'Mira');
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    MIRA_SAVED_NOTES,
  );
});

Then('the unsent Oracle question remains intact', async ({ page }) => {
  await openRailView(page, 'Oracle');
  await expect(composer(page)).toHaveValue(CAMPAIGN_A_QUESTION);
  expect(await submissions(page)).toHaveLength(0);
});

Given(
  'a save of an earlier entity {string} draft revision is in progress',
  async ({ page }, name: string) => {
    expect(name).toBe('Mira');
    const form = await openEntity(page, name);
    await hold(page, 'update_entity');
    await form.getByRole('textbox', { name: 'Notes', exact: true }).fill(EARLIER_MIRA_NOTES);
    await form.getByRole('button', { name: 'Save', exact: true }).click();
    await expect.poll(() => activeWrites(page, 'update_entity')).toBe(1);
  },
);

When('I make a newer edit to entity {string}', async ({ page }, name: string) => {
  expect(name).toBe('Mira');
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('textbox', { name: 'Notes', exact: true })
    .fill(NEWER_MIRA_NOTES);
});

Then('Discard changes is unavailable for Mira', async ({ page }) => {
  await page
    .getByRole('form', { name: 'Entity form' })
    .getByRole('button', {
      name: 'Cancel',
      exact: true,
    })
    .click();
  const dialog = unsavedChangesDialog(page);
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Discard changes' })).toBeDisabled();
});

Then('I am told to wait for saving to finish', async ({ page }) => {
  await expect(unsavedChangesDialog(page)).toContainText(
    'Wait for saving to finish before discarding changes',
  );
});

When('the earlier entity save completes', async ({ page }) => {
  await resolveNext(page, 'update_entity');
  await expect.poll(() => activeWrites(page, 'update_entity')).toBe(0);
});

Then('my newer entity edit remains visible as unsaved', async ({ page }) => {
  await expect(
    page
      .getByRole('form', { name: 'Entity form' })
      .getByRole('textbox', { name: 'Notes', exact: true }),
  ).toHaveValue(NEWER_MIRA_NOTES);
  await expect(page.getByText('Unsaved changes', { exact: true }).first()).toBeVisible();
});

Then('Discard changes is available for Mira', async ({ page }) => {
  await expect(
    unsavedChangesDialog(page).getByRole('button', { name: 'Discard changes' }),
  ).toBeEnabled();
});

Then('the version acknowledged by the completed save is restored', async ({ page }) => {
  const form = await openEntity(page, 'Mira');
  await expect(form.getByRole('textbox', { name: 'Notes', exact: true })).toHaveValue(
    EARLIER_MIRA_NOTES,
  );
  const mira = await persisted<{ notes: string }>(page, 'update_entity', 'mira');
  expect(mira?.notes).toBe(EARLIER_MIRA_NOTES);
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

Given('an unsent Oracle question has focus', async () => {
  skipNativeCloseContract();
});

async function nativeCloseOnly(): Promise<void> {
  throw new Error('This step belongs to the native tauri-driver close contract.');
}

When('a normal window close is requested', nativeCloseOnly);
Then('closing is paused by an unsaved-work dialog', nativeCloseOnly);
Then('Cancel has focus', nativeCloseOnly);
When('I press Escape', async ({ page }) => {
  await page.keyboard.press('Escape');
});
Then('the dialog closes', nativeCloseOnly);
Then('focus returns to the Oracle composer', nativeCloseOnly);
When('I request window closing again', nativeCloseOnly);
When('I activate {string} with the keyboard', async ({ page: _page }, _label: string) => {
  await nativeCloseOnly();
});
Then('the retained draft is discarded and window closing proceeds', nativeCloseOnly);
Then('no draft is submitted or saved during closing', nativeCloseOnly);
Given('I have made a newer unsaved edit to the session', nativeCloseOnly);
Then('Discard and close is unavailable', nativeCloseOnly);
Then('I am told to wait for saving to finish before closing', nativeCloseOnly);
When('the earlier session save completes', nativeCloseOnly);
Then('closing remains paused', nativeCloseOnly);
Then('my newer session edit remains unsaved', nativeCloseOnly);
Then('Discard and close becomes available', nativeCloseOnly);
Then('window closing proceeds', nativeCloseOnly);
Then('closing did not start another save', nativeCloseOnly);
