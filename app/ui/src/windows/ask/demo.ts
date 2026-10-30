// Mode demo untuk melihat tampilan di browser saat `pnpm dev`, tanpa Tauri dan model.
// Hanya dimuat di build dev. Teksnya contoh rekaan dari kanvas desain 03.

import type { AskBackend, AskContext, AskNotice } from './backend';
import { AskSession } from './session.svelte';

const CONTEXT: AskContext = {
  source: 'selection',
  appName: 'Mail',
  preview:
    'jadi gini, rapat minggu depan digeser ke kamis aja soalnya ruangan dipake dan materi dari tim keuangan belum lengkap. tolong kabarin bisa ga',
};
const PROMPT = 'Rapikan jadi formal dan singkat';
const ANSWER = `Halo semua,

Rapat koordinasi minggu depan perlu dijadwalkan ulang karena ruangan sedang digunakan dan materi dari tim keuangan belum lengkap. Kami mengusulkan Kamis atau Jumat.

Mohon kabari hari yang paling memungkinkan. Terima kasih.`;

function backend(contextTruncated: boolean): AskBackend {
  return {
    async ask(_request, onToken) {
      for (const piece of ANSWER.match(/\S+\s*/g) ?? []) {
        await new Promise((resolve) => setTimeout(resolve, 25));
        onToken(piece);
      }
      return { contextTruncated };
    },
  };
}

const NOTICES: AskNotice[] = ['paused', 'notAllowed', 'privateWindow', 'banking', 'protectedField', 'unreadable'];

/**
 * `?jawab` langsung menjalankan perintah contoh; `?panjang` menandai teks terpilih terpotong;
 * `?tanpa-cuplikan` meniru mode yang tidak menampilkan cuplikan; `?kosong` membuka tanpa
 * konteks; `?catatan=<alasan>` (lihat `NOTICES`) membuka tanpa konteks beserta alasannya.
 */
export function demoSession(params: URLSearchParams): AskSession {
  const session = new AskSession(backend(params.has('panjang')));
  const notice = NOTICES.find((candidate) => candidate === params.get('catatan')) ?? null;
  const context = params.has('tanpa-cuplikan') ? { ...CONTEXT, preview: null } : CONTEXT;
  session.open(params.has('kosong') || notice !== null ? null : context, notice);
  session.prompt = PROMPT;
  if (params.has('jawab')) {
    void session.submit();
  }
  return session;
}
