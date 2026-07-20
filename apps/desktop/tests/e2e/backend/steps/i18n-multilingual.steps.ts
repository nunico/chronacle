import { expect, type Page } from '@playwright/test';
import { Given, When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

interface IpcCall {
  cmd: string;
  args?: Record<string, unknown>;
}

interface LanguageScenarioState {
  osLocale: string;
  uiLocale: string;
  indexedEmbeddingModel: string | null;
}

const scenarioState = new WeakMap<Page, LanguageScenarioState>();

function stateFor(page: Page): LanguageScenarioState {
  const existing = scenarioState.get(page);
  if (existing) return existing;

  const state = { osLocale: 'en-US', uiLocale: 'auto', indexedEmbeddingModel: null };
  scenarioState.set(page, state);
  return state;
}

async function installLanguageScenarioMock(page: Page): Promise<void> {
  const state = stateFor(page);
  await installIpcMock(
    page,
    {
      'plugin:os|locale': state.osLocale,
      get_settings: {
        llm_provider: 'openai',
        llm_model: 'gpt-4o-mini',
        llm_api_key: 'sk-test',
        llm_base_url: '',
        active_campaign_id: '',
        ui_locale: state.uiLocale,
        embedding_mode: 'local_nomic',
      },
      get_embedding_provider_status: {
        backend: 'local',
        mode: 'local_nomic',
        model: 'nomic-embed-text-v1.5',
        dimension: 768,
        api_key_configured: false,
        local_available: true,
        local_cached: true,
      },
      get_embedding_model_mismatch: {
        active_model: 'nomic-embed-text-v1.5',
        stale: [],
      },
      reconfigure_embedding_provider: 'multilingual-e5-base',
    },
    undefined,
    {
      get_settings: {
        llm_provider: 'openai',
        llm_model: 'gpt-4o-mini',
        llm_api_key: 'sk-test',
        llm_base_url: '',
        active_campaign_id: '',
        ui_locale: state.uiLocale,
        embedding_mode: 'local_multilingual',
      },
      get_embedding_provider_status: {
        backend: 'local',
        mode: 'local_multilingual',
        model: 'multilingual-e5-base',
        dimension: 768,
        api_key_configured: false,
        local_available: true,
        local_cached: true,
      },
      get_embedding_model_mismatch: {
        active_model: 'multilingual-e5-base',
        stale: state.indexedEmbeddingModel
          ? [{ embed_model: state.indexedEmbeddingModel, source_count: 1 }]
          : [],
      },
    },
  );
}

async function getIpcCalls(page: Page): Promise<IpcCall[]> {
  return page.evaluate(() => (window as unknown as { __ipcCalls: IpcCall[] }).__ipcCalls);
}

Given('the operating system locale is {string}', async ({ page }, osLocale: string) => {
  stateFor(page).osLocale = osLocale;
  await installLanguageScenarioMock(page);
});

Given('the saved interface language is {string}', async ({ page }, uiLocale: string) => {
  stateFor(page).uiLocale = uiLocale;
  await installLanguageScenarioMock(page);
});

When('Chronacle opens Settings', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /Settings|Einstellungen/ }).click();
});

Then('the Settings heading is {string}', async ({ page }, heading: string) => {
  await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible();
});

When('I ask Oracle {string}', async ({ page }, question: string) => {
  await page.goto('/');
  await page.locator('textarea').fill(question);
  await page.getByRole('button', { name: /Send|Senden/ }).click();
});

Then('the Oracle request response language is {string}', async ({ page }, language: string) => {
  await expect
    .poll(async () => {
      const calls = await getIpcCalls(page);
      return calls.find((call) => call.cmd === 'chat_send')?.args?.request;
    })
    .toMatchObject({ responseLanguage: language });
});

Given('sources were indexed with {string}', async ({ page }, embedModel: string) => {
  stateFor(page).indexedEmbeddingModel = embedModel;
  await installLanguageScenarioMock(page);
});

When('I select the local multilingual embedding mode', async ({ page }) => {
  await page.locator('#embed-mode').selectOption('local_multilingual');
  await page.getByRole('button', { name: /Save embedding provider/ }).click();
});

When('I open embedding settings', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /Settings/ }).click();
});

Then('the Nomic sources do not require re-indexing', async ({ page }) => {
  await expect(page.getByTestId('mismatch-banner')).toHaveCount(0);
});

Then('Chronacle shows that source embeddings require re-indexing', async ({ page }) => {
  const calls = await getIpcCalls(page);
  expect(
    calls.some(
      (call) =>
        call.cmd === 'update_setting' &&
        call.args?.key === 'embedding_mode' &&
        call.args?.value === 'local_multilingual',
    ),
  ).toBe(true);
  await expect(page.getByTestId('mismatch-banner')).toBeVisible();
  await expect(page.getByTestId('mismatch-reindex')).toBeVisible();
});
