import { describe, expect, it } from 'vitest';

import { failureMessage } from './tauri';

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
