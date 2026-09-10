import { expect } from '@playwright/test';
import {
  observations,
  requestApplicationExit,
  skipNativeCloseContract,
} from './draft-reliability-support';
import { Given, Then, When } from './fixtures';

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

Given('I make a newer focused edit before the close decision', async ({ page }) => {
  const title = page.getByRole('textbox', { name: 'Name', exact: true });
  await title.fill('Newer focused edit retained during close');
  await expect(title).toBeFocused();
});

When('an application close decision opens in the browser contract', async ({ page }) => {
  await requestApplicationExit(page, 73);
});

Then('closing is paused by the browser unsaved-work dialog', async ({ page }) => {
  await expect(page.getByRole('dialog', { name: 'Unsaved changes' })).toBeVisible();
});

Then('only the earlier session save has started', async ({ page }) => {
  expect(await observations(page, 'update_session')).toHaveLength(1);
});

Then('the newer session edit remains unsaved', async ({ page }) => {
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue(
    'Newer focused edit retained during close',
  );
  await expect(page.getByText('Unsaved changes', { exact: true }).last()).toBeVisible();
});
