// Native close-request coverage for retained frontend drafts.
//
// This must run through tauri-driver: the mocked browser acceptance suite cannot
// emit Tauri's WINDOW_CLOSE_REQUESTED event or prove that destroy closes the
// native window.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { By, Key, until } from 'selenium-webdriver';
import { APP_URL, createNativeTestSession, invoke, navigateToApp, pollUntil } from './driver.mjs';

const execFileAsync = promisify(execFile);

const ORACLE_BUTTON = By.xpath('//button[normalize-space(.)="Oracle"]');
const ORACLE_COMPOSER = By.xpath('//textarea[contains(@placeholder, "Ask a rule")]');
const CLOSE_DIALOG = By.xpath('//*[@role="dialog" and .//*[normalize-space(.)="Unsaved changes"]]');
const CANCEL = By.xpath('//button[normalize-space(.)="Cancel"]');
const DISCARD_AND_CLOSE = By.xpath('//button[normalize-space(.)="Discard and close"]');
const SESSIONS_BUTTON = By.xpath('//button[contains(normalize-space(.), "Sessions")]');
const SESSION_TITLE = By.xpath('//div[contains(@class, "session-body")]//input[@type="text"]');
const WAIT_FOR_SAVE = By.xpath(
  '//*[normalize-space(.)="Wait for saving to finish before discarding and closing."]',
);

async function requestNativeClose() {
  // Send the window manager's WM_DELETE_WINDOW request. Selenium's W3C close
  // command invalidates the tauri-driver session even when Tauri prevents the
  // close, while xdotool leaves the session attached for dialog assertions.
  const { stdout } = await execFileAsync('xdotool', [
    'search',
    '--onlyvisible',
    '--name',
    '^Chronacle$',
  ]);
  const windows = stdout.trim().split(/\s+/).filter(Boolean);
  if (windows.length !== 1) {
    throw new Error(`Expected one visible Chronacle window, found ${windows.length}`);
  }
  await execFileAsync('xdotool', ['windowactivate', '--sync', windows[0]]);
  await execFileAsync('xdotool', ['key', '--clearmodifiers', 'alt+F4']);
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

async function beginHeldSessionUpdate(driver, nextField) {
  return driver.executeScript(
    `const [nextField] = arguments;
     const callbacks = window.__TAURI_INTERNALS__.callbacks;
     const before = new Set(callbacks.keys());

     // Focus dispatches the title's real blur handler synchronously. Tauri
     // registers the command's success/error callbacks before yielding to Rust,
     // so this script can defer the acknowledgment without replacing hardened
     // IPC internals or skipping the real update_session command.
     nextField.focus();
     const callbackIds = [...callbacks.keys()].filter((id) => !before.has(id));
     if (callbackIds.length !== 2) {
       throw new Error(
         'Expected update_session to register two IPC callbacks, found ' + callbackIds.length,
       );
     }

     const held = [];
     for (const id of callbackIds) {
       const callback = callbacks.get(id);
       callbacks.set(id, (...args) => held.push(() => callback(...args)));
     }
     window.__CHRONACLE_NATIVE_E2E__ = {
       held,
       releaseNext() {
         const acknowledgment = held.shift();
         if (!acknowledgment) throw new Error('No held update_session acknowledgment');
         acknowledgment();
       },
     };
     return callbackIds.length;`,
    nextField,
  );
}

async function heldSessionAcknowledgmentCount(driver) {
  return driver.executeScript('return window.__CHRONACLE_NATIVE_E2E__?.held.length ?? 0;');
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

    await requestNativeClose();
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

    await requestNativeClose();
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
    await driver.executeScript(
      "localStorage.setItem('chronacle_active_campaign_id', arguments[0]);",
      campaign.id,
    );
    await driver.get(`${APP_URL}?native-session=${encodeURIComponent(campaign.id)}`);

    await driver.wait(until.elementLocated(SESSIONS_BUTTON), 10000);
    await driver.findElement(SESSIONS_BUTTON).click();
    const sessionHeader = await driver.wait(
      until.elementLocated(By.xpath('//button[contains(@class, "session-header")]')),
      10000,
    );
    await sessionHeader.click();
    const title = await driver.wait(until.elementLocated(SESSION_TITLE), 10000);

    await title.clear();
    await title.sendKeys('The saving title');
    const date = await driver.findElement(
      By.xpath('//div[contains(@class, "session-body")]//input[@type="date"]'),
    );
    assert.equal(await beginHeldSessionUpdate(driver, date), 2);
    await pollUntil(async () => (await heldSessionAcknowledgmentCount(driver)) === 1, {
      timeoutMs: 10000,
      intervalMs: 100,
    });
    const persistedDuringHeldAcknowledgment = await invoke(driver, 'get_session', {
      id: savedSession.id,
    });
    assert.equal(persistedDuringHeldAcknowledgment.id, savedSession.id);
    assert.equal(persistedDuringHeldAcknowledgment.campaign_id, campaign.id);
    assert.equal(persistedDuringHeldAcknowledgment.title, 'The saving title');
    assert.notEqual(
      persistedDuringHeldAcknowledgment.updated_at,
      savedSession.updated_at,
      'the first update_session write must reach backend persistence before its acknowledgment',
    );

    await title.click();
    await title.clear();
    await title.sendKeys('The newer unsaved title');
    await requestNativeClose();

    const dialog = await driver.wait(until.elementLocated(CLOSE_DIALOG), 10000);
    assert.equal(await dialog.isDisplayed(), true, 'active save must prevent native close');
    const discardWhileSaving = await driver.findElement(DISCARD_AND_CLOSE);
    assert.equal(await discardWhileSaving.isEnabled(), false);
    assert.equal(await driver.findElement(WAIT_FOR_SAVE).isDisplayed(), true);
    assert.equal(await title.getAttribute('value'), 'The newer unsaved title');
    assert.equal(
      await heldSessionAcknowledgmentCount(driver),
      1,
      'close must leave the original save acknowledgment held',
    );

    await releaseHeldSessionUpdate(driver);
    await driver.wait(async () => {
      try {
        return await driver.findElement(DISCARD_AND_CLOSE).isEnabled();
      } catch {
        return false;
      }
    }, 10000);
    assert.equal(
      await driver.findElement(SESSION_TITLE).getAttribute('value'),
      'The newer unsaved title',
      'the older acknowledgment must not overwrite the newer edit',
    );
    assert.equal(
      await driver.findElement(By.xpath('//*[normalize-space(.)="Unsaved changes"]')).isDisplayed(),
      true,
      'the newer revision must remain visibly unsaved',
    );
    const persisted = await invoke(driver, 'get_session', { id: savedSession.id });
    assert.equal(persisted.id, savedSession.id);
    assert.equal(persisted.campaign_id, campaign.id);
    assert.equal(persisted.title, 'The saving title');
    assert.equal(
      persisted.updated_at,
      persistedDuringHeldAcknowledgment.updated_at,
      'native close must not persist an additional session update',
    );

    await activateDiscardAndObserveDestroy(driver, await driver.findElement(DISCARD_AND_CLOSE));
  });
});
