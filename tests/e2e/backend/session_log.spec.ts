import { test, expect } from '@playwright/test';

const MOCK_CAMPAIGN = { id: 'camp1', name: 'Test Campaign', system: 'D&D 5e' };

const MOCK_SESSION = {
  id: 'sess1',
  campaign_id: 'camp1',
  session_number: 1,
  title: 'The First Session',
  date_played: '2026-06-05',
  notes: '[[Torvin]] was seen at the tavern.',
  created_at: '2026-06-05T00:00:00Z',
  updated_at: '2026-06-05T00:00:00Z',
};

const MOCK_NPC = {
  id: 'npc1',
  kind: 'npc',
  campaign_id: 'camp1',
  name: 'Torvin',
  summary: 'A mysterious stranger',
  notes: null,
  created_at: null,
  updated_at: null,
  date_start: null,
  date_end: null,
  is_ongoing: null,
  sequence_index: null,
  era: null,
  duration_label: null,
  session_id: null,
  player_name: null,
  character_class: null,
  character_level: null,
  status: null,
};

test.describe('Session Log', () => {
  test.beforeEach(async ({ page }) => {
    // Seed localStorage so Shell picks up the active campaign without needing
    // settings to have active_campaign_id (Shell uses localStorage, not settings).
    await page.addInitScript(() => {
      localStorage.setItem('chronacle_active_campaign_id', 'camp1');
    });

    await page.addInitScript(() => {
      // @ts-expect-error -- __TAURI_INTERNALS__ is injected by Tauri at runtime
      window.__TAURI_INTERNALS__ = {
        invoke: (cmd: string, args?: Record<string, unknown>) => {
          switch (cmd) {
            // ── Embedding model — return true so ModelDownload calls onModelReady
            case 'check_embedding_model':
              return Promise.resolve(true);

            case 'download_embedding_model':
              return Promise.resolve(null);

            case 'get_embedding_model_mismatch':
              return Promise.resolve({
                active_model: 'nomic-embed-text-v1.5',
                stale: [],
              });

            // ── Settings
            case 'get_settings':
              return Promise.resolve({
                llm_provider: 'openai',
                llm_model: 'gpt-4o-mini',
                llm_api_key: 'sk-test',
                llm_base_url: '',
                active_campaign_id: 'camp1',
              });

            case 'update_setting':
              return Promise.resolve(null);

            // ── Campaigns
            case 'get_campaigns':
              return Promise.resolve([MOCK_CAMPAIGN]);

            case 'get_campaign':
              return Promise.resolve(MOCK_CAMPAIGN);

            // ── Sessions
            case 'get_sessions':
              return Promise.resolve([MOCK_SESSION]);

            case 'create_session':
              return Promise.resolve({
                id: 'sess2',
                campaign_id: 'camp1',
                session_number: 2,
                title: 'Session 2',
                date_played: new Date().toISOString().slice(0, 10),
                notes: '',
                created_at: new Date().toISOString(),
                updated_at: new Date().toISOString(),
              });

            case 'update_session':
              return Promise.resolve(MOCK_SESSION);

            case 'delete_session':
              return Promise.resolve(null);

            case 'get_session_entities':
              return Promise.resolve([MOCK_NPC]);

            // ── Entities — return MOCK_NPC for npc kind, empty for others
            case 'get_entities': {
              const kind = (args as Record<string, unknown>)?.kind;
              if (kind === 'npc') return Promise.resolve([MOCK_NPC]);
              return Promise.resolve([]);
            }

            // ── LLM / chat
            case 'get_llm_provider_status':
              return Promise.resolve({
                provider_type: 'openai',
                model: 'gpt-4o-mini',
                api_key_configured: true,
              });

            case 'get_chat_history':
              return Promise.resolve([]);

            case 'chat_send':
              return Promise.resolve(null);

            case 'reconfigure_llm_provider':
              return Promise.resolve('openai');

            // ── Collections / sources
            case 'get_collections':
              return Promise.resolve([]);

            case 'get_sources':
              return Promise.resolve([]);

            case 'get_campaign_collections':
              return Promise.resolve([]);

            // ── Custom providers / models
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

  test('shows Sessions tab in navigation', async ({ page }) => {
    await page.goto('http://localhost:1420');
    // Wait for the shell to render (ModelDownload resolves synchronously via mock)
    await expect(page.locator('.rail-scroll')).toBeVisible();
    // Sessions button is rendered by CampaignRail inside rail-scroll
    await expect(page.locator('.rail-scroll button:has-text("Sessions")')).toBeVisible();
  });

  test('displays session list when campaign is active', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('.rail-scroll')).toBeVisible();

    // Click the Sessions nav item
    await page.click('.rail-scroll button:has-text("Sessions")');

    // Session title should be visible in the session list
    await expect(page.locator('text=The First Session')).toBeVisible();
  });

  test('wikilinked entity badge shown in WikiText preview', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('.rail-scroll')).toBeVisible();

    // Navigate to Sessions view
    await page.click('.rail-scroll button:has-text("Sessions")');
    await expect(page.locator('text=The First Session')).toBeVisible();

    // Expand the session row by clicking the session header
    await page.click('button.session-header');

    // The WikiText preview renders [[Torvin]] as an .entity-badge
    // (Torvin is in entityMap because get_entities for 'npc' returns MOCK_NPC)
    await expect(page.locator('.wiki-preview .entity-badge')).toBeVisible();
    await expect(page.locator('.wiki-preview .entity-badge')).toHaveText('Torvin');
  });

  test('shows + New Session button', async ({ page }) => {
    await page.goto('http://localhost:1420');
    await expect(page.locator('.rail-scroll')).toBeVisible();

    // Navigate to Sessions view
    await page.click('.rail-scroll button:has-text("Sessions")');

    // The + New Session button should be visible in the session log header
    await expect(page.locator('button:has-text("+ New Session")')).toBeVisible();
  });
});
