import { type Locator, type Page } from '@playwright/test';
import { type DraftReliabilityControls, type DraftWriteCommand } from '../ipc-mock';
import { test } from './fixtures';

export const CAMPAIGN_A_QUESTION = 'Campaign A secret question';
export const SESSION_REVISION_TWO_TITLE = 'Session draft revision two';

export interface DraftWindow extends Window {
  __draftReliability: DraftReliabilityControls;
  __ipcCalls: Array<{ cmd: string; args?: Record<string, unknown> }>;
}

export async function hold(page: Page, command: DraftWriteCommand): Promise<void> {
  await page.evaluate((commandName) => {
    (window as unknown as DraftWindow).__draftReliability.hold(commandName);
  }, command);
}

export async function commitPending(
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

export async function rejectNext(page: Page, command: string, error: unknown): Promise<void> {
  await page.evaluate(
    ({ commandName, rejection }) => {
      (window as unknown as DraftWindow).__draftReliability.rejectNext(commandName, rejection);
    },
    { commandName: command, rejection: error },
  );
}

export async function rejectPending(
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

export async function removeEntity(page: Page, id: string): Promise<void> {
  await page.evaluate((recordId) => {
    (window as unknown as DraftWindow).__draftReliability.removeEntity(recordId);
  }, id);
}

export async function removeSession(page: Page, id: string): Promise<void> {
  await page.evaluate((recordId) => {
    (window as unknown as DraftWindow).__draftReliability.removeSession(recordId);
  }, id);
}

export async function removeRule(page: Page, id: string): Promise<void> {
  await page.evaluate((recordId) => {
    (window as unknown as DraftWindow).__draftReliability.removeRule(recordId);
  }, id);
}

export async function setEntityCanonical(
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

export async function holdNextEntityList(
  page: Page,
  campaignId: string,
  kind: string,
): Promise<void> {
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

export async function holdNextEntityListWithout(
  page: Page,
  campaignId: string,
  kind: string,
  omittedId: string,
): Promise<void> {
  await page.evaluate(
    ({ heldCampaignId, heldKind, recordId }) => {
      (window as unknown as DraftWindow).__draftReliability.holdNextEntityList(
        heldCampaignId,
        heldKind,
        recordId,
      );
    },
    { heldCampaignId: campaignId, heldKind: kind, recordId: omittedId },
  );
}

export async function resolveNextEntityList(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as unknown as DraftWindow).__draftReliability.resolveNextEntityList();
  });
}

export async function holdNextSessionList(page: Page, campaignId: string): Promise<void> {
  await page.evaluate((heldCampaignId) => {
    (window as unknown as DraftWindow).__draftReliability.holdNextSessionList(heldCampaignId);
  }, campaignId);
}

export async function resolveNextSessionList(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as unknown as DraftWindow).__draftReliability.resolveNextSessionList();
  });
}

export async function pendingSessionLists(page: Page): Promise<number> {
  return page.evaluate(() =>
    (window as unknown as DraftWindow).__draftReliability.pendingSessionLists(),
  );
}

export async function setSessionCanonical(
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

export async function acknowledgeNextRuleAs(page: Page, id: string): Promise<void> {
  await page.evaluate((ruleId) => {
    (window as unknown as DraftWindow).__draftReliability.acknowledgeNextRuleAs(ruleId);
  }, id);
}

export async function pendingEntityLists(page: Page): Promise<number> {
  return page.evaluate(() =>
    (window as unknown as DraftWindow).__draftReliability.pendingEntityLists(),
  );
}

export async function resolveNext(page: Page, command: string): Promise<void> {
  await page.evaluate((commandName) => {
    (window as unknown as DraftWindow).__draftReliability.resolveNext(commandName);
  }, command);
}

export async function resolveNextWithCanonicalInput(
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

export async function activeWrites(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.activeWrites(commandName),
    command,
  );
}

export async function maxConcurrentWrites(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.maxConcurrentWrites(commandName),
    command,
  );
}

export async function pendingWrites(page: Page, command: string): Promise<number> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.pendingWrites(commandName),
    command,
  );
}

export async function persisted<T>(
  page: Page,
  command: string,
  id: string,
): Promise<T | undefined> {
  return page.evaluate(
    ({ commandName, recordId }) =>
      (window as unknown as DraftWindow).__draftReliability.persisted(commandName, recordId) as
        | T
        | undefined,
    { commandName: command, recordId: id },
  );
}

export async function observations(
  page: Page,
  command: string,
): Promise<Array<Record<string, unknown>>> {
  return page.evaluate(
    (commandName) =>
      (window as unknown as DraftWindow).__draftReliability.observations(commandName),
    command,
  );
}

export async function hasEntity(page: Page, id: string, kind: string): Promise<boolean> {
  return page.evaluate(
    ({ recordId, entityKind }) =>
      (window as unknown as DraftWindow).__draftReliability.hasEntity(recordId, entityKind),
    { recordId: id, entityKind: kind },
  );
}

export async function hasCampaign(page: Page, id: string): Promise<boolean> {
  return page.evaluate((campaignId) => {
    return (window as unknown as DraftWindow).__draftReliability.hasCampaign(campaignId);
  }, id);
}

export async function requestApplicationExit(page: Page, intent = 41): Promise<void> {
  await page.evaluate((exitIntent) => {
    (window as unknown as DraftWindow).__draftReliability.requestApplicationExit(exitIntent);
  }, intent);
}

export async function pressChord(page: Page, secondKey: string): Promise<void> {
  await page.keyboard.press('g');
  await page.keyboard.press(secondKey);
}

export async function submissions(page: Page): Promise<Array<Record<string, unknown>>> {
  return page.evaluate(() => (window as unknown as DraftWindow).__draftReliability.submissions());
}

export function composer(page: Page): Locator {
  return page.getByRole('textbox', { name: 'Ask a rule, a name, a place…' });
}

export async function openRailView(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name }).click();
}

export async function openCampaign(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name: 'Switch campaign' }).click();
  const switcher = page.getByRole('dialog', { name: 'Switch campaign' });
  await switcher.getByRole('button', { name }).click();
}
export async function dispatchBlurWithoutMovingFocus(locator: Locator): Promise<void> {
  await locator.evaluate((element) => element.dispatchEvent(new FocusEvent('blur')));
}

export function skipNativeCloseContract(): void {
  test.skip(
    true,
    'Native Tauri close requests cannot be exercised by the mocked browser suite; Task 5 binds this contract in tauri-driver.',
  );
}
