// Mode demo Pengaturan untuk melihat tampilan di browser saat `pnpm dev`. Hanya dimuat di build dev.

import type { OllamaInfo } from '$lib/ollama';
import type { StatusView } from '$lib/status';

import type { AuditItem, SettingsBackend, SettingsView } from './backend';
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
  terbuka: { status: 'exposed', version: '0.40.13', models: [], plan: null, hardware: HARDWARE },
  installed: { status: 'installed', version: null, models: [], plan: null, hardware: HARDWARE },
  missing: { status: 'missing', version: null, models: [], plan: null, hardware: HARDWARE },
};

/** Riwayat sintetis: Tanya Nuenly di beberapa aplikasi, terbaru lebih dulu, tiap 47 menit. */
function demoActivity(): AuditItem[] {
  const apps = ['notepad.exe', 'winword.exe', 'chrome.exe', null, 'klinik-dummy.exe'];
  const outcomes = ['answered', 'answered', 'stopped', 'answered', 'failed'] as const;
  return Array.from({ length: 14 }, (_, index) => ({
    kind: 'ask' as const,
    atMs: Date.now() - index * 47 * 60 * 1000,
    app: apps[index % apps.length] ?? null,
    mode: index % apps.length === 4 ? 'medical' : 'all',
    model: 'gemma3:4b',
    contextChars: index % 3 === 0 ? 0 : 180 * index,
    outcome: outcomes[index % outcomes.length] ?? 'answered',
  }));
}

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
 * `?ollama=running|kosong|terbuka|installed|missing`, `?otomatis`, `?jeda`, `?tanpa-aplikasi`,
 * `?pintasan=taken|failed`, `?tombol=<pintasan>` (misalnya `Alt+Space`), dan
 * `?riwayat=kosong|gagal`.
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
    appName: params.has('tanpa-aplikasi') ? null : 'Notepad',
    pausedUntilMs: params.has('jeda') ? Date.now() + 15 * 60 * 1000 : null,
  };
  const ollama = OLLAMA[params.get('ollama') ?? 'running'] ?? OLLAMA.running;
  let activity = params.get('riwayat') === 'kosong' ? [] : demoActivity();
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
    listActivity: () => (params.get('riwayat') === 'gagal' ? Promise.reject('gagal') : Promise.resolve(activity)),
    clearActivity: () => Promise.resolve(void (activity = [])),
  };

  const state = new SettingsState(backend, settings, status);
  const page = PAGES.find((candidate) => candidate.id === params.get('halaman'));
  state.page = page?.id ?? 'model';
  return state;
}
