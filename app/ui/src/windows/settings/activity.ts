// Riwayat aktivitas di Pengaturan › Privasi: catatan audit dari Rust menjadi baris yang bisa
// dibaca orang. Catatan tidak pernah memuat teks pengguna, jadi yang tampil hanya keterangan.

import type { ModeInfo } from '$lib/status';

import type { AskOutcome, AuditEvent, AuditItem } from './backend';

/** Jumlah catatan terbaru yang dikirim Rust; sama dengan `LIMIT` di `app/src/audit.rs`. */
export const ACTIVITY_LIMIT = 50;
/** Jumlah catatan yang tampil sebelum "Tampilkan semua". */
export const ACTIVITY_PREVIEW = 10;

const TITLE: Record<AuditEvent['kind'], string> = { ask: 'Tanya Nuenly' };

const OUTCOME: Record<AskOutcome, string> = {
  answered: 'Terjawab',
  stopped: 'Dihentikan',
  failed: 'Gagal',
};

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agu', 'Sep', 'Okt', 'Nov', 'Des'];
const DAY_MS = 24 * 60 * 60 * 1000;
const numberFormat = new Intl.NumberFormat('id-ID');

export interface ActivityRow {
  title: string;
  /** Aplikasi, mode, model, dan banyaknya teks layar, dipisah titik tengah. */
  detail: string;
  outcome: string;
  failed: boolean;
}

export function describeActivity(item: AuditItem, modes: readonly ModeInfo[]): ActivityRow {
  const mode = modes.find((candidate) => candidate.id === item.mode)?.label ?? item.mode;
  const context =
    item.contextChars === 0 ? 'tanpa teks layar' : `${numberFormat.format(item.contextChars)} karakter dari layar`;
  return {
    title: TITLE[item.kind],
    detail: [item.app, mode, item.model, context].filter((part) => part !== null).join(' · '),
    outcome: OUTCOME[item.outcome],
    failed: item.outcome === 'failed',
  };
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
