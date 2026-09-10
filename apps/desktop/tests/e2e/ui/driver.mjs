// tauri-driver + selenium-webdriver lifecycle and IPC helpers.
//
// tauri-driver is a WebDriver proxy that launches the built Tauri binary and
// bridges to the platform webview driver. It supports Linux (WebKitWebDriver)
// and Windows (Edge driver) ONLY — there is no macOS support, so this harness
// runs in Linux CI, not on a dev Mac. See README.md.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Builder } from 'selenium-webdriver';

const DRIVER_PORT = 4444;
function driverUrl(port) {
  return `http://127.0.0.1:${port}/`;
}

/**
 * Create an app-only XDG environment for one native journey.
 *
 * tauri-driver and the application inherit these paths, so RocksDB, settings,
 * WebKit configuration, and caches cannot read or mutate the developer's
 * profile. HOME is deliberately left untouched.
 */
export function createIsolatedAppEnvironment() {
  const root = mkdtempSync(join(tmpdir(), 'chronacle-native-e2e-'));
  const dataHome = join(root, 'data');
  const configHome = join(root, 'config');
  const cacheHome = join(root, 'cache');
  for (const directory of [dataHome, configHome, cacheHome]) {
    mkdirSync(directory, { recursive: true });
  }

  return {
    root,
    env: {
      ...process.env,
      XDG_DATA_HOME: dataHome,
      XDG_CONFIG_HOME: configHome,
      XDG_CACHE_HOME: cacheHome,
      // Mesa's asynchronous shader-cache writers can outlive WebKit teardown
      // briefly and race removal of the journey's temporary XDG tree.
      MESA_SHADER_CACHE_DISABLE: 'true',
    },
    cleanup() {
      rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
    },
  };
}

/** Absolute path to the built release binary the driver should launch. */
export function appBinary() {
  // `src-tauri` is a workspace member, so the build output lands in the
  // workspace-root `target/`, not `src-tauri/target/`. Honor CARGO_TARGET_DIR
  // when set, otherwise default to the repo-root target dir. This file lives at
  // `apps/desktop/tests/e2e/ui/driver.mjs`, so the repo root is five levels up.
  const repoRoot = new URL('../../../../../', import.meta.url);
  const targetDir = process.env.CARGO_TARGET_DIR
    ? new URL('release/chronacle', `file://${process.env.CARGO_TARGET_DIR}/`)
    : new URL('target/release/chronacle', repoRoot);
  const bin = fileURLToPath(targetDir);
  if (!existsSync(bin)) {
    throw new Error(
      `Built app not found at ${bin}. Run \`pnpm tauri build --debug --features rocksdb\` (or a ` +
        `release build) before the UI E2E. See tests/e2e/ui/README.md.`,
    );
  }
  return bin;
}

/** Spawn `tauri-driver`. Returns the child process; kill it in teardown. */
export function startTauriDriver({ port = DRIVER_PORT, env = process.env } = {}) {
  const child = spawn('tauri-driver', ['--port', String(port)], {
    stdio: 'inherit',
    env,
  });
  child.on('error', (e) => {
    throw new Error(
      `Failed to spawn tauri-driver (${e.message}). Install it with ` +
        `\`cargo install tauri-driver\` and ensure WebKitWebDriver is on PATH.`,
    );
  });
  return child;
}

/** Build a WebDriver session against the launched app. */
export async function buildDriver({ port = DRIVER_PORT } = {}) {
  return new Builder()
    .withCapabilities({
      browserName: 'wry',
      'tauri:options': { application: appBinary() },
    })
    .usingServer(driverUrl(port))
    .build();
}

async function stopTauriDriver(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const stopped = new Promise((resolve) => child.once('exit', resolve));
  child.kill();
  const didStop = await Promise.race([
    stopped.then(() => true),
    new Promise((resolve) => setTimeout(() => resolve(false), 5000)),
  ]);
  if (didStop || child.exitCode !== null || child.signalCode !== null) return;
  child.kill('SIGKILL');
  await stopped;
}

/** Launch one driver/application pair with a fresh profile and reliable cleanup. */
export async function createNativeTestSession({ port = DRIVER_PORT } = {}) {
  const isolated = createIsolatedAppEnvironment();
  const tauriDriver = startTauriDriver({ port, env: isolated.env });
  let driver;
  let closed = false;

  async function close() {
    if (closed) return;
    closed = true;
    if (driver) {
      try {
        await driver.quit();
      } catch {
        // A journey may intentionally destroy the native window first.
      }
    }
    try {
      await stopTauriDriver(tauriDriver);
    } finally {
      isolated.cleanup();
    }
  }

  try {
    driver = await buildDriver({ port });
    await waitForWebviewReady(driver);
    return { driver, appEnvironment: isolated, close };
  } catch (error) {
    await close();
    throw error;
  }
}

/**
 * Invoke a Tauri command through the real webview IPC and await its result.
 * Uses `__TAURI_INTERNALS__.invoke` because the app does not enable
 * `withGlobalTauri`, so `window.__TAURI__` is unavailable.
 */
export async function invoke(driver, cmd, args = {}) {
  const outcome = await driver.executeAsyncScript(
    `const [cmd, args, done] = arguments;
     window.__TAURI_INTERNALS__
       .invoke(cmd, args)
       .then((r) => done({ ok: r }))
       .catch((e) => done({ err: String(e) }));`,
    cmd,
    args,
  );
  if (outcome && Object.prototype.hasOwnProperty.call(outcome, 'err')) {
    throw new Error(`invoke(${cmd}) failed: ${outcome.err}`);
  }
  return outcome.ok;
}

/** The app's content origin under webkit2gtk's custom protocol. */
export const APP_URL = 'tauri://localhost/';

/**
 * Navigate the WebDriver-controlled webview to the app's served URL.
 *
 * WebKitWebDriver attaches to the app's main window but resets its document to
 * `about:blank` on session start, so the Svelte frontend never loads and the
 * IPC `Origin` is `null` — every `invoke` is rejected with "Origin header is
 * not a valid URL". Loading the app URL restores a real document with a valid
 * origin. Call this before any IPC or DOM interaction, and after any reload
 * (a refresh re-triggers the about:blank reset).
 */
export async function navigateToApp(driver) {
  await driver.get(APP_URL);
}

/**
 * Wait until the webview is on the app URL and IPC answers `get_settings`.
 *
 * Centralizes the readiness gate both specs share. Logs the real error on each
 * failed invoke — the previous inline `catch {}` swallowed it, making every
 * failure look like an opaque timeout (the about:blank/origin bug above went
 * undiagnosed for exactly this reason).
 */
export async function waitForWebviewReady(driver, { timeoutMs = 20000, intervalMs = 500 } = {}) {
  await navigateToApp(driver);
  return pollUntil(
    async () => {
      try {
        await invoke(driver, 'get_settings');
        return true;
      } catch (e) {
        console.log('[e2e] webview not ready:', String(e));
        return false;
      }
    },
    { timeoutMs, intervalMs },
  );
}

/** Poll `fn` until it returns a truthy value or `timeoutMs` elapses. */
export async function pollUntil(fn, { timeoutMs = 60000, intervalMs = 1000 } = {}) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    last = await fn();
    if (last) return last;
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  throw new Error(`pollUntil timed out after ${timeoutMs}ms (last value: ${JSON.stringify(last)})`);
}
