/** Isi event `toast:show` dari Rust (`app/src/toast.rs`). */
export type ToastPayload = {
  id: string;
  /** `null`: tetap tampil sampai ditutup pengguna, misalnya saran penting di Medical. */
  durationMs: number | null;
} & (
  | {
      kind: 'suggestion';
      title: string;
      body: string;
      /** Baris peringatan merah di bawah deskripsi. */
      warning: string | null;
      /** Catatan kecil dari mode pack, misalnya "Saran, bukan diagnosis". */
      note: string | null;
      /** Rincian yang muncul setelah "Lihat". */
      points: string[];
      source: string | null;
    }
  | { kind: 'success'; message: string }
  | { kind: 'thinking'; message: string }
  | { kind: 'permission'; title: string; body: string }
);
