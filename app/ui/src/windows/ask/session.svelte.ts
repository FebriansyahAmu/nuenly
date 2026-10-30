import { type AskBackend, type AskContext, AskError, type AskNotice } from './backend';

export type AskPhase = 'idle' | 'streaming' | 'done' | 'error';

const GENERIC_ERROR = 'Nuenly tidak bisa menjawab sekarang. Coba lagi sebentar lagi.';

/** Status satu sesi Tanya Nuenly, dari jendela dibuka sampai ditutup. */
export class AskSession {
  context = $state<AskContext | null>(null);
  /** Kenapa teks terpilih tidak dipakai, misalnya karena Nuenly dijeda. */
  notice = $state<AskNotice | null>(null);
  prompt = $state('');
  answer = $state('');
  phase = $state<AskPhase>('idle');
  error = $state<string | null>(null);

  #backend: AskBackend;
  #lastPrompt = $state('');
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

  /** Memulai sesi baru, misalnya setiap kali hotkey ditekan. */
  open(context: AskContext | null, notice: AskNotice | null = null): void {
    this.#run += 1;
    this.context = context;
    this.notice = context === null ? notice : null;
    this.prompt = '';
    this.answer = '';
    this.phase = 'idle';
    this.error = null;
    this.#lastPrompt = '';
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
    this.phase = 'streaming';

    try {
      await this.#backend.ask({ prompt, useContext: this.context !== null }, (token) => {
        if (run === this.#run) {
          this.answer += token;
        }
      });
      if (run === this.#run) {
        this.phase = 'done';
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
