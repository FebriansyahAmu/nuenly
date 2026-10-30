import { captureShortcut } from '$lib/shortcut';

type KeyEvent = Pick<
  KeyboardEvent,
  'key' | 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'preventDefault' | 'stopPropagation'
>;

const plain = (event: Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>) =>
  !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey;

/** Merekam satu kombinasi tombol untuk pintasan baru di Pengaturan › Umum. */
export class ShortcutRecorder {
  recording = $state(false);
  /** Tombol pengubah yang sedang ditahan, misalnya "Ctrl+…". */
  held = $state<string | null>(null);
  /** Tombol terakhir tidak bisa dipakai sebagai pintasan. */
  invalid = $state(false);

  start(): void {
    this.recording = true;
    this.held = null;
    this.invalid = false;
  }

  stop(): void {
    this.recording = false;
    this.held = null;
  }

  /** Membaca `keydown` selama merekam. Hasilnya pintasan lengkap, atau `null` bila belum ada. */
  keydown(event: KeyEvent): string | null {
    if (!this.recording) {
      return null;
    }
    // Tab tetap memindahkan fokus, jadi perekaman berhenti.
    if (plain(event) && event.key === 'Tab') {
      this.stop();
      return null;
    }
    event.preventDefault();
    event.stopPropagation();
    if (plain(event) && event.key === 'Escape') {
      this.stop();
      return null;
    }
    const capture = captureShortcut(event);
    if (capture.kind === 'partial') {
      this.held = capture.text;
      this.invalid = false;
      return null;
    }
    if (capture.kind === 'invalid') {
      this.held = null;
      this.invalid = true;
      return null;
    }
    this.stop();
    return capture.shortcut;
  }

  keyup(event: Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>): void {
    if (this.recording && plain(event)) {
      this.held = null;
    }
  }
}
