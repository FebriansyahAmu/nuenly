import { describe, expect, it } from 'vitest';

import { nextIndex } from './mode-switch';

describe('nextIndex', () => {
  it('maju dan mundur sambil berputar', () => {
    expect(nextIndex(0, 'ArrowRight', 2)).toBe(1);
    expect(nextIndex(1, 'ArrowRight', 2)).toBe(0);
    expect(nextIndex(0, 'ArrowLeft', 2)).toBe(1);
    expect(nextIndex(1, 'ArrowUp', 2)).toBe(0);
  });

  it('Home dan End ke ujung, tombol lain diabaikan', () => {
    expect(nextIndex(1, 'Home', 3)).toBe(0);
    expect(nextIndex(0, 'End', 3)).toBe(2);
    expect(nextIndex(0, 'Enter', 3)).toBeNull();
    expect(nextIndex(0, 'ArrowRight', 0)).toBeNull();
  });
});
