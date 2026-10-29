import { describe, expect, it } from 'vitest';

import { toastLayout } from './layout.svelte';

describe('toastLayout', () => {
  it('mengikuti tinggi toast terbaru dan kembali ke yang sebelumnya', () => {
    expect(toastLayout.frontHeight).toBe(0);

    toastLayout.set('a', 80);
    toastLayout.set('b', 120);
    expect(toastLayout.frontHeight).toBe(120);

    toastLayout.set('b', 200);
    expect(toastLayout.frontHeight).toBe(200);

    toastLayout.delete('b');
    expect(toastLayout.frontHeight).toBe(80);

    toastLayout.delete('a');
    expect(toastLayout.frontHeight).toBe(0);
  });

  it('mengabaikan toast yang belum terukur agar jendela tidak disembunyikan terlalu cepat', () => {
    toastLayout.set('lama', 90);
    toastLayout.set('baru', 0);
    expect(toastLayout.frontHeight).toBe(90);

    toastLayout.set('baru', 140);
    expect(toastLayout.frontHeight).toBe(140);

    toastLayout.delete('baru');
    toastLayout.delete('lama');
  });
});
