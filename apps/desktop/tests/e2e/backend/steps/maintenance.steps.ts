import { expect } from '@playwright/test';
import { Given, When, Then } from './fixtures';
import { installIpcMock } from '../ipc-mock';

interface IpcCall {
  cmd: string;
  args?: Record<string, unknown>;
}

const ANSWER_TEXT =
  'Yes, a grappled creature has a speed of 0. [Source: "SRD 5.2", p.190, quote: "Speed becomes 0."]';

const PENDING_PROPOSALS = [
  {
    id: 'prop1',
    kind: 'entity_article_update',
    target: 'npc1',
    target_name: 'Mira',
    current_text: 'Mira is a wandering merchant.',
    payload: {
      proposed_text: 'Mira is a wandering merchant who now deals in relics.',
      rationale: 'The party learned Mira deals in ancient relics.',
      name: null,
      entity_kind: null,
      category: null,
    },
    origin_kind: 'session',
    status: 'pending',
    created_at: '2026-07-05T00:00:00Z',
  },
  {
    id: 'prop2',
    kind: 'new_entity',
    target: null,
    target_name: null,
    current_text: null,
    payload: {
      proposed_text: 'A ruined watchtower on the northern ridge.',
      rationale: 'A new location was mentioned in the session.',
      name: 'Northern Watchtower',
      entity_kind: 'location',
      category: null,
    },
    origin_kind: 'session',
    status: 'pending',
    created_at: '2026-07-05T00:05:00Z',
  },
];

async function getIpcCalls(page: import('@playwright/test').Page): Promise<IpcCall[]> {
  return page.evaluate(() => (window as unknown as { __ipcCalls: IpcCall[] }).__ipcCalls);
}

Given('the assistant has answered a question', async ({ page }) => {
  await installIpcMock(page, {
    get_chat_history: [{ role: 'assistant', content: ANSWER_TEXT }],
    save_chat_to_codex: 2,
  });
  await page.goto('/');
});

When('the GM clicks {string} on the answer', async ({ page }, label: string) => {
  await page.getByRole('button', { name: label, exact: false }).click();
});

Then('the save-to-codex command is sent with the answer text', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const saveCall = calls.find(
    (c) => c.cmd === 'save_chat_to_codex' && c.args?.content === ANSWER_TEXT,
  );
  expect(saveCall).toBeDefined();
});

Then('a toast reports the created proposals', async ({ page }) => {
  await expect(page.locator('.toast-msg')).toContainText(/proposal/i);
});

Given('the maintenance inbox lists a pending proposal for {string}', async ({ page }, name: string) => {
  await installIpcMock(page, {
    get_proposals: PENDING_PROPOSALS,
    get_maintenance_counts: { pending_proposals: PENDING_PROPOSALS.length, unresolved_findings: 0 },
    accept_proposal: null,
    reject_proposal: null,
  });
  await page.goto('/');
  await page.getByRole('button', { name: /Maintenance/ }).click();
  await expect(page.locator('.proposal-card', { hasText: name })).toBeVisible();
});

When('the GM accepts the proposal', async ({ page }) => {
  await page.getByRole('button', { name: 'Accept proposal' }).first().click();
});

Then('the accept command is sent for that proposal', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const acceptCall = calls.find((c) => c.cmd === 'accept_proposal' && c.args?.id === 'prop1');
  expect(acceptCall).toBeDefined();
});

When('the GM rejects the remaining proposal', async ({ page }) => {
  // The accept-proposal fixture is static and doesn't remove the accepted card
  // from the mocked list, so the "remaining" proposal is the second card.
  await page.getByRole('button', { name: 'Reject proposal' }).nth(1).click();
});

Then('the reject command is sent for that proposal', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const rejectCall = calls.find((c) => c.cmd === 'reject_proposal' && c.args?.id === 'prop2');
  expect(rejectCall).toBeDefined();
});

Given(
  'the maintenance inbox has a broken-wikilink finding for {string}',
  async ({ page }, linkText: string) => {
    const bareLinkText = linkText.replace(/^\[\[/, '').replace(/\]\]$/, '');
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint1',
          kind: 'broken_wikilink',
          payload: { entity: 'npc:mira', link_text: bareLinkText },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      resolve_lint_finding: null,
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

Given(
  'the maintenance inbox has a duplicate-entity finding for {string}',
  async ({ page }, name: string) => {
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint2',
          kind: 'duplicate_entity',
          payload: { a: `npc:${name}`, b: `npc:${name}-2`, similarity: 1.0 },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      resolve_lint_finding: null,
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

When('the GM opens the findings tab', async ({ page }) => {
  await page.getByRole('tab', { name: /Findings/ }).click();
});

Then(
  'the finding {string} is listed with {string}',
  async ({ page }, kindLabel: string, detail: string) => {
    const group = page.locator('.finding-group', { hasText: kindLabel });
    await expect(group).toBeVisible();
    await expect(group).toContainText(detail);
  },
);

Given(
  'the inbox has a naming conflict for {string} between {string} and {string}',
  async ({ page }, term: string, nameA: string, nameB: string) => {
    await installIpcMock(page, {
      get_proposals: [],
      get_lint_findings: [
        {
          id: 'lint3',
          kind: 'alias_collision',
          payload: {
            alias: term,
            a: 'faction:a',
            b: 'faction:b',
            a_name: nameA,
            b_name: nameB,
            a_is_name: false,
            b_is_name: false,
          },
          created_at: '2026-07-06T00:00:00Z',
        },
      ],
      get_maintenance_counts: { pending_proposals: 0, unresolved_findings: 1 },
      resolve_lint_finding: null,
      resolve_alias_collision: null,
    });
    await page.goto('/');
    await page.getByRole('button', { name: /Maintenance/ }).click();
  },
);

When('the GM keeps the term on {string}', async ({ page }, name: string) => {
  await page.getByRole('button', { name: `Keep on ${name}` }).click();
});

Then(
  'the resolve-collision command keeps {string} and drops {string}',
  async ({ page }, keepId: string, dropId: string) => {
    const calls = await getIpcCalls(page);
    const call = calls.find(
      (c) =>
        c.cmd === 'resolve_alias_collision' &&
        c.args?.keepId === keepId &&
        c.args?.dropId === dropId,
    );
    expect(call).toBeDefined();
  },
);

When('the GM marks the finding resolved', async ({ page }) => {
  await page.getByRole('button', { name: 'Mark resolved' }).first().click();
});

Then('the resolve command is sent for that finding', async ({ page }) => {
  const calls = await getIpcCalls(page);
  const resolveCall = calls.find(
    (c) => c.cmd === 'resolve_lint_finding' && c.args?.id === 'lint1',
  );
  expect(resolveCall).toBeDefined();
});
