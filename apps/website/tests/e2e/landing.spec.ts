import { expect, test } from './fixtures';

test('uses English as the root fallback and publishes route-absolute metadata', async ({
  page,
}) => {
  await page.goto('/');

  await expect(
    page.getByRole('heading', { name: 'Chronacle — Ask your books. Check the answer.' }),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(page).toHaveTitle('Chronacle — cited answers from your books');
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute('href', '/');
  await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
    'content',
    '/brand/chronacle-icon.png',
  );
  const seoValues = await page
    .locator('link[rel="canonical"], meta[property^="og:"]')
    .evaluateAll((elements) =>
      elements.map((element) => element.getAttribute('href') ?? element.getAttribute('content')),
    );
  expect(seoValues.join(' ')).not.toMatch(/localhost|127\.0\.0\.1/);
});

test('initializes German from the browser language without changing the route', async ({
  browser,
}) => {
  const context = await browser.newContext({ locale: 'de-DE' });
  const page = await context.newPage();
  const failures: string[] = [];
  page.on('pageerror', (error) => failures.push(error.message));
  page.on('console', (message) => {
    if (message.type() === 'error') failures.push(message.text());
  });

  await page.goto('http://127.0.0.1:4174/');

  await expect(
    page.getByRole('heading', { name: 'Chronacle — Frag deine Bücher. Prüf die Antwort.' }),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'de');
  expect(new URL(page.url()).pathname).toBe('/');
  expect(failures).toEqual([]);
  await context.close();
});

test('switches copy in place and sends the manual call to the matching locale', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Deutsch' }).click();

  await expect(
    page.getByRole('heading', { name: 'Chronacle — Frag deine Bücher. Prüf die Antwort.' }),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'de');
  await expect(page.getByRole('link', { name: 'Handbuch lesen' }).first()).toHaveAttribute(
    'href',
    '/de/handbuch',
  );
  expect(new URL(page.url()).pathname).toBe('/');
});

test('uses the reviewed download and source destinations and external-link policy', async ({
  page,
}) => {
  await page.goto('/');

  const downloads = page.getByRole('link', { name: 'Download Chronacle' });
  await expect(downloads).toHaveCount(3);
  for (let index = 0; index < (await downloads.count()); index += 1) {
    await expect(downloads.nth(index)).toHaveAttribute(
      'href',
      'https://github.com/nunico/chronacle/releases/latest',
    );
    await expect(downloads.nth(index)).toHaveAttribute('target', '_blank');
    await expect(downloads.nth(index)).toHaveAttribute('rel', 'external noopener noreferrer');
  }

  const sources = page.getByRole('link', { name: 'Source' });
  await expect(sources).toHaveCount(2);
  for (let index = 0; index < (await sources.count()); index += 1) {
    await expect(sources.nth(index)).toHaveAttribute('href', 'https://github.com/nunico/chronacle');
    await expect(sources.nth(index)).toHaveAttribute('rel', 'external');
    await expect(sources.nth(index)).not.toHaveAttribute('target', '_blank');
  }
});

test('serves the custom English page through the static fallback', async ({ page }) => {
  const response = await page.goto('/not-a-real-page');

  expect(response?.status()).toBe(404);
  await expect(page.getByRole('heading', { name: 'That page is not here.' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Manual overview' })).toHaveAttribute(
    'href',
    '/en/manual',
  );
});
