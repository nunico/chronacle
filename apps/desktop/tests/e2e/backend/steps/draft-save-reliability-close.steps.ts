import { Given, Then, When, test } from './fixtures';

function skipNativeCloseContract(): void {
  test.skip(
    true,
    'Native Tauri close requests cannot be exercised by the mocked browser suite; Task 5 binds this contract in tauri-driver.',
  );
}

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
