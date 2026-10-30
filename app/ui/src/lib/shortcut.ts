// Merekam pintasan keyboard di Pengaturan. Aturannya sama dengan `Shortcut::parse` di
// `crates/nuenly-core/src/shortcut.rs`: Ctrl atau Alt wajib, lalu satu huruf, angka, F1–F24,
// atau Space, dengan urutan `Ctrl+Alt+Shift+Win+Tombol`.

/** Pintasan bawaan Tanya Nuenly. */
export const DEFAULT_SHORTCUT = 'Ctrl+Space';

/** Hasil membaca tombol yang sedang ditekan saat merekam. */
export type ShortcutCapture =
  /** Baru tombol pengubah yang ditekan, misalnya `Ctrl+…`. */
  | { kind: 'partial'; text: string }
  | { kind: 'complete'; shortcut: string }
  /** Tombol utama tidak didukung, atau tanpa Ctrl maupun Alt. */
  | { kind: 'invalid' };

type Keys = Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'code'>;

const MODIFIER_CODE = /^(Control|Alt|Shift|Meta|OS)(Left|Right)$/;

function keyName(code: string): string | null {
  if (code === 'Space') {
    return 'Space';
  }
  const match = /^Key([A-Z])$/.exec(code) ?? /^Digit([0-9])$/.exec(code);
  if (match) {
    return match[1] ?? null;
  }
  return /^F([1-9]|1[0-9]|2[0-4])$/.test(code) ? code : null;
}

/** Membaca kombinasi dari `keydown`. Memakai `code`, jadi tidak bergantung tata letak keyboard. */
export function captureShortcut(keys: Keys): ShortcutCapture {
  const modifiers = [
    keys.ctrlKey ? 'Ctrl' : null,
    keys.altKey ? 'Alt' : null,
    keys.shiftKey ? 'Shift' : null,
    keys.metaKey ? 'Win' : null,
  ].filter((name) => name !== null);
  if (MODIFIER_CODE.test(keys.code)) {
    return { kind: 'partial', text: [...modifiers, '…'].join('+') };
  }
  const key = keyName(keys.code);
  if (key === null || !(keys.ctrlKey || keys.altKey)) {
    return { kind: 'invalid' };
  }
  return { kind: 'complete', shortcut: [...modifiers, key].join('+') };
}
