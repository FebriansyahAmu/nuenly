import '../../app.css';

import { isTauri } from '@tauri-apps/api/core';
import { mount } from 'svelte';

import { watchStatus } from '$lib/status';

import App from './App.svelte';
import { AskSession } from './session.svelte';
import { tauriBackend } from './tauri';

// Di browser saat `pnpm dev`, sesi diisi contoh sintetis agar tampilan bisa dicek tanpa Tauri.
// Di build rilis cabang ini hilang karena `import.meta.env.DEV` bernilai false.
const session =
  import.meta.env.DEV && !isTauri()
    ? (await import('./demo')).demoSession(new URLSearchParams(location.search))
    : new AskSession(tauriBackend);

const target = document.getElementById('app');
if (target) {
  mount(App, { target, props: { session } });
}
if (isTauri()) {
  // Titik logo mengikuti warna aksen mode aktif.
  void watchStatus();
}
