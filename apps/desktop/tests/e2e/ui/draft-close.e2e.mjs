// Native close-request coverage for retained frontend drafts.
//
// This must run through tauri-driver: the mocked browser acceptance suite cannot
// emit Tauri's WINDOW_CLOSE_REQUESTED event or prove that destroy closes the
// native window.
import assert from 'node:assert/strict';
import { By, Key, until } from 'selenium-webdriver';
import { createNativeTestSession, invoke, navigateToApp, pollUntil } from './driver.mjs';

const ORACLE_BUTTON = By.xpath('//button[normalize-space(.)="Oracle"]');
const ORACLE_COMPOSER = By.xpath('//textarea[contains(@placeholder, "Ask a rule")]');
const CLOSE_DIALOG = By.xpath('//*[@role="dialog" and .//*[normalize-space(.)="Unsaved changes"]]');
const CANCEL = By.xpath('//button[normalize-space(.)="Cancel"]');
const DISCARD_AND_CLOSE = By.xpath('//button[normalize-space(.)="Discard and close"]');
const SESSIONS_BUTTON = By.xpath('//button[normalize-space(.)="Sessions"]');
const WAIT_FOR_SAVE = By.xpath(
  '//*[normalize-space(.)="Wait for saving to finish before discarding and closing."]',
);

async function requestNativeClose(driver) {
  // The W3C close command crosses tauri-driver into the native window. The
  // application must prevent it while a draft decision is outstanding.
  await driver.close();
}

async function nativeWindowIsGone(driver) {
  const sessionId = await driver.getSession().then((session) => session.getId());
  try {
    // Query the W3C endpoint directly so a destroyed WebView cannot turn the
    // observation itself into Selenium's otherwise information-free
    // WebDriverError.
    const response = await fetch(
      `http://127.0.0.1:4444/session/${encodeURIComponent(sessionId)}/window/handles`,
    );
    const payload = await response.json();
    if (response.ok) return Array.isArray(payload.value) && payload.value.length === 0;
    return (
      payload.value?.error === 'no such window' || payload.value?.error === 'invalid session id'
    );
  } catch {
    // A transport failure alone does not prove that the requested window was
    // destroyed. Keep polling until the protocol supplies an affirmative state.
    return false;
  }
}

async function activateDiscardAndObserveDestroy(driver, discard) {
  let activationError;
  try {
    await discard.sendKeys(Key.ENTER);
  } catch (error) {
    activationError = error;
  }

  try {
    await pollUntil(() => nativeWindowIsGone(driver), { timeoutMs: 10000, intervalMs: 100 });
  } catch (observationError) {
    // Preserve the interaction failure when destruction was not independently
    // observed; arbitrary WebDriver errors must never become passing evidence.
    if (activationError) throw activationError;
    throw observationError;
  }
}

async function holdSessionUpdates(driver) {
  await driver.executeScript(`
    const internals = window.__TAURI_INTERNALS__;
    const originalInvoke = internals.invoke.bind(internals);
    const held = [];
    window.__CHRONACLE_NATIVE_E2E__ = {
      updateCalls: [],
      held,
      releaseNext() {
        const pending = held.shift();
        if (!pending) throw new Error('No held update_session request');
        originalInvoke('update_session', pending.args, pending.options)
          .then(pending.resolve, pending.reject);
      },
    };
    internals.invoke = (command, args, options) => {
      if (command !== 'update_session') return originalInvoke(command, args, options);
      window.__CHRONACLE_NATIVE_E2E__.updateCalls.push(structuredClone(args));
      return new Promise((resolve, reject) => held.push({ args, options, resolve, reject }));
    };
  `);
}

async function heldSessionUpdateCount(driver) {
  return driver.executeScript('return window.__CHRONACLE_NATIVE_E2E__?.updateCalls.length ?? 0;');
}

async function releaseHeldSessionUpdate(driver) {
  await driver.executeScript('window.__CHRONACLE_NATIVE_E2E__.releaseNext();');
}

describe('Draft retention — native window close', function () {
  this.timeout(180000);

  let nativeSession;
  let driver;

  beforeEach(async () => {
    nativeSession = await createNativeTestSession();
    driver = nativeSession.driver;
    // Bypass the first-run local-model download gate. Oracle draft retention is
    // frontend-only and does not contact the configured embedding provider.
    await invoke(driver, 'update_setting', { key: 'embedding_backend', value: 'openai' });
    await navigateToApp(driver);
  });

  afterEach(async () => {
    await nativeSession?.close();
    nativeSession = undefined;
    driver = undefined;
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
    await activateDiscardAndObserveDestroy(driver, discard);
  });

  it('retains a newer session edit while an earlier native save settles', async () => {
    const campaign = await invoke(driver, 'create_campaign', {
      name: 'Native Close Campaign',
      system: '5e',
    });
    const savedSession = await invoke(driver, 'create_session', {
      campaignId: campaign.id,
      input: {
        sessionNumber: 1,
        title: 'The saved title',
        datePlayed: '2026-09-10',
        notes: '',
      },
    });
    await navigateToApp(driver);

    await driver.wait(until.elementLocated(SESSIONS_BUTTON), 10000);
    await driver.findElement(SESSIONS_BUTTON).click();
    const sessionHeader = await driver.wait(
      until.elementLocated(By.xpath('//button[contains(@class, "session-header")]')),
      10000,
    );
    await sessionHeader.click();
    const title = await driver.wait(
      until.elementLocated(
        By.xpath('//div[contains(@class, "session-body")]//input[@type="text"]'),
      ),
      10000,
    );

    await holdSessionUpdates(driver);
    await title.clear();
    await title.sendKeys('The saving title');
    const date = await driver.findElement(
      By.xpath('//div[contains(@class, "session-body")]//input[@type="date"]'),
    );
    await date.click();
    await pollUntil(async () => (await heldSessionUpdateCount(driver)) === 1, {
      timeoutMs: 10000,
      intervalMs: 100,
    });

    await title.click();
    await title.clear();
    await title.sendKeys('The newer unsaved title');
    await requestNativeClose(driver);

    const dialog = await driver.wait(until.elementLocated(CLOSE_DIALOG), 10000);
    assert.equal(await dialog.isDisplayed(), true, 'active save must prevent native close');
    const discardWhileSaving = await driver.findElement(DISCARD_AND_CLOSE);
    assert.equal(await discardWhileSaving.isEnabled(), false);
    assert.equal(await driver.findElement(WAIT_FOR_SAVE).isDisplayed(), true);
    assert.equal(await title.getAttribute('value'), 'The newer unsaved title');
    assert.equal(await heldSessionUpdateCount(driver), 1, 'close must not request another save');

    await releaseHeldSessionUpdate(driver);
    await driver.wait(async () => (await discardWhileSaving.isEnabled()) === true, 10000);
    assert.equal(
      await title.getAttribute('value'),
      'The newer unsaved title',
      'the older acknowledgment must not overwrite the newer edit',
    );
    assert.equal(
      await driver.findElement(By.xpath('//*[normalize-space(.)="Unsaved changes"]')).isDisplayed(),
      true,
      'the newer revision must remain visibly unsaved',
    );
    assert.equal(await heldSessionUpdateCount(driver), 1, 'settlement must not start another save');
    const persisted = await invoke(driver, 'get_session', { id: savedSession.id });
    assert.equal(persisted.title, 'The saving title');

    await activateDiscardAndObserveDestroy(driver, discardWhileSaving);
  });
});
