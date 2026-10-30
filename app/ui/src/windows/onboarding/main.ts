import '../../app.css';

import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { mount } from 'svelte';

import { watchStatus } from '$lib/status';

import App from './App.svelte';
import { tauriBackend } from './backend';
import { OnboardingFlow } from './flow.svelte';

const target = document.getElementById('app');

if (target && import.meta.env.DEV && !isTauri()) {
  // Di browser saat `pnpm dev`: lihat `demo.ts` untuk parameter URL.
  const { demoOnboarding } = await import('./demo');
  const { backend, step } = demoOnboarding(new URLSearchParams(location.search));
  mount(App, { target, props: { flow: new OnboardingFlow(backend, step) } });
} else if (target) {
  const flow = new OnboardingFlow(tauriBackend);
  mount(App, { target, props: { flow } });
  // Titik logo mengikuti warna aksen mode aktif.
  void watchStatus();
  // Setelah memasang atau membuka Ollama, pengguna kembali ke jendela ini; periksa lagi saat itu.
  void getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (focused) {
      flow.recheck();
    }
  });
}
