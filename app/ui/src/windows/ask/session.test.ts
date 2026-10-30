import { describe, expect, it } from 'vitest';

import { type AskBackend, AskError, type AskRequest } from './backend';
import { AskSession } from './session.svelte';

const CONTEXT = { source: 'selection', appName: 'Mail' } as const;

function streaming(tokens: string[], contextTruncated = false): AskBackend & { requests: AskRequest[] } {
  const requests: AskRequest[] = [];
  return {
    requests,
    async ask(request, onToken) {
      requests.push(request);
      for (const token of tokens) {
        onToken(token);
      }
      return { contextTruncated };
    },
  };
}

function failing(error: unknown): AskBackend {
  return { ask: () => Promise.reject(error) };
}

describe('AskSession', () => {
  it('menggabungkan token jawaban lalu selesai', async () => {
    const backend = streaming(['Halo ', 'semua']);
    const session = new AskSession(backend);
    session.open(CONTEXT);
    session.prompt = '  Rapikan  ';

    await session.submit();

    expect(session.phase).toBe('done');
    expect(session.answer).toBe('Halo semua');
    expect(session.canCopy).toBe(true);
    expect(backend.requests).toEqual([{ prompt: 'Rapikan', useContext: true }]);
  });

  it('mencatat teks terpilih yang terpotong sampai permintaan berikutnya', async () => {
    const session = new AskSession(streaming(['ok'], true));
    session.open(CONTEXT);
    session.prompt = 'Ringkas';

    await session.submit();
    expect(session.contextTruncated).toBe(true);

    session.open(CONTEXT);
    expect(session.contextTruncated).toBe(false);
  });

  it('tidak mengirim perintah kosong', async () => {
    const backend = streaming(['x']);
    const session = new AskSession(backend);
    session.prompt = '   ';

    await session.submit();

    expect(session.phase).toBe('idle');
    expect(backend.requests).toHaveLength(0);
  });

  it('tanpa konteks setelah chip dihapus', async () => {
    const backend = streaming(['ok']);
    const session = new AskSession(backend);
    session.open(CONTEXT);
    session.clearContext();
    session.prompt = 'Jelaskan';

    await session.submit();

    expect(backend.requests[0]?.useContext).toBe(false);
  });

  it('keterangan hanya berlaku tanpa konteks dan hilang di sesi berikutnya', () => {
    const session = new AskSession(streaming([]));

    session.open(null, 'paused');
    expect(session.notice).toBe('paused');

    session.open(CONTEXT, 'unreadable');
    expect(session.notice).toBeNull();

    session.open(null);
    expect(session.notice).toBeNull();
  });

  it('menampilkan pesan AskError apa adanya', async () => {
    const session = new AskSession(failing(new AskError('Model belum siap.')));
    session.prompt = 'Halo';

    await session.submit();

    expect(session.phase).toBe('error');
    expect(session.error).toBe('Model belum siap.');
    expect(session.canRetry).toBe(true);
  });

  it('tidak membocorkan pesan error lain', async () => {
    const session = new AskSession(failing(new Error('connect ECONNREFUSED 127.0.0.1:11434')));
    session.prompt = 'Halo';

    await session.submit();

    expect(session.error).not.toContain('11434');
  });

  it('coba lagi mengirim ulang perintah terakhir', async () => {
    const backend = streaming(['ok']);
    const session = new AskSession(backend);
    session.prompt = 'Ringkas';
    await session.submit();
    session.prompt = 'sudah diubah';

    await session.retry();

    expect(backend.requests.map((r) => r.prompt)).toEqual(['Ringkas', 'Ringkas']);
  });

  it('membuka ulang mengabaikan token dari permintaan lama', async () => {
    let emit: (token: string) => void = () => {};
    let finish: () => void = () => {};
    const backend: AskBackend = {
      ask: (_request, onToken) =>
        new Promise((resolve) => {
          emit = onToken;
          finish = () => resolve({ contextTruncated: false });
        }),
    };
    const session = new AskSession(backend);
    session.prompt = 'Halo';
    const pending = session.submit();

    session.open(null);
    emit('token lama');
    finish();
    await pending;

    expect(session.answer).toBe('');
    expect(session.phase).toBe('idle');
  });
});
