import js from '@eslint/js';
import tsParser from '@typescript-eslint/parser';
import typescriptEslint from '@typescript-eslint/eslint-plugin';
import sveltePlugin from 'eslint-plugin-svelte';
import globals from 'globals';

export default [
  {
    ignores: ['.agents/*', '.claude/*', 'dist/*'],
  },
  js.configs.recommended,
  ...sveltePlugin.configs['flat/recommended'],
  {
    files: ['**/*.ts'],
    plugins: {
      '@typescript-eslint': typescriptEslint,
    },
    languageOptions: {
      parser: tsParser,
      sourceType: 'module',
      ecmaVersion: 2022,
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    rules: {
      ...typescriptEslint.configs.recommended.rules,
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
      'no-console': 'off',
    },
  },
  {
    files: ['**/*.svelte'],
    plugins: {
      '@typescript-eslint': typescriptEslint,
    },
    languageOptions: {
      globals: {
        ...globals.browser,
      },
      parserOptions: {
        parser: tsParser,
      },
    },
    rules: {
      ...typescriptEslint.configs.recommended.rules,
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
      'no-console': 'off',
    },
  },
];
