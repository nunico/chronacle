import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { dirname } from 'node:path';
import { createIsolatedAppEnvironment } from './driver.mjs';

describe('native UI test environment', () => {
  it('isolates application data, configuration, and cache without changing HOME', () => {
    const originalHome = process.env.HOME;
    const isolated = createIsolatedAppEnvironment();

    try {
      assert.equal(process.env.HOME, originalHome);
      assert.equal(isolated.env.HOME, originalHome);
      assert.equal(dirname(isolated.env.XDG_DATA_HOME), isolated.root);
      assert.equal(dirname(isolated.env.XDG_CONFIG_HOME), isolated.root);
      assert.equal(dirname(isolated.env.XDG_CACHE_HOME), isolated.root);
      assert.equal(isolated.env.MESA_SHADER_CACHE_DISABLE, 'true');
      assert.notEqual(isolated.env.XDG_DATA_HOME, isolated.env.XDG_CONFIG_HOME);
      assert.notEqual(isolated.env.XDG_DATA_HOME, isolated.env.XDG_CACHE_HOME);
      assert.notEqual(isolated.env.XDG_CONFIG_HOME, isolated.env.XDG_CACHE_HOME);
      assert.equal(existsSync(isolated.env.XDG_DATA_HOME), true);
      assert.equal(existsSync(isolated.env.XDG_CONFIG_HOME), true);
      assert.equal(existsSync(isolated.env.XDG_CACHE_HOME), true);
    } finally {
      isolated.cleanup();
    }

    assert.equal(existsSync(isolated.root), false);
    assert.doesNotThrow(() => isolated.cleanup(), 'cleanup should be safe to repeat');
  });
});
