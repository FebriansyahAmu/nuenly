import { describe, expect, it } from 'vitest';

import { ShortcutRecorder } from './recorder.svelte';

function key(code: string, key = '', held: Partial<Record<'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', boolean>> = {}) {
  const event = {
    code,
    key,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    prevented: false,
    preventDefault() {
      event.prevented = true;
    },
    stopPropagation() {},
    ...held,
  };
  return event;
}

describe('ShortcutRecorder', () => {
  it('mengabaikan tombol saat tidak merekam', () => {
    const recorder = new ShortcutRecorder();
    const event = key('Space', ' ', { altKey: true });
    expect(recorder.keydown(event)).toBeNull();
    expect(event.prevented).toBe(false);
  });

  it('menampilkan pengubah yang ditahan lalu mengembalikan kombinasi lengkap', () => {
    const recorder = new ShortcutRecorder();
    recorder.start();

    expect(recorder.keydown(key('AltLeft', 'Alt', { altKey: true }))).toBeNull();
    expect(recorder.held).toBe('Alt+…');

    expect(recorder.keydown(key('Space', ' ', { altKey: true }))).toBe('Alt+Space');
    expect(recorder.recording).toBe(false);
  });

  it('menandai kombinasi yang tidak bisa dipakai dan tetap merekam', () => {
    const recorder = new ShortcutRecorder();
    recorder.start();

    expect(recorder.keydown(key('KeyK', 'K', { shiftKey: true }))).toBeNull();
    expect(recorder.invalid).toBe(true);
    expect(recorder.recording).toBe(true);
  });

  it('Esc dan Tab menghentikan perekaman tanpa pintasan', () => {
    const recorder = new ShortcutRecorder();
    recorder.start();
    expect(recorder.keydown(key('Escape', 'Escape'))).toBeNull();
    expect(recorder.recording).toBe(false);

    recorder.start();
    const tab = key('Tab', 'Tab');
    expect(recorder.keydown(tab)).toBeNull();
    expect(recorder.recording).toBe(false);
    expect(tab.prevented).toBe(false);
  });
});
