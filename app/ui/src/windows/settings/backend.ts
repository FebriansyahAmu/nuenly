import { invoke } from '@tauri-apps/api/core';

import type { OllamaInfo } from '$lib/ollama';
import type { StatusView } from '$lib/status';

/** Bentuknya sama dengan `SettingsView` di `app/src/settings.rs`. */
export interface SettingsView {
  /** Model pilihan pengguna; `null` berarti dipilih otomatis sesuai kemampuan PC. */
  model: string | null;
  hotkey: string;
  ollamaAddress: string;
  /** Aplikasi yang tidak pernah dibaca, apa pun modenya. */
  blockedApps: string[];
}

/** Aksi jendela Pengaturan ke Rust; dipisah agar mode demo di browser bisa menggantinya. */
export interface SettingsBackend {
  setModel(model: string | null): Promise<SettingsView>;
  checkOllama(): Promise<OllamaInfo>;
  openDownload(): Promise<void>;
  setMode(id: string): Promise<StatusView>;
  pause(): Promise<StatusView>;
  resume(): Promise<StatusView>;
}

export const tauriBackend: SettingsBackend = {
  setModel: (model) => invoke('settings_set_model', { model }),
  checkOllama: () => invoke('ollama_status'),
  openDownload: () => invoke('ollama_open_download'),
  setMode: (id) => invoke('status_set_mode', { id }),
  pause: () => invoke('status_pause'),
  resume: () => invoke('status_resume'),
};
