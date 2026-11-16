import { type AskBackend, type AskContext, AskError, type AskNotice } from './backend';

export type AskPhase = 'idle' | 'streaming' | 'done' | 'error';

const GENERIC_ERROR = 'Nuenly tidak bisa menjawab sekarang. Coba lagi sebentar lagi.';
const REPLACE_ERROR = 'Teks tidak bisa diganti. Salin jawabannya, lalu tempel sendiri.';

/** Status satu sesi Tanya Nuenly, dari jendela dibuka sampai ditutup. */
export class AskSession {
  context = $state<AskContext | null>(null);
  /** Kenapa teks dari aplikasi tidak dipakai, misalnya karena Nuenly dijeda. */
  notice = $state<AskNotice | null>(null);
  prompt = $state('');
  answer = $state('');
  phase = $state<AskPhase>('idle');
  error = $state<string | null>(null);
  /** Hanya sebagian teks konteks yang dipakai karena terlalu panjang. */
  contextTruncated = $state(false);
  /** Ganti teks sedang berjalan. */
  replacing = $state(false);
  /** Kenapa Ganti teks terakhir tidak menempel jawaban. */
  replaceError = $state<string | null>(null);

  #backend: AskBackend;
  #lastPrompt = $state('');
  /** Ganti teks sudah dicoba untuk jawaban ini; Rust hanya mengizinkannya sekali per jawaban. */
  #replaceTried = $state(false);
  /** Naik setiap permintaan baru atau sesi dibuka ulang; token dari permintaan lama diabaikan. */
  #run = 0;

  constructor(backend: AskBackend) {
    this.#backend = backend;
  }

  get canCopy(): boolean {
    return this.phase === 'done' && this.answer.length > 0;
  }

  get canRetry(): boolean {
    return this.#lastPrompt !== '' && (this.phase === 'done' || this.phase === 'error');
  }

  /** Jawaban selesai, konteksnya masih dipakai dan bisa diganti, dan belum pernah dicoba. */
  get canReplace(): boolean {
    return this.canCopy && this.context?.replaceable === true && !this.replacing && !this.#replaceTried;
  }

  /** Ganti teks. Bila berhasil, Rust menutup jendela; bila tidak, alasannya tampil di `replaceError`. */
  async replace(): Promise<void> {
    if (!this.canReplace) {
      return;
    }
    this.replacing = true;
    this.replaceError = null;
    this.#replaceTried = true;
    try {
      await this.#backend.replace();
    } catch (err) {
      this.replaceError = err instanceof AskError ? err.message : REPLACE_ERROR;
    } finally {
      this.replacing = false;
    }
  }

  /** Memulai sesi baru, misalnya setiap kali hotkey ditekan. */
  open(context: AskContext | null, notice: AskNotice | null = null): void {
    this.#run += 1;
    this.context = context;
    this.notice = context === null ? notice : null;
    this.prompt = '';
    this.answer = '';
    this.phase = 'idle';
    this.error = null;
    this.contextTruncated = false;
    this.#lastPrompt = '';
    this.#resetReplace();
  }

  #resetReplace(): void {
    this.replacing = false;
    this.replaceError = null;
    this.#replaceTried = false;
  }

  clearContext(): void {
    this.context = null;
  }

  async submit(): Promise<void> {
    const prompt = this.prompt.trim();
    if (prompt === '' || this.phase === 'streaming') {
      return;
    }
    this.#lastPrompt = prompt;
    await this.#ask(prompt);
  }

  async retry(): Promise<void> {
    if (this.canRetry) {
      await this.#ask(this.#lastPrompt);
    }
  }

  async #ask(prompt: string): Promise<void> {
    this.#run += 1;
    const run = this.#run;
    this.answer = '';
    this.error = null;
    this.contextTruncated = false;
    this.#resetReplace();
    this.phase = 'streaming';

    try {
      const reply = await this.#backend.ask({ prompt, useContext: this.context !== null }, (token) => {
        if (run === this.#run) {
          this.answer += token;
        }
      });
      if (run === this.#run) {
        this.phase = 'done';
        this.contextTruncated = reply.contextTruncated;
      }
    } catch (err) {
      if (run === this.#run) {
        this.phase = 'error';
        // Pesan error lain bisa memuat detail teknis; yang tampil hanya pesan dari AskError.
        this.error = err instanceof AskError ? err.message : GENERIC_ERROR;
      }
    }
  }
}
