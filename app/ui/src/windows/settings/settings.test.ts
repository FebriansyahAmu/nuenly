import { describe, expect, it } from 'vitest';

import type { OllamaInfo, OllamaStatus } from '$lib/ollama';
import type { StatusView } from '$lib/status';

import type { SettingsBackend, SettingsView } from './backend';
import { formatMemory, modelOptions, modelState, SettingsState } from './settings.svelte';

const SETTINGS: SettingsView = {
  model: null,
  hotkey: 'Ctrl+Space',
  hotkeyStatus: 'active',
  ollamaAddress: '127.0.0.1:11434',
  blockedApps: ['nuenly.exe'],
};
const STATUS: StatusView = {
  modes: [{ id: 'all', label: 'Umum', accent: '#4f46e5' }],
  activeMode: 'all',
  pausedUntilMs: null,
};
const info = (status: OllamaStatus, models: string[] = []): OllamaInfo => ({
  status,
  version: status === 'running' ? '0.12.6' : null,
  models,
  plan: null,
  hardware: { ramBytes: 0, vramBytes: 0, gpu: null },
});
const running = (models: string[]) => info('running', models);

function backend(overrides: Partial<SettingsBackend> = {}): SettingsBackend {
  return {
    setModel: (model) => Promise.resolve({ ...SETTINGS, model }),
    setHotkey: (shortcut) => Promise.resolve({ ...SETTINGS, hotkey: shortcut ?? 'Ctrl+Space' }),
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
    expect(modelState(info('missing'), null)).toBe('not-installed');
    expect(modelState(info('installed'), null)).toBe('not-running');
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

describe('formatMemory', () => {
  it('menulis ukuran dalam GB dengan koma desimal', () => {
    expect(formatMemory(8_283_000_000)).toBe('7,7 GB');
    expect(formatMemory(6 * 1024 ** 3)).toBe('6 GB');
  });
});

describe('SettingsState', () => {
  it('menyimpan pilihan model lalu menghitung ulang rencananya', async () => {
    let checks = 0;
    const state = new SettingsState(
      backend({
        checkOllama: () => {
          checks += 1;
          return Promise.resolve(running(['gpt-oss:20b']));
        },
      }),
      SETTINGS,
      STATUS,
    );

    await state.chooseModel('gpt-oss:20b');

    expect(state.settings.model).toBe('gpt-oss:20b');
    expect(state.error).toBeNull();
    expect(checks).toBe(1);
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

  it('mengganti pintasan dan kembali ke bawaan', async () => {
    const state = new SettingsState(backend(), SETTINGS, STATUS);

    await state.setHotkey('Alt+Space');
    expect(state.settings.hotkey).toBe('Alt+Space');
    expect(state.hotkeyError).toBeNull();

    await state.setHotkey(null);
    expect(state.settings.hotkey).toBe('Ctrl+Space');
  });

  it('menjelaskan pintasan yang ditolak tanpa detail teknis', async () => {
    const failing = (error: unknown) =>
      new SettingsState(backend({ setHotkey: () => Promise.reject(error) }), SETTINGS, STATUS);

    const taken = failing('taken');
    await taken.setHotkey('Alt+Space');
    expect(taken.settings.hotkey).toBe('Ctrl+Space');
    expect(taken.hotkeyError).toBe('Kombinasi ini sudah dipakai aplikasi lain. Pintasan lama tetap dipakai.');

    const unknown = failing('Command settings_set_hotkey not allowed');
    await unknown.setHotkey('Alt+Space');
    expect(unknown.hotkeyError).toBe('Perubahan tidak bisa disimpan. Coba lagi.');
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
    pending[0]?.(info('missing'));
    await Promise.all([first, second]);

    expect(state.modelState).toBe('ready');
  });
});
