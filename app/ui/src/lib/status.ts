import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/** Bentuknya sama dengan `ModeInfo` di `app/src/status.rs`. */
export interface ModeInfo {
  id: string;
  label: string;
  accent: string;
}

/** Bentuknya sama dengan `StatusView` di `app/src/status.rs`. */
export interface StatusView {
  modes: ModeInfo[];
  activeMode: string;
  /** Akhir jeda (ms sejak epoch Unix), atau `null` bila tidak dijeda. */
  pausedUntilMs: number | null;
}

export function activeModeOf(status: StatusView): ModeInfo | undefined {
  return status.modes.find((mode) => mode.id === status.activeMode);
}

/** Memasang warna aksen mode aktif ke `--mode-accent` di dokumen ini. */
export function applyAccent(status: StatusView): void {
  const mode = activeModeOf(status);
  if (mode) {
    document.documentElement.style.setProperty('--mode-accent', mode.accent);
  }
}

/**
 * Mengambil status dari Rust lalu mengikuti event `status:changed`. Aksen mode selalu dipasang;
 * `onChange` dipanggil untuk setiap status baru. Mengembalikan fungsi untuk berhenti.
 */
export async function watchStatus(onChange?: (status: StatusView) => void): Promise<() => void> {
  const update = (status: StatusView) => {
    applyAccent(status);
    onChange?.(status);
  };
  update(await invoke<StatusView>('status_get'));
  return listen<StatusView>('status:changed', ({ payload }) => update(payload));
}
