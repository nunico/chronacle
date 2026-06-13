// UI-driven check of the "Enrich related entities" toggle in SettingsView.
//
// Lighter than enrichment-flow.e2e (no LLM/PDF/embeddings): it clicks the real
// checkbox in the rendered Settings page and confirms the choice round-trips
// through IPC → SurrealDB and reloads back into the UI. Exercises the
// SettingsView wiring added alongside the enrichment feature.
import assert from 'node:assert/strict';
import { By, until } from 'selenium-webdriver';
import { startTauriDriver, buildDriver, invoke, pollUntil } from './driver.mjs';

const SETTINGS_BTN = By.xpath('//button[contains(normalize-space(.), "Settings")]');
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
    await pollUntil(async () => {
      try {
        await invoke(driver, 'get_settings');
        return true;
      } catch {
        return false;
      }
    }, { timeoutMs: 30000, intervalMs: 1000 });
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
    // Start from a known-off state so the click definitely turns it on.
    await invoke(driver, 'update_setting', {
      key: 'extraction_enrich_neighbors',
      value: 'false',
    });

    let checkbox = await openSettings();
    assert.equal(await checkbox.isSelected(), false, 'should load unchecked');

    await checkbox.click();
    // The onchange handler saves immediately; confirm via the backend.
    await pollUntil(async () => {
      const s = await invoke(driver, 'get_settings');
      return s.extraction_enrich_neighbors === 'true';
    }, { timeoutMs: 8000, intervalMs: 500 });

    // Reload the webview; the persisted value must hydrate the checkbox.
    await driver.navigate().refresh();
    checkbox = await openSettings();
    assert.equal(
      await checkbox.isSelected(),
      true,
      'checkbox should reflect the persisted setting after reload',
    );
  });
});
