import AxeBuilder from '@axe-core/playwright';
import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';

// Serious and critical findings block this suite. Moderate/minor findings remain visible in axe
// output without turning the first accessibility gate into an unreviewed zero-violation policy.
async function expectNoHighImpactViolations(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page }).analyze();
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
  test(`has no serious or critical axe violations at ${path}`, async ({ page }) => {
    await page.goto(path);
    await expectNoHighImpactViolations(page);
  });
}

test('has no serious or critical violations with manual search open', async ({ page }) => {
  await page.goto('/en/manual');
  await page.getByRole('button', { name: 'Search the manual' }).first().click();
  await expect(page.getByRole('dialog', { name: 'Search the manual' })).toBeVisible();
  await expectNoHighImpactViolations(page);
});

test('has no serious or critical violations with the mobile drawer open', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/en/manual');
  await page.getByRole('button', { name: 'Open manual navigation' }).click();
  await expect(page.getByRole('dialog', { name: 'Manual navigation' })).toBeVisible();
  await expectNoHighImpactViolations(page);
});
