import '../../app.css';

import { isTauri } from '@tauri-apps/api/core';
import { type Component, mount } from 'svelte';

import App from './App.svelte';

const target = document.getElementById('app');

if (target && import.meta.env.DEV && !isTauri()) {
  // Di browser saat `pnpm dev`: `?galeri` menampilkan semua varian, `?varian=<nama>` satu toast.
  const params = new URLSearchParams(location.search);
  const { DEMO_TOASTS } = await import('./demo');
  if (params.has('galeri')) {
    const Gallery: Component = (await import('./Gallery.svelte')).default;
    mount(Gallery, { target });
  } else {
    const demo = DEMO_TOASTS[params.get('varian') ?? 'saran'];
    mount(App, { target, props: { initial: demo ? [demo] : [] } });
  }
} else if (target) {
  mount(App, { target });
}
