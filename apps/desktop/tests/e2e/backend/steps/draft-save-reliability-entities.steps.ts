import { expect, type Locator, type Page } from '@playwright/test';
import { Given, Then, When } from './fixtures';
import {
  CAMPAIGN_A_QUESTION,
  type DraftWindow,
  SESSION_REVISION_TWO_TITLE,
  activeWrites,
  commitPending,
  composer,
  hold,
  holdNextEntityList,
  observations,
  openCampaign,
  openRailView,
  pendingEntityLists,
  pendingWrites,
  persisted,
  rejectNext,
  rejectPending,
  removeEntity,
  resolveNext,
  resolveNextEntityList,
  resolveNextWithCanonicalInput,
  setEntityCanonical,
  submissions,
} from './draft-reliability-support';

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
const pagesWithPreAcknowledgmentEntityList = new WeakSet<Page>();

async function openEntity(page: Page, name: string): Promise<Locator> {
  await openRailView(page, 'NPCs');
  await page.getByRole('button', { name, exact: true }).click();
  const form = page.getByRole('form', { name: 'Entity form' });
  await expect(form).toBeVisible();
  return form;
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
