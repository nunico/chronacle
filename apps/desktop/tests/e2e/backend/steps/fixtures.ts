import { test as base, createBdd } from 'playwright-bdd';
import { installIpcMock } from '../ipc-mock';

/** BDD test with the Tauri IPC mock auto-installed before every scenario. */
// eslint-disable-next-line @typescript-eslint/no-invalid-void-type
export const test = base.extend<{ ipcMock: void }>({
  ipcMock: [
    async ({ page }, use) => {
      await installIpcMock(page);
      await use();
    },
    { auto: true },
  ],
});

export const { Given, When, Then } = createBdd(test);
