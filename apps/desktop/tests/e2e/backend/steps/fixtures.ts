import { test as base, createBdd } from 'playwright-bdd';
import { installIpcMock } from '../ipc-mock';

/** BDD test with the Tauri IPC mock auto-installed before every scenario. */
export const test = base.extend<{ ipcMock: unknown }>({
  ipcMock: [
    async ({ page }, use) => {
      await installIpcMock(page);
      await use();
    },
    { auto: true },
  ],
});

export const { Given, When, Then } = createBdd(test);
