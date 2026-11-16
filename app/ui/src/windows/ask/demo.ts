// Mode demo untuk melihat tampilan di browser saat `pnpm dev`, tanpa Tauri dan model.
// Hanya dimuat di build dev. Teksnya contoh rekaan dari kanvas desain 03.

import { type AskBackend, type AskContext, AskError, type AskNotice } from './backend';
import { AskSession } from './session.svelte';
import { replaceFailureMessage } from './tauri';

const CONTEXT: AskContext = {
  source: 'selection',
  appName: 'Mail',
  preview:
    'jadi gini, rapat minggu depan digeser ke kamis aja soalnya ruangan dipake dan materi dari tim keuangan belum lengkap. tolong kabarin bisa ga',
  replaceable: true,
};
const PROMPT = 'Rapikan jadi formal dan singkat';
const ANSWER = `Halo semua,

Rapat koordinasi minggu depan perlu dijadwalkan ulang karena ruangan sedang digunakan dan materi dari tim keuangan belum lengkap. Kami mengusulkan Kamis atau Jumat.

Mohon kabari hari yang paling memungkinkan. Terima kasih.`;

function backend(contextTruncated: boolean, changed: boolean): AskBackend {
  return {
    async ask(_request, onToken) {
      for (const piece of ANSWER.match(/\S+\s*/g) ?? []) {
        await new Promise((resolve) => setTimeout(resolve, 25));
        onToken(piece);
      }
      return { contextTruncated };
    },
    // Di aplikasi, Rust menempel jawaban lalu menutup jendela; di demo tidak ada yang ditempel.
    async replace() {
      await new Promise((resolve) => setTimeout(resolve, 300));
      if (changed) {
        throw new AskError(replaceFailureMessage('changed'));
      }
    },
  };
}

const NOTICES: AskNotice[] = ['paused', 'notAllowed', 'privateWindow', 'banking', 'protectedField', 'unreadable'];

/**
 * `?jawab` langsung menjalankan perintah contoh; `?panjang` menandai teks konteks terpotong;
 * `?field` memakai isi field sebagai konteks, bukan teks terpilih; `?dokumen` memakai bagian
 * dokumen panjang yang tidak bisa diganti; `?berubah` membuat Ganti teks ditolak karena teksnya
 * sudah berubah; `?tanpa-cuplikan` meniru mode yang tidak menampilkan cuplikan; `?kosong` membuka
 * tanpa konteks; `?catatan=<alasan>` (lihat `NOTICES`) membuka tanpa konteks beserta alasannya.
 */
export function demoSession(params: URLSearchParams): AskSession {
  const session = new AskSession(backend(params.has('panjang'), params.has('berubah')));
  const notice = NOTICES.find((candidate) => candidate === params.get('catatan')) ?? null;
  const source: AskContext['source'] = params.has('field') || params.has('dokumen') ? 'field' : 'selection';
  const context = {
    ...CONTEXT,
    source,
    replaceable: !params.has('dokumen'),
    ...(params.has('tanpa-cuplikan') ? { preview: null } : {}),
  };
  session.open(params.has('kosong') || notice !== null ? null : context, notice);
  session.prompt = PROMPT;
  if (params.has('jawab')) {
    void session.submit();
  }
  return session;
}
