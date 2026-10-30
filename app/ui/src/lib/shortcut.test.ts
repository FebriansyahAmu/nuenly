import { describe, expect, it } from 'vitest';

import { captureShortcut } from './shortcut';

const keys = (code: string, held: Partial<Record<'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', boolean>> = {}) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...held,
});

describe('captureShortcut', () => {
  it('membaca kombinasi lengkap dengan urutan tetap', () => {
    expect(captureShortcut(keys('Space', { altKey: true }))).toEqual({ kind: 'complete', shortcut: 'Alt+Space' });
    expect(captureShortcut(keys('KeyK', { shiftKey: true, ctrlKey: true }))).toEqual({
      kind: 'complete',
      shortcut: 'Ctrl+Shift+K',
    });
    expect(captureShortcut(keys('F24', { ctrlKey: true, altKey: true, metaKey: true }))).toEqual({
      kind: 'complete',
      shortcut: 'Ctrl+Alt+Win+F24',
    });
    expect(captureShortcut(keys('Digit7', { altKey: true }))).toEqual({ kind: 'complete', shortcut: 'Alt+7' });
  });

  it('menunggu tombol utama selama baru pengubah yang ditekan', () => {
    expect(captureShortcut(keys('ControlLeft', { ctrlKey: true }))).toEqual({ kind: 'partial', text: 'Ctrl+…' });
    expect(captureShortcut(keys('AltRight', { altKey: true, shiftKey: true }))).toEqual({
      kind: 'partial',
      text: 'Alt+Shift+…',
    });
  });

  it('menolak kombinasi yang menangkap ketikan biasa atau tombol yang tidak didukung', () => {
    for (const capture of [
      keys('KeyK'),
      keys('KeyK', { shiftKey: true }),
      keys('KeyK', { metaKey: true }),
      keys('Enter', { ctrlKey: true }),
      keys('F25', { ctrlKey: true }),
      keys('Numpad1', { altKey: true }),
    ]) {
      expect(captureShortcut(capture)).toEqual({ kind: 'invalid' });
    }
  });
});
