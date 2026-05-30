import { test, expect } from '@playwright/test';

test.describe('Chronacle Backend IPC', () => {
  test.beforeEach(async ({ page }) => {
    // Set up IPC mocks before each test.
    // We mock window.__TAURI_INTERNALS__.invoke directly instead of
    // importing mockIPC from @tauri-apps/api/mocks, because
    // addInitScript runs in the browser context where the module
    // isn't available.
    await page.addInitScript(() => {
      // @ts-ignore
      window.__TAURI_INTERNALS__ = {
        invoke: (cmd: string, args?: Record<string, unknown>) => {
          switch (cmd) {
            case 'get_settings':
              return {
                llm_provider: 'openai',
                llm_model: 'gpt-4o-mini',
                llm_api_key: 'sk-test',
                llm_base_url: '',
                active_campaign_id: '',
              };

            case 'update_setting':
              return null;

            case 'get_llm_provider_status':
              return {
                provider_type: 'openai',
                model: 'gpt-4o-mini',
                api_key_configured: true,
              };

            case 'get_chat_history':
              return [];

            case 'chat_send':
              return null;

            case 'get_custom_providers':
              return [];

            case 'get_provider_models':
              return [];

            default:
              console.warn(`Unhandled IPC mock: ${cmd}`);
              return null;
          }
        },
      };
    });
  });

  test('loads the app without crashing', async ({ page }) => {
    await page.goto('http://localhost:1420');
    // App should render with the Chronacle header
    await expect(page.locator('header h1')).toHaveText('Chronacle');
  });

  test('shows the settings page when clicking Settings', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await page.click('button:has-text("Settings")');
    await expect(page.locator('h2')).toHaveText('Settings');
  });

  test('shows the chat interface by default', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('textarea')).toBeVisible();
    await expect(page.locator('button:has-text("Send")')).toBeVisible();
  });

  test('upload PDF button is visible', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('button:has-text("Upload PDF")')).toBeVisible();
  });

  test('shows welcome message when no chat history', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('text=Welcome to Chronacle')).toBeVisible();
  });
});
