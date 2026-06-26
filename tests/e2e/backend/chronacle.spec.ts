import { test, expect } from '@playwright/test';

test.describe('Chronacle Backend IPC', () => {
  test.beforeEach(async ({ page }) => {
    // Set up IPC mocks before each test.
    // We mock window.__TAURI_INTERNALS__.invoke directly instead of
    // importing mockIPC from @tauri-apps/api/mocks, because
    // addInitScript runs in the browser context where the module
    // isn't available.
    await page.addInitScript(() => {
      let _cbId = 0;
      // @ts-expect-error -- __TAURI_INTERNALS__ is injected by Tauri at runtime
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      // @ts-expect-error -- __TAURI_INTERNALS__ is injected by Tauri at runtime
      window.__TAURI_INTERNALS__ = {
        // listen() calls transformCallback() to register a JS callback and get
        // a numeric handle to pass to the backend. Return a simple counter; we
        // don't need events to actually fire in these tests.
        transformCallback: (_cb: unknown, _once?: boolean) => ++_cbId,
        invoke: (cmd: string, _args?: Record<string, unknown>) => {
          switch (cmd) {
            // Tauri event plugin — listen/unlisten go through invoke internally
            case 'plugin:event|listen':
              return Promise.resolve(0);
            case 'plugin:event|unlisten':
              return Promise.resolve(null);
            // OS plugin — locale is requested at startup
            case 'plugin:os|locale':
              return Promise.resolve('en-US');
            // ── Embedding model — non-local backend bypasses ModelDownload
            case 'get_embedding_provider_status':
              return Promise.resolve({
                backend: 'openai',
                model: 'text-embedding-3-small',
                dimension: 1536,
                api_key_configured: true,
                local_available: false,
                local_cached: false,
              });

            case 'get_embedding_model_mismatch':
              return Promise.resolve({ active_model: 'nomic-embed-text-v1.5', stale: [] });

            // ── Campaigns / rail counts
            case 'get_campaigns':
              return Promise.resolve([{ id: 'camp1', name: 'Test Campaign', system: 'D&D 5e' }]);

            case 'get_entity_counts':
              return Promise.resolve({});

            case 'get_sessions':
              return Promise.resolve([]);

            case 'get_collections':
              return Promise.resolve([]);

            case 'get_sources':
              return Promise.resolve([]);

            // ── Settings
            case 'get_settings':
              return Promise.resolve({
                llm_provider: 'openai',
                llm_model: 'gpt-4o-mini',
                llm_api_key: 'sk-test',
                llm_base_url: '',
                active_campaign_id: '',
              });

            case 'update_setting':
              return Promise.resolve(null);

            case 'get_llm_provider_status':
              return Promise.resolve({
                provider_type: 'openai',
                model: 'gpt-4o-mini',
                api_key_configured: true,
              });

            // ── Chat
            case 'get_chat_history':
              return Promise.resolve([]);

            case 'chat_send':
              return Promise.resolve(null);

            // ── Custom providers
            case 'get_custom_providers':
              return Promise.resolve([]);

            case 'get_provider_models':
              return Promise.resolve([]);

            default:
              console.warn(`Unhandled IPC mock: ${cmd}`);
              return Promise.resolve(null);
          }
        },
      };
    });
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
