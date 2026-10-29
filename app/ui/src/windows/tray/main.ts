import '../../app.css';

import { invoke, isTauri } from '@tauri-apps/api/core';
import { mount } from 'svelte';

import { type StatusView, watchStatus } from '$lib/status';

import App from './App.svelte';
import { tauriBackend } from './backend';

const target = document.getElementById('app');

if (target && import.meta.env.DEV && !isTauri()) {
  // Di browser saat `pnpm dev`: status contoh tanpa Tauri.
  const { demoTray } = await import('./demo');
  // Browser headless punya lebar minimum; batasi ke ukuran popup agar tampilannya setara.
  target.style.width = '320px';
  target.style.outline = '1px solid #e4e4e7';
  mount(App, { target, props: demoTray(new URLSearchParams(location.search)) });
} else if (target) {
  const initial = await invoke<StatusView>('status_get');
  mount(App, { target, props: { backend: tauriBackend, initial, watch: watchStatus } });
}
