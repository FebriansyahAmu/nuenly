// Mode demo Pengaturan untuk melihat tampilan di browser saat `pnpm dev`. Hanya dimuat di build dev.

import type { OllamaInfo } from '$lib/ollama';
import type { StatusView } from '$lib/status';

import type { SettingsBackend, SettingsView } from './backend';
import { PAGES, SettingsState } from './settings.svelte';

const MODES = [
  { id: 'all', label: 'Umum', accent: '#4f46e5' },
  { id: 'medical', label: 'Medical', accent: '#0f766e' },
];

/** PC pengembang: RAM 7,7 GB dan RTX 3050 6 GB. */
const HARDWARE = { ramBytes: 8_283_000_000, vramBytes: 6 * 1024 ** 3, gpu: 'NVIDIA GeForce RTX 3050 6GB Laptop GPU' };

const OLLAMA: Record<string, OllamaInfo> = {
  running: { status: 'running', version: '0.40.13', models: ['gemma3:4b', 'gemma4:e4b'], plan: null, hardware: HARDWARE },
  kosong: { status: 'running', version: '0.40.13', models: [], plan: null, hardware: HARDWARE },
  installed: { status: 'installed', version: null, models: [], plan: null, hardware: HARDWARE },
  missing: { status: 'missing', version: null, models: [], plan: null, hardware: HARDWARE },
};

/** Meniru `ResourceProfiler` untuk PC pengembang: gemma3:4b muat di GPU, gemma4:e4b tidak. */
function withPlan(info: OllamaInfo, model: string | null): OllamaInfo {
  const name = model ?? 'gemma3:4b';
  if (info.status !== 'running' || !info.models.includes(name)) {
    return info;
  }
  const gpu = name === 'gemma3:4b';
  return {
    ...info,
    plan: {
      model: name,
      automatic: model === null,
      placement: gpu ? 'gpu' : 'split',
      numCtx: gpu ? 8192 : 4096,
      keepAliveSecs: gpu ? 1800 : 300,
      kvQ8Recommended: !gpu,
    },
  };
}

/**
 * `?halaman=general|model|privacy|connector` (bawaannya model, seperti desain 07),
 * `?ollama=running|kosong|installed|missing`, `?otomatis`, `?jeda`, `?pintasan=taken|failed`,
 * dan `?tombol=<pintasan>` (misalnya `Alt+Space`).
 */
export function demoSettings(params: URLSearchParams): SettingsState {
  const hotkeyStatus = params.get('pintasan');
  let settings: SettingsView = {
    model: params.has('otomatis') ? null : 'gemma4:e4b',
    hotkey: params.get('tombol') ?? 'Ctrl+Space',
    hotkeyStatus: hotkeyStatus === 'taken' || hotkeyStatus === 'failed' ? hotkeyStatus : 'active',
    ollamaAddress: '127.0.0.1:11434',
    blockedApps: ['nuenly.exe', 'credentialuibroker.exe', 'consent.exe', 'keepass.exe', '1password.exe'],
  };
  let status: StatusView = {
    modes: MODES,
    activeMode: 'all',
    pausedUntilMs: params.has('jeda') ? Date.now() + 15 * 60 * 1000 : null,
  };
  const ollama = OLLAMA[params.get('ollama') ?? 'running'] ?? OLLAMA.running;
  const updateStatus = (change: Partial<StatusView>) => Promise.resolve((status = { ...status, ...change }));
  const backend: SettingsBackend = {
    setModel: (model) => Promise.resolve((settings = { ...settings, model })),
    // Ctrl+Shift+K meniru kombinasi yang sudah dipakai aplikasi lain.
    setHotkey: (shortcut) =>
      shortcut === 'Ctrl+Shift+K'
        ? Promise.reject('taken')
        : Promise.resolve((settings = { ...settings, hotkey: shortcut ?? 'Ctrl+Space', hotkeyStatus: 'active' })),
    checkOllama: () => Promise.resolve(withPlan(ollama as OllamaInfo, settings.model)),
    openDownload: () => Promise.resolve(),
    setMode: (id) => updateStatus({ activeMode: id }),
    pause: () => updateStatus({ pausedUntilMs: Date.now() + 15 * 60 * 1000 }),
    resume: () => updateStatus({ pausedUntilMs: null }),
  };

  const state = new SettingsState(backend, settings, status);
  const page = PAGES.find((candidate) => candidate.id === params.get('halaman'));
  state.page = page?.id ?? 'model';
  return state;
}
