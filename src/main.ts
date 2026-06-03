// @ts-ignore - @fontsource packages are CSS-only, no TS declarations
import '@fontsource-variable/cinzel';
// @ts-ignore - @fontsource packages are CSS-only, no TS declarations
import '@fontsource/spectral';
// @ts-ignore - @fontsource packages are CSS-only, no TS declarations
import '@fontsource-variable/hanken-grotesk';
// @ts-ignore - @fontsource packages are CSS-only, no TS declarations
import '@fontsource-variable/jetbrains-mono';
import './lib/tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

mount(App, {
  target: document.getElementById('app')!,
});