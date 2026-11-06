import { describe, expect, it } from 'vitest';

import { describeActivity, formatWhen } from './activity';
import type { AuditItem } from './backend';

const MODES = [
  { id: 'all', label: 'Umum', accent: '#4f46e5' },
  { id: 'medical', label: 'Medical', accent: '#0f766e' },
];

type AskItem = Extract<AuditItem, { kind: 'ask' }>;

const item = (change: Partial<AskItem> = {}): AskItem => ({
  kind: 'ask',
  atMs: 0,
  app: 'notepad.exe',
  mode: 'all',
  model: 'gemma3:4b',
  contextChars: 1200,
  outcome: 'answered',
  ...change,
});

describe('describeActivity', () => {
  it('menyebut aplikasi, mode, model, dan banyaknya teks layar', () => {
    expect(describeActivity(item(), MODES)).toEqual({
      title: 'Tanya Nuenly',
      detail: 'notepad.exe · Umum · gemma3:4b · 1.200 karakter dari layar',
      outcome: 'Terjawab',
      failed: false,
    });
  });

  it('melewati aplikasi yang tidak diketahui dan menandai jawaban gagal', () => {
    const row = describeActivity(item({ app: null, mode: 'medical', contextChars: 0, outcome: 'failed' }), MODES);
    expect(row.detail).toBe('Medical · gemma3:4b · tanpa teks layar');
    expect(row.outcome).toBe('Gagal');
    expect(row.failed).toBe(true);
  });

  it('memakai ID mode bila modenya tidak dikenal lagi', () => {
    expect(describeActivity(item({ mode: 'lama', outcome: 'stopped' }), MODES)).toMatchObject({
      detail: 'notepad.exe · lama · gemma3:4b · 1.200 karakter dari layar',
      outcome: 'Dihentikan',
    });
  });

  it('menjelaskan pemeriksaan saran beserta hasilnya', () => {
    const check: AuditItem = { ...item(), kind: 'suggest', contextChars: 300, outcome: 'nothing' };
    expect(describeActivity(check, MODES)).toEqual({
      title: 'Saran',
      detail: 'notepad.exe · Umum · gemma3:4b · 300 karakter dari layar',
      outcome: 'Tidak ada saran',
      failed: false,
    });
    expect(describeActivity({ ...check, outcome: 'dropped' }, MODES).outcome).toBe('Dibuang');
    expect(describeActivity({ ...check, outcome: 'held' }, MODES).outcome).toBe('Ditahan');
    expect(describeActivity({ ...check, outcome: 'failed' }, MODES).failed).toBe(true);
  });

  it('menyebut tanggapan saran dan lamanya sejak saran tampil', () => {
    const reaction = (change: { reaction: 'disliked' | 'copied' | 'expired'; afterMs: number }): AuditItem => ({
      kind: 'reaction',
      atMs: 0,
      app: 'winword.exe',
      mode: 'all',
      ...change,
    });
    expect(describeActivity(reaction({ reaction: 'disliked', afterMs: 1600 }), MODES)).toEqual({
      title: 'Tanggapan saran',
      detail: 'winword.exe · Umum · setelah 2 detik',
      outcome: 'Tidak membantu',
      failed: false,
    });
    expect(describeActivity(reaction({ reaction: 'copied', afterMs: 400 }), MODES).detail).toBe(
      'winword.exe · Umum · setelah kurang dari 1 detik',
    );
    expect(describeActivity(reaction({ reaction: 'expired', afterMs: 150_000 }), MODES)).toMatchObject({
      detail: 'winword.exe · Umum · setelah 3 menit',
      outcome: 'Hilang sendiri',
    });
  });
});

describe('formatWhen', () => {
  const now = new Date(2026, 10, 5, 16, 30);

  it('menulis hari ini dan kemarin dengan jam', () => {
    expect(formatWhen(new Date(2026, 10, 5, 14, 57).getTime(), now)).toBe('Hari ini 14.57');
    expect(formatWhen(new Date(2026, 10, 4, 9, 5).getTime(), now)).toBe('Kemarin 09.05');
  });

  it('menulis tanggal untuk hari yang lebih lama', () => {
    expect(formatWhen(new Date(2026, 10, 3, 8, 0).getTime(), now)).toBe('3 Nov 08.00');
    expect(formatWhen(new Date(2026, 9, 7, 23, 59).getTime(), now)).toBe('7 Okt 23.59');
  });
});
