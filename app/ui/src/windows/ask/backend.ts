/** Asal konteks untuk chip "Dari:". Teksnya sendiri tetap di Rust dan tidak dikirim ke jendela. */
export interface AskContext {
  source: 'selection' | 'field';
  /** Nama aplikasi untuk pengguna, misalnya "Mail". */
  appName: string;
}

/** Alasan teks terpilih tidak dipakai; sama dengan `Notice` di `app/src/ask.rs`. */
export type AskNotice = 'paused' | 'notAllowed' | 'protectedField' | 'unreadable';

/** Isi event `ask:open`; sama dengan `AskOpen` di `app/src/ask.rs`. */
export interface AskOpen {
  context: AskContext | null;
  notice: AskNotice | null;
}

export interface AskRequest {
  prompt: string;
  /** `false` bila pengguna menghapus chip konteks. */
  useContext: boolean;
}

/** Jalur ke model. Implementasi Tauri + Ollama menyusul di langkah 6–7 (F-10). */
export interface AskBackend {
  /** Mengirim permintaan dan memanggil `onToken` untuk setiap potongan jawaban. */
  ask(request: AskRequest, onToken: (token: string) => void): Promise<void>;
}

/** Error yang pesannya aman dan ramah untuk ditampilkan ke pengguna. */
export class AskError extends Error {}

/** Dipakai selama Nuenly belum tersambung ke model. */
export const unavailableBackend: AskBackend = {
  ask() {
    return Promise.reject(new AskError('Nuenly belum tersambung ke model AI.'));
  },
};
