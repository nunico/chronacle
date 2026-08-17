import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { mdsvex } from 'mdsvex';
import { fileURLToPath } from 'node:url';

const manualLayout = fileURLToPath(
  new URL('./src/lib/manual/ManualArticle.svelte', import.meta.url),
);

/** @type {import('@sveltejs/kit').Config} */
const config = {
  extensions: ['.svelte', '.md'],
  preprocess: [
    vitePreprocess(),
    mdsvex({
      extensions: ['.md'],
      layout: manualLayout,
    }),
  ],
  kit: {
    adapter: adapter({ fallback: '404.html' }),
    prerender: {
      handleHttpError: 'fail',
    },
  },
};

export default config;
