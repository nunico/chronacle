import '@fontsource-variable/cinzel';
import '@fontsource/spectral';
import '@fontsource-variable/hanken-grotesk';
import '@fontsource-variable/jetbrains-mono';
import './lib/tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

mount(App, {
  target: document.getElementById('app')!,
});
