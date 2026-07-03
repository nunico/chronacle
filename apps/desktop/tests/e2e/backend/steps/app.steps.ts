import { expect } from '@playwright/test';
import { Given, When, Then } from './fixtures';

// eslint-disable-next-line no-empty-pattern
Given('the app is running with a seeded campaign {string}', async ({}, _name: string) => {
  // Seeding comes from the shared IPC mock (get_campaigns → "Test Campaign").
  // The argument documents the precondition in the scenario text.
});

When('the GM opens the app', async ({ page }) => {
  await page.goto('/');
});

Then('the topbar shows the app title {string}', async ({ page }, title: string) => {
  await expect(page.locator('header .title')).toHaveText(title);
});
