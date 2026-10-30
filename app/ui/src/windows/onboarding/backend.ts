import { invoke } from '@tauri-apps/api/core';

import type { OllamaInfo, OllamaStatus } from '$lib/ollama';

export type { OllamaStatus };

/** Aksi onboarding ke Rust; dipisah agar mode demo di browser bisa menggantinya. */
export interface OnboardingBackend {
  /** Menyimpan persetujuan privasi. Sebelum ini Nuenly tidak membaca apa pun. */
  accept(): Promise<void>;
  checkOllama(): Promise<OllamaStatus>;
  openDownload(): Promise<void>;
  /** Menutup onboarding; Nuenly lanjut berjalan di tray. */
  finish(): Promise<void>;
}

export const tauriBackend: OnboardingBackend = {
  accept: () => invoke('onboarding_accept'),
  checkOllama: async () => (await invoke<OllamaInfo>('ollama_status')).status,
  openDownload: () => invoke('ollama_open_download'),
  finish: () => invoke('onboarding_finish'),
};
