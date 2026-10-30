/** Asal konteks untuk chip "Dari:". Teksnya sendiri tetap di Rust dan tidak dikirim ke jendela. */
export interface AskContext {
  source: 'selection' | 'field';
  /** Nama aplikasi untuk pengguna, misalnya "Mail". */
  appName: string;
  /** Cuplikan satu baris (≤ 200 karakter) bila mode mengizinkan; `null` di mode lain. */
  preview: string | null;
}

/** Alasan teks terpilih tidak dipakai; sama dengan `Notice` di `app/src/ask.rs`. */
export type AskNotice = 'paused' | 'notAllowed' | 'privateWindow' | 'banking' | 'protectedField' | 'unreadable';

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

/** Sama dengan `AskReply` di `app/src/answer.rs`. */
export interface AskReply {
  /** Teks terpilih terlalu panjang, jadi hanya bagian awalnya yang dipakai. */
  contextTruncated: boolean;
}

/** Jalur ke model; di aplikasi lewat `tauriBackend` di `./tauri`, di mode demo jawaban contoh. */
export interface AskBackend {
  /** Mengirim permintaan dan memanggil `onToken` untuk setiap potongan jawaban. */
  ask(request: AskRequest, onToken: (token: string) => void): Promise<AskReply>;
}

/** Error yang pesannya aman dan ramah untuk ditampilkan ke pengguna. */
export class AskError extends Error {}
