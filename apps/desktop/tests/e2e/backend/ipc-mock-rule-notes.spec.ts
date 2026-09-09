import { expect, test } from '@playwright/test';
import { installIpcMock } from './ipc-mock';

interface TauriWindow extends Window {
  __TAURI_INTERNALS__: {
    invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
  };
}

test('ordinary rule-note saves acknowledge with the canonical rule entry', async ({ page }) => {
  await installIpcMock(page);
  await page.goto('/');

  const saved = await page.evaluate(async () => {
    return (window as unknown as TauriWindow).__TAURI_INTERNALS__.invoke('update_rule_notes', {
      id: 'rule1',
      notes: 'One initiative roll per side.',
    });
  });

  expect(saved).toEqual({
    id: 'rule1',
    name: 'Initiative',
    category: 'mechanic',
    body: 'Roll a d20 and add your Dexterity modifier to determine turn order.',
    notes: 'One initiative roll per side.',
    page_refs: [{ source_name: 'Core Rulebook', page_start: 12, page_end: 13 }],
    stale: false,
  });
});
