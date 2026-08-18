import AxeBuilder from '@axe-core/playwright';
import type { Page, TestInfo } from '@playwright/test';
import { expect, test } from './fixtures';

// Serious and critical findings block this suite. Every axe finding is attached as structured JSON
// so moderate and minor findings remain reviewable without becoming an unreviewed blocking policy.
async function expectNoHighImpactViolations(page: Page, testInfo: TestInfo): Promise<void> {
  const results = await new AxeBuilder({ page }).analyze();
  await testInfo.attach('axe-violations', {
    body: JSON.stringify(results.violations, undefined, 2),
    contentType: 'application/json',
  });
  const blocking = results.violations.filter(
    (violation) => violation.impact === 'serious' || violation.impact === 'critical',
  );
  expect(blocking).toEqual([]);
}

for (const path of [
  '/',
  '/en/manual',
  '/de/handbuch',
  '/en/manual/troubleshooting/common-problems',
  '/not-a-real-page',
]) {
  test(`has no serious or critical axe violations at ${path}`, async ({ page }, testInfo) => {
    await page.goto(path);
    await expectNoHighImpactViolations(page, testInfo);
  });
}

test('has no serious or critical violations with manual search open', async ({
  page,
}, testInfo) => {
  await page.goto('/en/manual');
  await page.getByRole('button', { name: 'Search the manual' }).first().click();
  await expect(page.getByRole('dialog', { name: 'Search the manual' })).toBeVisible();
  await expectNoHighImpactViolations(page, testInfo);
});

test('has no serious or critical violations with the mobile drawer open', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/en/manual');
  await page.getByRole('button', { name: 'Open manual navigation' }).click();
  await expect(page.getByRole('dialog', { name: 'Manual navigation' })).toBeVisible();
  await expectNoHighImpactViolations(page, testInfo);
});

for (const [path, headerSelector] of [
  ['/', '.site-header'],
  ['/en/manual', '.manual-header'],
  ['/de/handbuch', '.manual-header'],
  ['/legal/open-game-license', '.legal-header'],
] as const) {
  test(`exposes one main with page-level header and footer landmarks at ${path}`, async ({
    page,
  }) => {
    await page.goto(path);
    const main = page.getByRole('main');
    const pageHeader = page.locator(headerSelector);
    const pageFooter = page.locator('.site-footer');
    await expect(main).toHaveCount(1);
    await expect(pageHeader).toHaveRole('banner');
    await expect(pageFooter).toHaveRole('contentinfo');
    await expect(main.locator(headerSelector)).toHaveCount(0);
    await expect(main.locator('.site-footer')).toHaveCount(0);
  });
}
