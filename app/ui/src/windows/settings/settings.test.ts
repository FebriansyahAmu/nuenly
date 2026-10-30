import { describe, expect, it } from 'vitest';

import type { OllamaInfo } from '$lib/ollama';
import type { StatusView } from '$lib/status';

import type { SettingsBackend, SettingsView } from './backend';
import { modelOptions, modelState, SettingsState } from './settings.svelte';

const SETTINGS: SettingsView = {
  model: null,
  hotkey: 'Ctrl+Space',
  ollamaAddress: '127.0.0.1:11434',
  blockedApps: ['nuenly.exe'],
};
const STATUS: StatusView = {
  modes: [{ id: 'all', label: 'Umum', accent: '#4f46e5' }],
  activeMode: 'all',
  pausedUntilMs: null,
};
const running = (models: string[]): OllamaInfo => ({ status: 'running', version: '0.12.6', models });

function backend(overrides: Partial<SettingsBackend> = {}): SettingsBackend {
  return {
    setModel: (model) => Promise.resolve({ ...SETTINGS, model }),
    checkOllama: () => Promise.resolve(running(['gpt-oss:20b'])),
    openDownload: () => Promise.resolve(),
    setMode: (id) => Promise.resolve({ ...STATUS, activeMode: id }),
    pause: () => Promise.resolve({ ...STATUS, pausedUntilMs: 1 }),
    resume: () => Promise.resolve(STATUS),
    ...overrides,
  };
}

describe('modelState', () => {
  it('siap bila model pilihan atau pilihan otomatis tersedia', () => {
    expect(modelState(running(['gpt-oss:20b']), 'gpt-oss:20b')).toBe('ready');
    expect(modelState(running(['gpt-oss:20b']), null)).toBe('ready');
  });

  it('menjelaskan kenapa model belum siap', () => {
    expect(modelState(null, null)).toBe('checking');
    expect(modelState({ status: 'missing', version: null, models: [] }, null)).toBe('not-installed');
    expect(modelState({ status: 'installed', version: null, models: [] }, null)).toBe('not-running');
    expect(modelState(running([]), null)).toBe('no-models');
    expect(modelState(running(['gemma3:4b']), 'gpt-oss:20b')).toBe('model-missing');
  });
});

describe('modelOptions', () => {
  it('tetap menampilkan pilihan tersimpan yang tidak ada di Ollama', () => {
    expect(modelOptions(running(['gemma3:4b']), 'gpt-oss:20b')).toEqual(['gpt-oss:20b', 'gemma3:4b']);
    expect(modelOptions(null, null)).toEqual([]);
  });
});

describe('SettingsState', () => {
  it('menyimpan pilihan model', async () => {
    const state = new SettingsState(backend(), SETTINGS, STATUS);

    await state.chooseModel('gpt-oss:20b');

    expect(state.settings.model).toBe('gpt-oss:20b');
    expect(state.error).toBeNull();
  });

  it('tidak membocorkan pesan error teknis', async () => {
    const state = new SettingsState(
      backend({ setModel: () => Promise.reject(new Error('os error 5: C:\\Users\\x\\config.json')) }),
      SETTINGS,
      STATUS,
    );

    await state.chooseModel('gpt-oss:20b');

    expect(state.settings.model).toBeNull();
    expect(state.error).toBe('Pilihan model tidak bisa disimpan. Coba lagi.');
  });

  it('menjeda lalu melanjutkan', async () => {
    const state = new SettingsState(backend(), SETTINGS, STATUS);

    await state.togglePause();
    expect(state.status.pausedUntilMs).toBe(1);

    await state.togglePause();
    expect(state.status.pausedUntilMs).toBeNull();
  });

  it('menganggap Ollama tidak ada bila pemeriksaan gagal', async () => {
    const state = new SettingsState(backend({ checkOllama: () => Promise.reject(new Error('x')) }), SETTINGS, STATUS);

    await state.checkOllama();

    expect(state.modelState).toBe('not-installed');
    expect(state.checking).toBe(false);
  });

  it('mengabaikan hasil pemeriksaan lama', async () => {
    const pending: ((info: OllamaInfo) => void)[] = [];
    const state = new SettingsState(
      backend({ checkOllama: () => new Promise((resolve) => pending.push(resolve)) }),
      SETTINGS,
      STATUS,
    );

    const first = state.checkOllama();
    const second = state.checkOllama();
    pending[1]?.(running(['gpt-oss:20b']));
    pending[0]?.({ status: 'missing', version: null, models: [] });
    await Promise.all([first, second]);

    expect(state.modelState).toBe('ready');
  });
});
