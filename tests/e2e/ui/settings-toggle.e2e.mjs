// UI-driven check of the "Enrich related entities" toggle in SettingsView.
//
// Lighter than enrichment-flow.e2e (no LLM/PDF/embeddings): it clicks the real
// checkbox in the rendered Settings page and confirms the choice round-trips
// through IPC → SurrealDB and reloads back into the UI. Exercises the
// SettingsView wiring added alongside the enrichment feature.
import assert from 'node:assert/strict';
import { By, until } from 'selenium-webdriver';
import {
  startTauriDriver,
  buildDriver,
  invoke,
  pollUntil,
  navigateToApp,
  waitForWebviewReady,
} from './driver.mjs';

// Icon-only button — identified by aria-label, not visible text.
const SETTINGS_BTN = By.xpath('//button[@aria-label="Settings"]');
const ENRICH_CHECKBOX = By.xpath(
  '//label[contains(normalize-space(.), "Enrich related entities")]//input[@type="checkbox"]',
);

describe('SettingsView — enrich neighbors toggle', function () {
  this.timeout(60000);

  let tauriDriver;
  let driver;

  before(async () => {
    tauriDriver = startTauriDriver();
    driver = await buildDriver();
    await waitForWebviewReady(driver);
  });

  after(async () => {
    if (driver) await driver.quit();
    if (tauriDriver) tauriDriver.kill();
  });

  async function openSettings() {
    await driver.wait(until.elementLocated(SETTINGS_BTN), 10000);
    await driver.findElement(SETTINGS_BTN).click();
    await driver.wait(until.elementLocated(ENRICH_CHECKBOX), 10000);
    return driver.findElement(ENRICH_CHECKBOX);
  }

  it('persists the toggle through IPC and a reload', async () => {
    // Bypass the first-run model-download gate (App.svelte renders ModelDownload
    // instead of the Shell until the embedding model is ready). A non-local
    // embedding backend sends the app straight into the Shell, so the Settings
    // UI renders without a 250 MB model download. Setting-scoped to this spec —
    // the enrichment spec needs the local mock embedder.
    await invoke(driver, 'update_setting', { key: 'embedding_backend', value: 'openai' });
    // Start from a known-off state so the click definitely turns it on.
    await invoke(driver, 'update_setting', {
      key: 'extraction_enrich_neighbors',
      value: 'false',
    });

    // Reload so the gate re-evaluates and the Shell (with Settings) renders.
    await navigateToApp(driver);
    let checkbox = await openSettings();
    assert.equal(await checkbox.isSelected(), false, 'should load unchecked');

    await checkbox.click();
    // The onchange handler saves immediately; confirm via the backend.
    await pollUntil(async () => {
      const s = await invoke(driver, 'get_settings');
      return s.extraction_enrich_neighbors === 'true';
    }, { timeoutMs: 8000, intervalMs: 500 });

    // Reload the webview; the persisted value must hydrate the checkbox.
    // (navigateToApp, not refresh — a plain refresh re-triggers the webview's
    // about:blank reset.)
    await navigateToApp(driver);
    checkbox = await openSettings();
    assert.equal(
      await checkbox.isSelected(),
      true,
      'checkbox should reflect the persisted setting after reload',
    );
  });
});
