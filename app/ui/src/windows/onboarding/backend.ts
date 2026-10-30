import { invoke } from '@tauri-apps/api/core';

/** Bentuknya sama dengan `OllamaStatus` di `app/src/ollama.rs`. */
export type OllamaStatus = 'running' | 'installed' | 'missing';

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
  checkOllama: () => invoke('onboarding_ollama'),
  openDownload: () => invoke('onboarding_open_download'),
  finish: () => invoke('onboarding_finish'),
};
