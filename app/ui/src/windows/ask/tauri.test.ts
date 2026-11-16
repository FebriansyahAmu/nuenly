import { describe, expect, it } from 'vitest';

import { failureMessage, replaceFailureMessage } from './tauri';

describe('failureMessage', () => {
  it('menerjemahkan kode gagal dari Rust', () => {
    expect(failureMessage('ollamaNotRunning')).toBe('Ollama belum berjalan. Buka Ollama dari menu Start, lalu coba lagi.');
    expect(failureMessage('cancelled')).toBe('Jawaban dihentikan.');
  });

  it('tidak menampilkan error lain apa adanya', () => {
    for (const error of ['toString', 'Command ask_send not allowed', new Error('11434'), undefined]) {
      expect(failureMessage(error)).toBeNull();
    }
  });
});

describe('replaceFailureMessage', () => {
  it('menerjemahkan kode gagal Ganti teks', () => {
    expect(replaceFailureMessage('changed')).toContain('sudah berubah sejak dibaca');
    expect(replaceFailureMessage('unavailable')).toContain('tidak bisa diganti lagi');
  });

  it('memakai pesan umum untuk error lain', () => {
    for (const error of ['toString', 'Command ask_replace not allowed', new Error('x'), undefined]) {
      expect(replaceFailureMessage(error)).toBe(replaceFailureMessage('failed'));
    }
  });
});
