// Mode demo untuk melihat tampilan di browser saat `pnpm dev`, tanpa Tauri dan model.
// Hanya dimuat di build dev. Teksnya contoh rekaan dari kanvas desain 03.

import type { AskBackend, AskContext } from './backend';
import { AskSession } from './session.svelte';

const CONTEXT: AskContext = { source: 'selection', appName: 'Mail' };
const PROMPT = 'Rapikan jadi formal dan singkat';
const ANSWER = `Halo semua,

Rapat koordinasi minggu depan perlu dijadwalkan ulang karena ruangan sedang digunakan dan materi dari tim keuangan belum lengkap. Kami mengusulkan Kamis atau Jumat.

Mohon kabari hari yang paling memungkinkan. Terima kasih.`;

const backend: AskBackend = {
  async ask(_request, onToken) {
    for (const piece of ANSWER.match(/\S+\s*/g) ?? []) {
      await new Promise((resolve) => setTimeout(resolve, 25));
      onToken(piece);
    }
  },
};

/** `?jawab` langsung menjalankan perintah contoh; `?kosong` membuka tanpa konteks. */
export function demoSession(params: URLSearchParams): AskSession {
  const session = new AskSession(backend);
  session.open(params.has('kosong') ? null : CONTEXT);
  session.prompt = PROMPT;
  if (params.has('jawab')) {
    void session.submit();
  }
  return session;
}
