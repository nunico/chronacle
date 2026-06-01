import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const host = process.env.TAURI_DEV_HOST;
const isTest = !!process.env.VITEST;

export default defineConfig({
  plugins: [
    svelte({
      hot: !isTest,
      // In the test environment, skip vitePreprocess so that Vite's
      // PartialEnvironment (which requires a real Vite server context) is not
      // invoked. The Svelte compiler handles <style> blocks natively.
      configFile: isTest ? false : undefined,
    }),
  ],
  // Force browser package conditions so Svelte resolves to its client bundle
  // (index-client.js) rather than the SSR bundle when running under Vitest.
  resolve: {
    conditions: ['browser'],
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host ? host : false,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['src/**/*.test.ts'],
  },
});
