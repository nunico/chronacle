import { expect, type Page } from '@playwright/test';
import { Given, When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

interface IpcCall {
  cmd: string;
  args?: Record<string, unknown>;
}

function entity(overrides: Record<string, unknown>) {
  return {
    id: 'npc1',
    kind: 'npc',
    campaign_id: 'camp1',
    name: 'Mira',
    aliases: [],
    summary: null,
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
    codex_article: null,
    codex_stale: null,
    codex_compiled_at: null,
    ...overrides,
  };
}

function bareLinkText(linkText: string): string {
  return linkText.replace(/^\[\[/, '').replace(/\]\]$/, '');
}

async function getIpcCalls(page: Page): Promise<IpcCall[]> {
  return page.evaluate(() => (window as unknown as { __ipcCalls: IpcCall[] }).__ipcCalls);
}

async function installArticleScenario(page: Page, linkText: string) {
  const linkName = bareLinkText(linkText);
  const npc = entity({
    id: 'npc1',
    kind: 'npc',
    name: 'Mira',
    codex_article: `Mira knows the [[${linkName}]].`,
    codex_stale: false,
  });
  const createdLocation = entity({
    id: 'loc1',
    kind: 'location',
    name: linkName,
    codex_article: null,
    codex_stale: null,
  });

  await installIpcMock(page, {
    get_entities: [npc],
    get_entity_relations: [],
    get_entity_graph: {
      nodes: [
        { id: 'npc1', kind: 'npc', name: 'Mira' },
        {
          id: 'missing_wikilink:npc:npc1:moon gate',
          kind: 'missing_wikilink',
          name: linkName,
          missing: true,
          source_id: 'npc1',
          source_kind: 'npc',
        },
      ],
      edges: [
        {
          from_id: 'npc1',
          from_kind: 'npc',
          to_id: 'missing_wikilink:npc:npc1:moon gate',
          to_kind: 'missing_wikilink',
          rel_type: 'unresolved',
          notes: null,
        },
      ],
    },
    create_entity: createdLocation,
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'NPCs' }).click();
  await page
    .locator('.entity-row', { hasText: 'Mira' })
    .getByRole('button', { name: 'Mira', exact: true })
    .click();
}

Given(
  'an NPC article contains the unresolved link {string}',
  async ({ page }, linkText: string) => {
    await installArticleScenario(page, linkText);
  },
);

Given(
  'Maintenance has a wikilink finding for {string} with a suggestion {string}',
  async ({ page }, linkText: string, candidateName: string) => {
    const linkName = bareLinkText(linkText);
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint-suggest',
          kind: 'broken_wikilink',
          payload: {
            entity: 'npc:mira',
            link_text: linkName,
            candidates: [{ id: 'location:moon-gate', name: candidateName, similarity: 0.91 }],
          },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      confirm_alias_suggestion: null,
      resolve_lint_finding: null,
      create_entity: entity({ id: 'loc2', kind: 'location', name: linkName }),
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

Given(
  'Maintenance has a wikilink finding for {string} with no candidates',
  async ({ page }, linkText: string) => {
    const linkName = bareLinkText(linkText);
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint-missing',
          kind: 'broken_wikilink',
          payload: { entity: 'npc:mira', link_text: linkName, candidates: [] },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      resolve_lint_finding: null,
      create_entity: entity({ id: 'loc3', kind: 'location', name: linkName }),
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

When('the GM clicks the unresolved link {string}', async ({ page }, linkText: string) => {
  await page.getByRole('button', { name: `Create article for ${bareLinkText(linkText)}` }).click();
});

When('creates a Location named {string}', async ({ page }, name: string) => {
  await page.locator('[data-testid="create-entity-chooser"] [data-entity-kind="location"]').click();
  await expect(page.getByTestId('entity-form-name')).toHaveValue(name);
  await page.getByTestId('entity-form-submit').click();
});

When('the GM opens the finding', async ({ page }) => {
  await page.getByRole('tab', { name: /Findings/ }).click();
});

When("the GM opens that NPC's relationship graph", async ({ page }) => {
  await page.getByRole('button', { name: 'Graph' }).click();
});

When('the GM clicks the missing-link node', async ({ page }) => {
  await page.locator('[data-missing="true"]').click();
});

Then('the create command is sent for a Location named {string}', async ({ page }, name: string) => {
  const calls = await getIpcCalls(page);
  const call = calls.find(
    (c) =>
      c.cmd === 'create_entity' &&
      c.args?.campaignId === 'camp1' &&
      c.args?.kind === 'location' &&
      (c.args?.input as { name?: string } | undefined)?.name === name,
  );
  expect(call).toBeDefined();
});

Then('they can use the suggestion', async ({ page }) => {
  const group = page.locator('.finding-group', { hasText: 'Wikilinks' });
  await expect(group).toContainText('Possible name mismatch');
  await expect(group).toContainText('Suggested match: Moon Gate');
  await expect(group.getByRole('button', { name: 'Use suggestion' })).toBeVisible();
});

Then('they can instead create a new article named {string}', async ({ page }, name: string) => {
  await page.getByRole('button', { name: 'Create article' }).click();
  await page.locator('[data-testid="create-entity-chooser"] [data-entity-kind="location"]').click();
  await expect(page.getByTestId('entity-form-name')).toHaveValue(name);
});

Then('the finding is labeled {string}', async ({ page }, label: string) => {
  const group = page.locator('.finding-group', { hasText: 'Wikilinks' });
  await expect(group).toContainText(label);
});

Then('the primary action is {string}', async ({ page }, label: string) => {
  const group = page.locator('.finding-group', { hasText: 'Wikilinks' });
  await expect(group.getByRole('button', { name: label })).toBeVisible();
  await expect(group.getByRole('button', { name: 'Use suggestion' })).toHaveCount(0);
});

Then(
  'the graph shows a distinct missing-link node named {string}',
  async ({ page }, linkText: string) => {
    await expect(page.locator('[data-missing="true"]', { hasText: linkText })).toBeVisible();
  },
);
