import { describe, expect, it } from 'vitest';

import { type AskBackend, type AskContext, AskError, type AskRequest } from './backend';
import { AskSession } from './session.svelte';

const CONTEXT = { source: 'selection', appName: 'Mail', preview: 'cuplikan sintetis', replaceable: true } as const;

function streaming(
  tokens: string[],
  contextTruncated = false,
  replace: () => Promise<void> = () => Promise.resolve(),
): AskBackend & { requests: AskRequest[]; replaced: number } {
  const requests: AskRequest[] = [];
  const backend = {
    requests,
    replaced: 0,
    async ask(request: AskRequest, onToken: (token: string) => void) {
      requests.push(request);
      for (const token of tokens) {
        onToken(token);
      }
      return { contextTruncated };
    },
    async replace() {
      backend.replaced += 1;
      await replace();
    },
  };
  return backend;
}

function failing(error: unknown): AskBackend {
  return { ask: () => Promise.reject(error), replace: () => Promise.resolve() };
}

async function answered(backend: AskBackend, context: AskContext = CONTEXT): Promise<AskSession> {
  const session = new AskSession(backend);
  session.open(context);
  session.prompt = 'Rapikan';
  await session.submit();
  return session;
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

  it('ganti teks hanya setelah jawaban selesai, dan sekali per jawaban', async () => {
    const backend = streaming(['Halo ', 'semua']);
    const session = new AskSession(backend);
    session.open(CONTEXT);
    expect(session.canReplace).toBe(false);

    session.prompt = 'Rapikan';
    await session.submit();
    expect(session.canReplace).toBe(true);

    await session.replace();
    await session.replace();
    expect(backend.replaced).toBe(1);
    expect(session.canReplace).toBe(false);

    // Jawaban baru boleh diganti lagi.
    await session.retry();
    expect(session.canReplace).toBe(true);
  });

  it('ganti teks tidak tersedia tanpa konteks yang bisa diganti', async () => {
    const document = await answered(streaming(['ok']), { ...CONTEXT, source: 'field', replaceable: false });
    expect(document.canReplace).toBe(false);

    const cleared = await answered(streaming(['ok']));
    cleared.clearContext();
    expect(cleared.canReplace).toBe(false);
  });

  it('menampilkan alasan ganti teks ditolak tanpa detail teknis', async () => {
    const changed = await answered(streaming(['ok'], false, () => Promise.reject(new AskError('Teks sudah berubah.'))));
    await changed.replace();
    expect(changed.replaceError).toBe('Teks sudah berubah.');
    expect(changed.replacing).toBe(false);
    expect(changed.canCopy).toBe(true);

    const unknown = await answered(streaming(['ok'], false, () => Promise.reject(new Error('E_ACCESSDENIED'))));
    await unknown.replace();
    expect(unknown.replaceError).not.toContain('E_ACCESSDENIED');

    unknown.open(CONTEXT);
    expect(unknown.replaceError).toBeNull();
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
      replace: () => Promise.resolve(),
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
