import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

import type { StatusView } from '$lib/status';

/** Aksi popup tray ke Rust; dipisah agar mode demo di browser bisa menggantinya. */
export interface TrayBackend {
  setMode(id: string): Promise<StatusView>;
  pause(): Promise<StatusView>;
  resume(): Promise<StatusView>;
  openSettings(): Promise<void>;
  quit(): Promise<void>;
  close(): Promise<void>;
}

export const tauriBackend: TrayBackend = {
  setMode: (id) => invoke('status_set_mode', { id }),
  pause: () => invoke('status_pause'),
  resume: () => invoke('status_resume'),
  openSettings: () => invoke('open_settings'),
  quit: () => invoke('app_quit'),
  close: () => getCurrentWindow().hide(),
};
