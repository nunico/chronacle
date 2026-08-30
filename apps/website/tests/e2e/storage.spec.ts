import { expect, test } from './fixtures';

test('switching, navigation, and search leave browser storage empty and requests local', async ({
  context,
  page,
}) => {
  const contactedHosts = new Set<string>();
  const prohibitedRequests: string[] = [];
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.protocol === 'http:' || url.protocol === 'https:') {
      contactedHosts.add(url.hostname);
      if (url.hostname !== '127.0.0.1') prohibitedRequests.push(request.url());
    }
  });

  await page.goto('/');
  await page.getByRole('button', { name: 'Deutsch' }).click();
  await page.getByRole('link', { name: 'Handbuch lesen' }).first().click();
  await expect(page).toHaveURL(/\/de\/handbuch\/?$/);
  await page.keyboard.press('Control+K');
  const input = page.getByRole('combobox', { name: 'Handbuch durchsuchen' });
  await input.fill('Kampagne');
  await expect(page.getByRole('dialog').getByRole('option').first()).toBeVisible();

  expect(await context.cookies()).toEqual([]);
  const browserState = await page.evaluate(async () => ({
    localStorageKeys: Object.keys(localStorage),
    sessionStorageKeys: Object.keys(sessionStorage),
    indexedDatabases: (await indexedDB.databases()).map((database) => database.name),
    serviceWorkerCount:
      'serviceWorker' in navigator ? (await navigator.serviceWorker.getRegistrations()).length : 0,
  }));
  expect(browserState).toEqual({
    localStorageKeys: [],
    sessionStorageKeys: [],
    indexedDatabases: [],
    serviceWorkerCount: 0,
  });
  expect([...contactedHosts]).toEqual(['127.0.0.1']);
  expect(prohibitedRequests).toEqual([]);
});
