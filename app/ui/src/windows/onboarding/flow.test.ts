import { describe, expect, it } from 'vitest';

import type { OllamaStatus, OnboardingBackend } from './backend';
import { OnboardingFlow } from './flow.svelte';

function backend(overrides: Partial<OnboardingBackend> = {}): OnboardingBackend & { calls: string[] } {
  const calls: string[] = [];
  const record =
    <T>(name: string, value: T) =>
    () => {
      calls.push(name);
      return Promise.resolve(value);
    };
  return {
    calls,
    accept: record('accept', undefined),
    checkOllama: record<OllamaStatus>('checkOllama', 'running'),
    openDownload: record('openDownload', undefined),
    finish: record('finish', undefined),
    ...overrides,
  };
}

/** Menunggu pemeriksaan Ollama yang dimulai tanpa `await`. */
const settle = () => new Promise((resolve) => setTimeout(resolve));

describe('OnboardingFlow', () => {
  it('maju dan mundur antara selamat datang dan privasi', () => {
    const flow = new OnboardingFlow(backend());
    expect(flow.step).toBe('welcome');
    expect(flow.index).toBe(0);

    flow.next();
    expect(flow.step).toBe('privacy');

    flow.back();
    expect(flow.step).toBe('welcome');
  });

  it('tidak melewati privasi tanpa Saya mengerti', () => {
    const fake = backend();
    const flow = new OnboardingFlow(fake, 'privacy');

    flow.next();

    expect(flow.step).toBe('privacy');
    expect(fake.calls).toEqual([]);
  });

  it('menyimpan persetujuan lalu memeriksa Ollama', async () => {
    const fake = backend();
    const flow = new OnboardingFlow(fake, 'privacy');

    await flow.accept();
    await settle();

    expect(flow.step).toBe('model');
    expect(flow.ollama).toBe('running');
    expect(fake.calls).toEqual(['accept', 'checkOllama']);
  });

  it('tetap di privasi bila persetujuan gagal disimpan', async () => {
    const fake = backend({ accept: () => Promise.reject(new Error('EACCES C:\\Users\\x')) });
    const flow = new OnboardingFlow(fake, 'privacy');

    await flow.accept();

    expect(flow.step).toBe('privacy');
    expect(flow.error).toBe('Persetujuan tidak bisa disimpan. Coba lagi.');
    expect(flow.ollama).toBeNull();
  });

  it('menganggap Ollama tidak ada bila pemeriksaan gagal', async () => {
    const flow = new OnboardingFlow(backend({ checkOllama: () => Promise.reject(new Error('x')) }), 'model');

    await flow.checkOllama();

    expect(flow.ollama).toBe('missing');
  });

  it('mengabaikan hasil pemeriksaan lama', async () => {
    const pending: ((status: OllamaStatus) => void)[] = [];
    const flow = new OnboardingFlow(
      backend({ checkOllama: () => new Promise((resolve) => pending.push(resolve)) }),
      'model',
    );

    const first = flow.checkOllama();
    const second = flow.checkOllama();
    pending[1]?.('running');
    pending[0]?.('missing');
    await Promise.all([first, second]);

    expect(flow.ollama).toBe('running');
  });

  it('memeriksa ulang saat kembali ke jendela hanya bila Ollama belum berjalan', async () => {
    let status: OllamaStatus = 'missing';
    let checks = 0;
    const flow = new OnboardingFlow(
      backend({
        checkOllama: () => {
          checks += 1;
          return Promise.resolve(status);
        },
      }),
      'model',
    );
    await flow.checkOllama();

    status = 'running';
    flow.recheck();
    await settle();
    expect(flow.ollama).toBe('running');

    flow.recheck();
    await settle();
    expect(checks).toBe(2);
  });

  it('tidak memeriksa ulang di luar langkah model', () => {
    const fake = backend();
    new OnboardingFlow(fake, 'privacy').recheck();
    expect(fake.calls).toEqual([]);
  });

  it('memberi pesan sederhana bila halaman unduhan tidak bisa dibuka', async () => {
    const flow = new OnboardingFlow(backend({ openDownload: () => Promise.reject(new Error('0x80004005')) }), 'model');

    await flow.openDownload();

    expect(flow.error).toContain('ollama.com/download');
    expect(flow.error).not.toContain('0x');
  });
});
