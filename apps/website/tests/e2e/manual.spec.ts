import { expect, test } from './fixtures';

test('opens the paired German article from an English article', async ({ page }) => {
  await page.goto('/en/manual/getting-started/install');
  await expect(page.getByRole('heading', { name: 'Install Chronacle', level: 1 })).toBeVisible();

  const translation = page.getByRole('link', { name: 'Deutsch' });
  await expect(translation).toHaveAttribute('href', '/de/handbuch/erste-schritte/installieren');
  await translation.click();

  await expect(page).toHaveURL(/\/de\/handbuch\/erste-schritte\/installieren\/?$/);
  await expect(
    page.getByRole('heading', { name: 'Chronacle installieren', level: 1 }),
  ).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'de');
});

test('supports both manual shortcuts, search results, Enter, Escape, and focus restoration', async ({
  page,
}) => {
  await page.goto('/en/manual');
  const trigger = page.getByRole('button', { name: 'Search the manual' }).first();
  await trigger.focus();

  await page.keyboard.press('Control+K');
  const dialog = page.getByRole('dialog', { name: 'Search the manual' });
  const input = page.getByRole('combobox', { name: 'Search the manual' });
  await expect(dialog).toBeVisible();
  await expect(input).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await expect(trigger).toBeFocused();

  await page.keyboard.press('Meta+K');
  await input.fill('campaign');
  const firstResult = dialog.getByRole('option').first();
  await expect(firstResult).toBeVisible();
  const resultHref = await firstResult.getAttribute('href');
  expect(resultHref).toMatch(/^\/en\/manual\//);
  await input.press('Enter');
  await expect(page).toHaveURL(
    new RegExp(`${resultHref?.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/?$`),
  );
});

test('opens and closes the mobile manual drawer at 390px', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/en/manual');

  const trigger = page.getByRole('button', { name: 'Open manual navigation' });
  await expect(trigger).toBeVisible();
  await trigger.click();
  const drawer = page.getByRole('dialog', { name: 'Manual navigation' });
  await expect(drawer).toBeVisible();
  await expect(drawer.getByRole('navigation', { name: 'Navigation in menu' })).toBeVisible();
  await drawer.getByRole('button', { name: 'Close manual navigation' }).click();
  await expect(drawer).not.toBeVisible();
  await expect(trigger).toBeFocused();
});

test('localizes an unknown manual route through the static fallback', async ({ page }) => {
  const response = await page.goto('/de/handbuch/gibt-es-nicht');

  expect(response?.status()).toBe(404);
  await expect(page.getByRole('heading', { name: 'Diese Seite gibt es nicht.' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Handbuchüberblick' })).toHaveAttribute(
    'href',
    '/de/handbuch',
  );
  await expect(page.getByRole('button', { name: 'Handbuch durchsuchen' })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('lang', 'de');
});
