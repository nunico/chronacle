import { expect, test as base } from '@playwright/test';

export const test = base.extend({
  page: async ({ page }, use) => {
    const browserFailures: string[] = [];
    const genericResourceErrors: string[] = [];
    let mainDocumentNotFound = false;
    page.on('pageerror', (error) => browserFailures.push(`pageerror: ${error.message}`));
    page.on('response', (response) => {
      const request = response.request();
      if (request.resourceType() === 'document' && request.frame() === page.mainFrame()) {
        mainDocumentNotFound = response.status() === 404;
      } else if (response.status() >= 400) {
        browserFailures.push(`response ${response.status()}: ${response.url()}`);
      }
    });
    page.on('console', (message) => {
      if (message.type() === 'error') {
        const text = message.text();
        if (
          text === 'Failed to load resource: the server responded with a status of 404 (Not Found)'
        ) {
          genericResourceErrors.push(text);
        } else {
          browserFailures.push(`console: ${text}`);
        }
      }
    });

    await use(page);

    if (!mainDocumentNotFound) {
      browserFailures.push(...genericResourceErrors.map((error) => `console: ${error}`));
    }
    expect(browserFailures, 'page errors and console errors').toEqual([]);
  },
});

export { expect };
