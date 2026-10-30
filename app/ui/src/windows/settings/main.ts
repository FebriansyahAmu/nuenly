import '../../app.css';

import { invoke, isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { mount } from 'svelte';

import { type StatusView, watchStatus } from '$lib/status';

import App from './App.svelte';
import { type SettingsView, tauriBackend } from './backend';
import { SettingsState } from './settings.svelte';

const target = document.getElementById('app');

if (target && import.meta.env.DEV && !isTauri()) {
  // Di browser saat `pnpm dev`: lihat `demo.ts` untuk parameter URL.
  const { demoSettings } = await import('./demo');
  mount(App, { target, props: { state: demoSettings(new URLSearchParams(location.search)) } });
} else if (target) {
  const [settings, status] = await Promise.all([
    invoke<SettingsView>('settings_get'),
    invoke<StatusView>('status_get'),
  ]);
  const state = new SettingsState(tauriBackend, settings, status);
  mount(App, { target, props: { state } });
  // Mode dan jeda bisa berubah dari tray; aksen mode ikut dipasang.
  void watchStatus((next) => (state.status = next));
  // Jendela ini disembunyikan, bukan ditutup; periksa Ollama lagi setiap kali dibuka.
  void getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (focused) {
      void state.checkOllama();
    }
  });
}
