import { test, expect } from '@playwright/test';
import { installIpcMock } from './ipc-mock';

test.describe('Chronacle Backend IPC', () => {
  test.beforeEach(async ({ page }) => {
    await installIpcMock(page);
  });

  test('loads the app without crashing', async ({ page }) => {
    await page.goto('http://localhost:1420');
    // App should render with the Oracle topbar title
    await expect(page.locator('header .title')).toHaveText('Oracle');
  });

  test('shows the settings page when clicking Settings', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await page.click('button[aria-label="Settings"]');
    await expect(page.locator('h2')).toHaveText('Settings');
  });

  test('shows the chat interface by default', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('textarea')).toBeVisible();
    await expect(page.locator('button[aria-label="Send"]')).toBeVisible();
  });

  test('upload PDF button is visible', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('button:has-text("Upload PDF")')).toBeVisible();
  });

  test('shows empty-library state when no sources indexed', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('text=The oracle has no tomes to consult yet.')).toBeVisible();
  });
});
