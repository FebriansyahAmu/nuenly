import type { OllamaStatus, OnboardingBackend } from './backend';

export const STEPS = ['welcome', 'privacy', 'model'] as const;
export type Step = (typeof STEPS)[number];

const ACCEPT_ERROR = 'Persetujuan tidak bisa disimpan. Coba lagi.';
const DOWNLOAD_ERROR = 'Halaman unduhan tidak bisa dibuka. Buka ollama.com/download di browser.';

/** Langkah onboarding dan hasil pemeriksaan Ollama. */
export class OnboardingFlow {
  step = $state<Step>('welcome');
  /** `null` sebelum pemeriksaan pertama. */
  ollama = $state<OllamaStatus | 'checking' | null>(null);
  accepting = $state(false);
  error = $state<string | null>(null);

  #backend: OnboardingBackend;
  /** Naik setiap pemeriksaan baru; hasil pemeriksaan lama diabaikan. */
  #check = 0;

  constructor(backend: OnboardingBackend, step: Step = 'welcome') {
    this.#backend = backend;
    this.step = step;
  }

  get index(): number {
    return STEPS.indexOf(this.step);
  }

  /** Dari selamat datang ke privasi. Langkah model hanya bisa dicapai lewat {@link accept}. */
  next(): void {
    if (this.step === 'welcome') {
      this.#go('privacy');
    }
  }

  back(): void {
    if (this.step === 'model') {
      this.#go('privacy');
    } else if (this.step === 'privacy') {
      this.#go('welcome');
    }
  }

  /** "Saya mengerti": menyimpan persetujuan, lalu lanjut ke langkah model. */
  async accept(): Promise<void> {
    if (this.accepting) {
      return;
    }
    this.accepting = true;
    this.error = null;
    try {
      await this.#backend.accept();
      this.#go('model');
      void this.checkOllama();
    } catch {
      this.error = ACCEPT_ERROR;
    } finally {
      this.accepting = false;
    }
  }

  async checkOllama(): Promise<void> {
    this.#check += 1;
    const check = this.#check;
    this.ollama = 'checking';
    let result: OllamaStatus;
    try {
      result = await this.#backend.checkOllama();
    } catch {
      result = 'missing';
    }
    if (check === this.#check) {
      this.ollama = result;
    }
  }

  /** Dipanggil saat jendela kembali fokus, misalnya setelah pengguna memasang Ollama. */
  recheck(): void {
    if (this.step === 'model' && this.ollama !== 'running' && this.ollama !== 'checking') {
      void this.checkOllama();
    }
  }

  async openDownload(): Promise<void> {
    this.error = null;
    try {
      await this.#backend.openDownload();
    } catch {
      this.error = DOWNLOAD_ERROR;
    }
  }

  async finish(): Promise<void> {
    await this.#backend.finish();
  }

  #go(step: Step): void {
    this.step = step;
    this.error = null;
  }
}
