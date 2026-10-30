import { invoke } from '@tauri-apps/api/core';

import type { OllamaInfo } from '$lib/ollama';
import type { StatusView } from '$lib/status';

/** Sama dengan `HotkeyStatus` di `app/src/hotkey.rs`. */
export type HotkeyStatus = 'active' | 'taken' | 'failed';

/** Sama dengan `HotkeyFailure` di `app/src/settings.rs`. */
export type HotkeyFailure = 'invalid' | 'taken' | 'failed' | 'notSaved';

/** Bentuknya sama dengan `SettingsView` di `app/src/settings.rs`. */
export interface SettingsView {
  /** Model pilihan pengguna; `null` berarti dipilih otomatis sesuai kemampuan PC. */
  model: string | null;
  /** Pintasan Tanya Nuenly yang sedang dipakai, misalnya `Ctrl+Space`. */
  hotkey: string;
  hotkeyStatus: HotkeyStatus;
  ollamaAddress: string;
  /** Aplikasi yang tidak pernah dibaca, apa pun modenya. */
  blockedApps: string[];
}

/** Aksi jendela Pengaturan ke Rust; dipisah agar mode demo di browser bisa menggantinya. */
export interface SettingsBackend {
  setModel(model: string | null): Promise<SettingsView>;
  /** `null` kembali ke Ctrl+Space. Gagal dengan kode `HotkeyFailure`. */
  setHotkey(shortcut: string | null): Promise<SettingsView>;
  checkOllama(): Promise<OllamaInfo>;
  openDownload(): Promise<void>;
  setMode(id: string): Promise<StatusView>;
  pause(): Promise<StatusView>;
  resume(): Promise<StatusView>;
}

export const tauriBackend: SettingsBackend = {
  setModel: (model) => invoke('settings_set_model', { model }),
  setHotkey: (shortcut) => invoke('settings_set_hotkey', { shortcut }),
  checkOllama: () => invoke('ollama_status'),
  openDownload: () => invoke('ollama_open_download'),
  setMode: (id) => invoke('status_set_mode', { id }),
  pause: () => invoke('status_pause'),
  resume: () => invoke('status_resume'),
};
