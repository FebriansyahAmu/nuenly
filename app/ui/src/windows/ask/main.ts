import '../../app.css';

import { isTauri } from '@tauri-apps/api/core';
import { mount } from 'svelte';

import App from './App.svelte';
import { unavailableBackend } from './backend';
import { AskSession } from './session.svelte';

// Di browser saat `pnpm dev`, sesi diisi contoh sintetis agar tampilan bisa dicek tanpa Tauri.
// Di build rilis cabang ini hilang karena `import.meta.env.DEV` bernilai false.
const session =
  import.meta.env.DEV && !isTauri()
    ? (await import('./demo')).demoSession(new URLSearchParams(location.search))
    : new AskSession(unavailableBackend);

const target = document.getElementById('app');
if (target) {
  mount(App, { target, props: { session } });
}
