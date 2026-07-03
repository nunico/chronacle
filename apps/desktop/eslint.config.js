// @ts-check
import { defineConfig } from 'eslint/config';
import js from '@eslint/js';
import ts from 'typescript-eslint';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import svelteConfig from './svelte.config.js';

export default defineConfig(
  {
    ignores: ['.agents/*', '.claude/*', 'dist/*', 'target/*', 'tests/e2e/.features-gen/**'],
  },
  { languageOptions: { globals: { ...globals.browser } } },
  js.configs.recommended,
  ts.configs.strict,
  ts.configs.stylistic,
  svelte.configs.recommended,
  {
    files: ['**/*.{js,ts}'],
    rules: {
      '@typescript-eslint/array-type': ['error', { default: 'array-simple' }],
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
      'no-console': 'off',
    },
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: {
      globals: { ...globals.browser, ...globals.svelte },
      parserOptions: {
        projectService: true,
        extraFileExtensions: ['.svelte'],
        parser: ts.parser,
        svelteConfig,
      },
    },
    rules: {
      '@typescript-eslint/array-type': ['error', { default: 'array-simple' }],
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
      'no-console': 'off',
    },
  },
  {
    files: ['**/*.{spec,test}.{js,ts}'],
    rules: {
      '@typescript-eslint/no-empty-function': 'off',
    }
  },
  {
    // tauri-driver E2E harness — Node scripts with Mocha globals.
    files: ['tests/e2e/ui/**/*.mjs'],
    languageOptions: { globals: { ...globals.node, ...globals.mocha } },
  }
);
