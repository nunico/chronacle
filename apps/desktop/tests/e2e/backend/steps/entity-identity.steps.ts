import { expect } from '@playwright/test';
import { Given, When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

interface IpcCall {
  cmd: string;
  args?: Record<string, unknown>;
}

async function getIpcCalls(page: import('@playwright/test').Page): Promise<IpcCall[]> {
  return page.evaluate(() => (window as unknown as { __ipcCalls: IpcCall[] }).__ipcCalls);
}

Given(
  'the maintenance inbox has a broken-wikilink finding for {string} that could mean {string}',
  async ({ page }, linkText: string, candidateName: string) => {
    const bareLinkText = linkText.replace(/^\[\[/, '').replace(/\]\]$/, '');
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint-suggest',
          kind: 'broken_wikilink',
          payload: {
            entity: 'npc:mira',
            link_text: bareLinkText,
            candidates: [{ id: 'faction:quassar', name: candidateName, similarity: 0.92 }],
          },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      confirm_alias_suggestion: null,
      resolve_lint_finding: null,
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

When('the GM confirms the suggestion {string}', async ({ page }, _candidateName: string) => {
  await page.getByRole('button', { name: /yes/i }).click();
});

Then('the confirm-alternate-name command is sent for that entity and alias', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const call = calls.find(
    (c) =>
      c.cmd === 'confirm_alias_suggestion' &&
      c.args?.entityId === 'faction:quassar' &&
      c.args?.alias === 'The Quassars',
  );
  expect(call).toBeDefined();
});

Given(
  'the maintenance inbox has a duplicate-entity finding for {string} and {string}',
  async ({ page }, _nameA: string, _nameB: string) => {
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint-dup',
          kind: 'duplicate_entity',
          payload: { a: 'faction:free1', b: 'faction:free2', similarity: 1.0 },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      // A single static fixture answers BOTH getEntity(idA) and getEntity(idB) —
      // the IPC mock cannot differentiate by argument. The dialog uses this
      // only to render names/summaries; the merge command itself is keyed on
      // the ids taken from the finding payload above, not off this fixture.
      get_entity: {
        id: 'free1',
        kind: 'faction',
        campaign_id: 'camp1',
        name: 'The Free League',
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
      },
      get_entity_relations: [],
      merge_entities: null,
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

When('the GM clicks {string} on the duplicate finding', async ({ page }, label: string) => {
  await page.getByRole('button', { name: label, exact: true }).first().click();
});

When('the GM keeps {string} as the survivor and confirms the merge', async ({ page }, _survivorName: string) => {
  const dialog = page.getByRole('dialog', { name: 'Merge entities' });
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Merge' }).click();
});

Then('the merge command is sent with the survivor and loser entities', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const call = calls.find(
    (c) =>
      c.cmd === 'merge_entities' &&
      c.args?.survivorId === 'faction:free1' &&
      c.args?.loserId === 'faction:free2',
  );
  expect(call).toBeDefined();
});
