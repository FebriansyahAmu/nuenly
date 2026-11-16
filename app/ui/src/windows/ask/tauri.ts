import { Channel, invoke } from '@tauri-apps/api/core';

import { OLLAMA_EXPOSED } from '$lib/ollama';

import { type AskBackend, AskError, type AskReply } from './backend';

/** Sama dengan `AnswerEvent` di `app/src/answer.rs`. */
type AnswerEvent = { kind: 'token'; text: string } | { kind: 'end' };

/** Sama dengan `AskFailure` di `app/src/answer.rs`. */
export type AskFailure =
  | 'ollamaNotRunning'
  | 'ollamaExposed'
  | 'noModel'
  | 'modelMissing'
  | 'modelTooLarge'
  | 'modelLoad'
  | 'tooLong'
  | 'timeout'
  | 'cancelled'
  | 'failed';

const FAILURE: Record<AskFailure, string> = {
  ollamaNotRunning: 'Ollama belum berjalan. Buka Ollama dari menu Start, lalu coba lagi.',
  ollamaExposed: `${OLLAMA_EXPOSED} Hapus pengaturan OLLAMA_HOST di Windows, buka ulang Ollama, lalu coba lagi.`,
  noModel: 'Belum ada model di Ollama. Caranya ada di Pengaturan › Model AI.',
  modelMissing: 'Model pilihan Anda tidak ada di Ollama. Pilih model lain di Pengaturan › Model AI.',
  modelTooLarge: 'Model ini terlalu berat untuk PC ini. Pilih model yang lebih kecil di Pengaturan › Model AI.',
  modelLoad:
    'Model tidak bisa dimuat karena memori GPU atau RAM sedang tidak cukup. Tutup aplikasi yang berat, atau pilih model yang lebih kecil di Pengaturan › Model AI.',
  tooLong: 'Perintahnya terlalu panjang. Coba persingkat.',
  timeout: 'Model terlalu lama menjawab. Coba lagi.',
  cancelled: 'Jawaban dihentikan.',
  failed: 'Nuenly tidak bisa menjawab sekarang. Coba lagi sebentar lagi.',
};

/** Pesan untuk pengguna dari kode gagal `ask_send`, atau `null` bila bukan kode yang dikenal. */
export function failureMessage(error: unknown): string | null {
  return typeof error === 'string' && Object.hasOwn(FAILURE, error) ? FAILURE[error as AskFailure] : null;
}

/** Sama dengan `ReplaceFailure` di `app/src/replace.rs`. */
export type ReplaceFailure = 'unavailable' | 'changed' | 'moved' | 'failed';

const REPLACE_FAILURE: Record<ReplaceFailure, string> = {
  unavailable: 'Teks ini tidak bisa diganti lagi. Salin jawabannya bila masih perlu.',
  changed: 'Teks di aplikasi sudah berubah sejak dibaca, jadi tidak diganti. Salin jawabannya bila masih perlu.',
  moved: 'Jendela asalnya tidak bisa dibuka lagi. Salin jawabannya, lalu tempel sendiri.',
  failed: 'Teks tidak bisa diganti. Salin jawabannya, lalu tempel sendiri.',
};

/** Pesan untuk pengguna dari kode gagal `ask_replace`; selain kode yang dikenal, pesan umum. */
export function replaceFailureMessage(error: unknown): string {
  return typeof error === 'string' && Object.hasOwn(REPLACE_FAILURE, error)
    ? REPLACE_FAILURE[error as ReplaceFailure]
    : REPLACE_FAILURE.failed;
}

/** Jawaban dari Ollama lewat command `ask_send`; token mengalir lewat `Channel`. */
export const tauriBackend: AskBackend = {
  async ask(request, onToken) {
    let ended = () => {};
    const end = new Promise<void>((resolve) => (ended = resolve));
    const channel = new Channel<AnswerEvent>((event) => {
      if (event.kind === 'token') {
        onToken(event.text);
      } else {
        ended();
      }
    });

    let reply: AskReply;
    try {
      reply = await invoke<AskReply>('ask_send', {
        prompt: request.prompt,
        useContext: request.useContext,
        channel,
      });
    } catch (error) {
      // Selain kode yang dikenal, error dibiarkan; sesi menampilkan pesan umum.
      const message = failureMessage(error);
      throw message === null ? error : new AskError(message);
    }
    // Balasan command bisa tiba lebih dulu dari token terakhir; tunggu penanda akhir channel.
    await end;
    return reply;
  },

  async replace() {
    try {
      await invoke('ask_replace');
    } catch (error) {
      throw new AskError(replaceFailureMessage(error));
    }
  },
};
