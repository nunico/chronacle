import '@fontsource-variable/cinzel';
import '@fontsource/spectral';
import '@fontsource-variable/hanken-grotesk';
import '@fontsource-variable/jetbrains-mono';
import './lib/tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

const target = document.getElementById('app')

if (target != null) {
  mount(App, { target });
} else {
  console.error('Target element for app mounting not found.')
}
