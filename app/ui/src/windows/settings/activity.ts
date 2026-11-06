// Riwayat aktivitas di Pengaturan › Privasi: catatan audit dari Rust menjadi baris yang bisa
// dibaca orang. Catatan tidak pernah memuat teks pengguna, jadi yang tampil hanya keterangan.

import type { ModeInfo } from '$lib/status';

import type { AskOutcome, AuditEvent, AuditItem, Reaction, SuggestOutcome } from './backend';

/** Jumlah catatan terbaru yang dikirim Rust; sama dengan `LIMIT` di `app/src/audit.rs`. */
export const ACTIVITY_LIMIT = 50;
/** Jumlah catatan yang tampil sebelum "Tampilkan semua". */
export const ACTIVITY_PREVIEW = 10;

const TITLE: Record<AuditEvent['kind'], string> = {
  ask: 'Tanya Nuenly',
  suggest: 'Saran',
  reaction: 'Tanggapan saran',
};

const ASK_OUTCOME: Record<AskOutcome, string> = {
  answered: 'Terjawab',
  stopped: 'Dihentikan',
  failed: 'Gagal',
};

const SUGGEST_OUTCOME: Record<SuggestOutcome, string> = {
  shown: 'Tampil',
  nothing: 'Tidak ada saran',
  dropped: 'Dibuang',
  failed: 'Gagal',
};

const REACTION: Record<Reaction, string> = {
  copied: 'Poin disalin',
  viewed: 'Dilihat',
  disliked: 'Tidak membantu',
  dismissed: 'Diabaikan',
  closed: 'Ditutup',
  expired: 'Hilang sendiri',
};

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agu', 'Sep', 'Okt', 'Nov', 'Des'];
const DAY_MS = 24 * 60 * 60 * 1000;
const numberFormat = new Intl.NumberFormat('id-ID');

export interface ActivityRow {
  title: string;
  /**
   * Aplikasi dan mode, lalu model dan banyaknya teks layar (atau lama sampai saran ditanggapi),
   * dipisah titik tengah.
   */
  detail: string;
  outcome: string;
  failed: boolean;
}

export function describeActivity(item: AuditItem, modes: readonly ModeInfo[]): ActivityRow {
  const mode = modes.find((candidate) => candidate.id === item.mode)?.label ?? item.mode;
  const row = (parts: string[], outcome: string, failed = false): ActivityRow => ({
    title: TITLE[item.kind],
    detail: [item.app, mode, ...parts].filter((part) => part !== null).join(' · '),
    outcome,
    failed,
  });
  switch (item.kind) {
    case 'ask':
      return row([item.model, context(item.contextChars)], ASK_OUTCOME[item.outcome], item.outcome === 'failed');
    case 'suggest':
      return row([item.model, context(item.contextChars)], SUGGEST_OUTCOME[item.outcome], item.outcome === 'failed');
    case 'reaction':
      return row([after(item.afterMs)], REACTION[item.reaction]);
  }
}

function context(chars: number): string {
  return chars === 0 ? 'tanpa teks layar' : `${numberFormat.format(chars)} karakter dari layar`;
}

/** Lama sejak saran tampil sampai ditanggapi, dibulatkan ke detik atau menit. */
function after(ms: number): string {
  const seconds = Math.round(ms / 1000);
  if (seconds < 1) {
    return 'setelah kurang dari 1 detik';
  }
  if (seconds < 60) {
    return `setelah ${seconds} detik`;
  }
  return `setelah ${Math.round(seconds / 60)} menit`;
}

/** Waktu catatan dalam zona waktu PC: "Hari ini 14.57", "Kemarin 09.05", atau "3 Nov 14.57". */
export function formatWhen(atMs: number, now: Date): string {
  const at = new Date(atMs);
  const pad = (value: number) => String(value).padStart(2, '0');
  const time = `${pad(at.getHours())}.${pad(at.getMinutes())}`;
  const midnight = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  // Dibulatkan karena hari peralihan jam musim panas tidak tepat 24 jam.
  const days = Math.round((midnight(now) - midnight(at)) / DAY_MS);
  if (days === 0) {
    return `Hari ini ${time}`;
  }
  if (days === 1) {
    return `Kemarin ${time}`;
  }
  return `${at.getDate()} ${MONTHS[at.getMonth()]} ${time}`;
}
