import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  resolve: {
    // Ensure Vite resolves the 'browser' entry for Svelte 5.
    // The @sveltejs/vite-plugin-svelte sets conditions:['svelte'] which is
    // absent in Svelte 5's exports map, causing the server-side build
    // (without mount/hydrate) to be selected.
    conditions: ['browser', 'module', 'svelte'],
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || '127.0.0.1',
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
});