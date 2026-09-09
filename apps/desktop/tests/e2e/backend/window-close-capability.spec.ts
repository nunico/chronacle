import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';

interface CapabilityManifest {
  readonly permissions: string[];
}

const manifestUrl = new URL('../../../src-tauri/capabilities/default.json', import.meta.url);
const manifest = JSON.parse(readFileSync(manifestUrl, 'utf8')) as CapabilityManifest;

for (const permission of ['core:window:allow-close', 'core:window:allow-destroy']) {
  test(`grants the exact native draft-close permission ${permission}`, () => {
    expect(manifest.permissions).toContain(permission);
  });
}

test('does not broaden window authority with wildcard or allow-all permissions', () => {
  const broadWindowPermissions = manifest.permissions.filter(
    (permission) =>
      permission.includes('*') ||
      permission === 'core:all' ||
      permission === 'core:window:all' ||
      permission === 'core:window:allow-all',
  );

  expect(broadWindowPermissions).toEqual([]);
});
