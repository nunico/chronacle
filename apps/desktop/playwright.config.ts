import { defineConfig } from '@playwright/test';
import { defineBddConfig } from 'playwright-bdd';

const bddTestDir = defineBddConfig({
  features: 'tests/e2e/features/**/*.feature',
  steps: 'tests/e2e/backend/steps/**/*.ts',
  outputDir: 'tests/e2e/.features-gen',
});

export default defineConfig({
  timeout: 30000,
  retries: 0,
  use: {
    baseURL: 'http://localhost:1420',
    headless: true,
  },
  webServer: {
    command: 'pnpm dev',
    port: 1420,
    reuseExistingServer: true,
    timeout: 15000,
  },
  projects: [
    // Existing hand-written backend specs (mocked-IPC frontend suite).
    { name: 'backend', testDir: './tests/e2e/backend' },
    // Generated from tests/e2e/features/*.feature by `bddgen` (ADR-011).
    { name: 'bdd', testDir: bddTestDir },
  ],
});
