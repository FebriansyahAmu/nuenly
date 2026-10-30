// Mode demo Pengaturan untuk melihat tampilan di browser saat `pnpm dev`. Hanya dimuat di build dev.

import type { OllamaInfo } from '$lib/ollama';
import type { StatusView } from '$lib/status';

import type { SettingsBackend, SettingsView } from './backend';
import { PAGES, SettingsState } from './settings.svelte';

const MODES = [
  { id: 'all', label: 'Umum', accent: '#4f46e5' },
  { id: 'medical', label: 'Medical', accent: '#0f766e' },
];

const OLLAMA: Record<string, OllamaInfo> = {
  running: { status: 'running', version: '0.12.6', models: ['gemma3:4b', 'gpt-oss:20b'] },
  kosong: { status: 'running', version: '0.12.6', models: [] },
  installed: { status: 'installed', version: null, models: [] },
  missing: { status: 'missing', version: null, models: [] },
};

/**
 * `?halaman=general|model|privacy|connector` (bawaannya model, seperti desain 07),
 * `?ollama=running|kosong|installed|missing`, dan `?jeda`.
 */
export function demoSettings(params: URLSearchParams): SettingsState {
  let settings: SettingsView = {
    model: 'gpt-oss:20b',
    hotkey: 'Ctrl+Space',
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
    checkOllama: () => Promise.resolve(ollama as OllamaInfo),
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
