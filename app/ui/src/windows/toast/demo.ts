// Contoh toast untuk melihat tampilan di browser saat `pnpm dev`. Hanya dimuat di build dev.
// Teksnya rekaan dari kanvas desain 02; tidak ada data pasien nyata.

import type { ToastPayload } from './types';

const POINTS = ['Sejak kapan nyeri dimulai, berapa lama', 'Riwayat alergi obat', 'Merokok, diabetes, hipertensi'];

export const DEMO_TOASTS: Record<string, ToastPayload> = {
  saran: {
    id: 'saran',
    durationMs: 8000,
    kind: 'suggestion',
    title: 'Email ini bisa lebih singkat',
    body: 'Dua kalimat terakhir bisa digabung.',
    warning: null,
    note: null,
    points: ['Gabungkan dua kalimat terakhir menjadi satu ajakan.'],
    source: null,
  },
  penting: {
    id: 'penting',
    durationMs: null,
    kind: 'suggestion',
    title: '3 hal belum tercatat',
    body: 'Durasi nyeri, alergi obat, dan faktor risiko.',
    warning: 'Pertimbangkan EKG segera.',
    note: 'Saran, bukan diagnosis',
    points: POINTS,
    source: 'Sumber: [PANDUAN KLINIS] · diproses di PC ini',
  },
  berhasil: {
    id: 'berhasil',
    durationMs: 3000,
    kind: 'success',
    message: 'Poin disalin. Tempel dengan Ctrl V.',
  },
  berpikir: {
    id: 'berpikir',
    durationMs: null,
    kind: 'thinking',
    message: 'Membaca field anamnesis…',
  },
  izin: {
    id: 'izin',
    durationMs: null,
    kind: 'permission',
    title: 'Boleh cari kode ICD-10?',
    body: 'Hanya keluhan yang dikirim, tanpa data pasien.',
  },
};
