// Native close-request coverage for retained frontend drafts.
//
// This must run through tauri-driver: the mocked browser acceptance suite cannot
// emit Tauri's WINDOW_CLOSE_REQUESTED event or prove that destroy closes the
// native window.
import assert from 'node:assert/strict';
import { By, Key, until } from 'selenium-webdriver';
import {
  buildDriver,
  invoke,
  navigateToApp,
  pollUntil,
  startTauriDriver,
  waitForWebviewReady,
} from './driver.mjs';

const ORACLE_BUTTON = By.xpath('//button[normalize-space(.)="Oracle"]');
const ORACLE_COMPOSER = By.xpath('//textarea[contains(@placeholder, "Ask a rule")]');
const CLOSE_DIALOG = By.xpath('//*[@role="dialog" and .//*[normalize-space(.)="Unsaved changes"]]');
const CANCEL = By.xpath('//button[normalize-space(.)="Cancel"]');
const DISCARD_AND_CLOSE = By.xpath('//button[normalize-space(.)="Discard and close"]');

async function requestNativeClose(driver) {
  // This is the same Tauri window command used by getCurrentWindow().close().
  // Unlike destroy, it emits WINDOW_CLOSE_REQUESTED and can be prevented by the
  // frontend close guard.
  await invoke(driver, 'plugin:window|close', { label: 'main' });
}

describe('Draft retention — native window close', function () {
  this.timeout(180000);

  let tauriDriver;
  let driver;

  before(async () => {
    tauriDriver = startTauriDriver();
    driver = await buildDriver();
    await waitForWebviewReady(driver);

    // Bypass the first-run local-model download gate. Oracle draft retention is
    // frontend-only and does not contact the configured embedding provider.
    await invoke(driver, 'update_setting', { key: 'embedding_backend', value: 'openai' });
    await navigateToApp(driver);
  });

  after(async () => {
    if (driver) {
      try {
        await driver.quit();
      } catch {
        // The tested explicit destroy may already have ended the WebDriver window.
      }
    }
    if (tauriDriver) tauriDriver.kill();
  });

  it('cancels or explicitly discards a focused Oracle draft without submitting it', async () => {
    await driver.wait(until.elementLocated(ORACLE_BUTTON), 10000);
    await driver.findElement(ORACLE_BUTTON).click();
    const composer = await driver.wait(until.elementLocated(ORACLE_COMPOSER), 10000);
    const question = 'Where did the silver key go?';
    await composer.sendKeys(question);
    assert.equal(await composer.getAttribute('value'), question);

    await requestNativeClose(driver);
    const dialog = await driver.wait(until.elementLocated(CLOSE_DIALOG), 10000);
    assert.equal(await dialog.isDisplayed(), true, 'close request should remain prevented');
    const cancel = await driver.findElement(CANCEL);
    assert.equal(
      await driver
        .switchTo()
        .activeElement()
        .then((element) => element.getId()),
      await cancel.getId(),
      'Cancel should receive initial focus',
    );

    await driver.actions().sendKeys(Key.ESCAPE).perform();
    await driver.wait(until.stalenessOf(dialog), 10000);
    assert.equal(
      await driver
        .switchTo()
        .activeElement()
        .then((element) => element.getId()),
      await composer.getId(),
      'Escape should restore focus to the exact composer',
    );
    assert.equal(await composer.getAttribute('value'), question, 'Cancel must retain the draft');

    // An unsent question is never submitted merely because close was requested.
    const history = await invoke(driver, 'get_chat_history', { campaignId: null });
    assert.deepEqual(history, []);

    await requestNativeClose(driver);
    const discard = await driver.wait(until.elementLocated(DISCARD_AND_CLOSE), 10000);
    const historyAfterSecondCloseRequest = await invoke(driver, 'get_chat_history', {
      campaignId: null,
    });
    assert.deepEqual(
      historyAfterSecondCloseRequest,
      [],
      'requesting close again must not submit the Oracle draft',
    );
    await discard.sendKeys(Key.ENTER);

    await pollUntil(
      async () => {
        try {
          return (await driver.getAllWindowHandles()).length === 0;
        } catch {
          return true;
        }
      },
      { timeoutMs: 10000, intervalMs: 100 },
    );
  });

  it.skip('active-save native close requires deterministic deferred real-backend write control; covered by component and Shell tests', () => {
    // The current native harness has no supported way to hold an update_session
    // command after the frontend starts it. Adding a production-only test hook
    // would weaken this test, so the deterministic active-write transition stays
    // in CloseDraftsDialog.test.ts and Shell.test.ts until the native harness has
    // an external controllable backend adapter.
  });
});
