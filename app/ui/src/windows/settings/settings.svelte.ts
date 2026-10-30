import type { OllamaInfo } from '$lib/ollama';
import type { StatusView } from '$lib/status';

import type { HotkeyFailure, SettingsBackend, SettingsView } from './backend';

export const PAGES = [
  { id: 'general', label: 'Umum', subtitle: 'Mode dan pintasan keyboard.' },
  { id: 'model', label: 'Model AI', subtitle: 'Otak yang dipakai Nuenly untuk memberi saran.' },
  { id: 'privacy', label: 'Privasi', subtitle: 'Apa yang dibaca Nuenly, dan kapan.' },
  { id: 'connector', label: 'Connector', subtitle: 'Hubungkan Nuenly dengan layanan lain, selalu dengan izin Anda.' },
] as const;
export type PageId = (typeof PAGES)[number]['id'];

/** Kesiapan model yang dipakai, untuk penanda di kartu "Sedang dipakai". */
export type ModelState = 'checking' | 'ready' | 'no-models' | 'model-missing' | 'not-running' | 'not-installed';

export function modelState(ollama: OllamaInfo | null, model: string | null): ModelState {
  if (ollama === null) {
    return 'checking';
  }
  switch (ollama.status) {
    case 'missing':
      return 'not-installed';
    case 'installed':
      return 'not-running';
    case 'running':
      if (ollama.models.length === 0) {
        return 'no-models';
      }
      return model === null || ollama.models.includes(model) ? 'ready' : 'model-missing';
  }
}

/** Pilihan model: yang sudah diunduh, ditambah pilihan tersimpan bila tidak ada di Ollama. */
export function modelOptions(ollama: OllamaInfo | null, model: string | null): string[] {
  const models = ollama?.models ?? [];
  return model === null || models.includes(model) ? models : [model, ...models];
}

const GB = 1024 ** 3;
const gbFormat = new Intl.NumberFormat('id-ID', { maximumFractionDigits: 1 });

/** Ukuran memori untuk dibaca orang, misalnya "7,7 GB". */
export function formatMemory(bytes: number): string {
  return `${gbFormat.format(bytes / GB)} GB`;
}

const ACTION_ERROR = 'Perubahan tidak bisa disimpan. Coba lagi.';

const HOTKEY_ERROR: Record<HotkeyFailure, string> = {
  invalid: 'Pakai Ctrl atau Alt bersama huruf, angka, F1–F24, atau Spasi.',
  taken: 'Kombinasi ini sudah dipakai aplikasi lain. Pintasan lama tetap dipakai.',
  failed: 'Kombinasi ini tidak bisa dipasang. Pintasan lama tetap dipakai.',
  notSaved: 'Pintasan sudah dipakai, tetapi tidak bisa disimpan untuk saat Nuenly dibuka lagi.',
};

/** Pesan untuk kode gagal dari `settings_set_hotkey`; selain kode yang dikenal, pesan umum. */
export function hotkeyError(error: unknown): string {
  return typeof error === 'string' && Object.hasOwn(HOTKEY_ERROR, error)
    ? HOTKEY_ERROR[error as HotkeyFailure]
    : ACTION_ERROR;
}

const MODEL_ERROR = 'Pilihan model tidak bisa disimpan. Coba lagi.';
const DOWNLOAD_ERROR = 'Halaman unduhan tidak bisa dibuka. Buka ollama.com/download di browser.';

/** Isi jendela Pengaturan: setelan, status mode/jeda, dan hasil pemeriksaan Ollama. */
export class SettingsState {
  page = $state<PageId>('general');
  settings = $state() as SettingsView;
  status = $state() as StatusView;
  ollama = $state<OllamaInfo | null>(null);
  checking = $state(false);
  error = $state<string | null>(null);
  /** Alasan pintasan terakhir tidak bisa dipakai; tampil di baris pintasan. */
  hotkeyError = $state<string | null>(null);
  savingHotkey = $state(false);

  #backend: SettingsBackend;
  /** Naik setiap pemeriksaan baru; hasil pemeriksaan lama diabaikan. */
  #check = 0;

  constructor(backend: SettingsBackend, settings: SettingsView, status: StatusView) {
    this.#backend = backend;
    this.settings = settings;
    this.status = status;
  }

  get modelState(): ModelState {
    return modelState(this.ollama, this.settings.model);
  }

  get modelOptions(): string[] {
    return modelOptions(this.ollama, this.settings.model);
  }

  async checkOllama(): Promise<void> {
    this.#check += 1;
    const check = this.#check;
    this.checking = true;
    let result: OllamaInfo;
    try {
      result = await this.#backend.checkOllama();
    } catch {
      result = {
        status: 'missing',
        version: null,
        models: [],
        plan: null,
        hardware: this.ollama?.hardware ?? { ramBytes: 0, vramBytes: 0, gpu: null },
      };
    }
    if (check === this.#check) {
      this.ollama = result;
      this.checking = false;
    }
  }

  /** `null` kembali ke pilihan otomatis. Rencana model dihitung ulang setelah tersimpan. */
  async chooseModel(model: string | null): Promise<void> {
    let saved = false;
    await this.#run(async () => {
      this.settings = await this.#backend.setModel(model);
      saved = true;
    }, MODEL_ERROR);
    if (saved) {
      await this.checkOllama();
    }
  }

  /** `null` kembali ke Ctrl+Space. Bila gagal, pintasan lama tetap dipakai dan alasannya tersimpan. */
  async setHotkey(shortcut: string | null): Promise<void> {
    this.hotkeyError = null;
    this.savingHotkey = true;
    try {
      this.settings = await this.#backend.setHotkey(shortcut);
    } catch (error) {
      this.hotkeyError = hotkeyError(error);
    } finally {
      this.savingHotkey = false;
    }
  }

  async setMode(id: string): Promise<void> {
    await this.#run(async () => (this.status = await this.#backend.setMode(id)), ACTION_ERROR);
  }

  async togglePause(): Promise<void> {
    const paused = this.status.pausedUntilMs !== null;
    await this.#run(
      async () => (this.status = await (paused ? this.#backend.resume() : this.#backend.pause())),
      ACTION_ERROR,
    );
  }

  async openDownload(): Promise<void> {
    await this.#run(() => this.#backend.openDownload(), DOWNLOAD_ERROR);
  }

  async #run(action: () => Promise<unknown>, message: string): Promise<void> {
    this.error = null;
    try {
      await action();
    } catch {
      // Pesan error dari Rust bisa memuat detail teknis; yang tampil selalu pesan tetap.
      this.error = message;
    }
  }
}
