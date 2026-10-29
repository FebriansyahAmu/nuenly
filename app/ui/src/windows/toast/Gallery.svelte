<script lang="ts">
  // Semua varian sekaligus untuk dibandingkan dengan kanvas desain 02 (hanya build dev).
  import { DEMO_TOASTS } from './demo';
  import ToastView from './ToastView.svelte';
</script>

<main class="grid min-h-screen grid-cols-3 content-start gap-x-6 gap-y-8 bg-neutral-100 p-12">
  <section>
    <p class="mb-3 text-sm">1 · Saran</p>
    <ToastView payload={DEMO_TOASTS['saran']!} />
  </section>
  <section>
    <p class="mb-3 text-sm">2 · Saran penting</p>
    <ToastView payload={DEMO_TOASTS['penting']!} />
  </section>
  <section>
    <p class="mb-3 text-sm">3 · Setelah "Lihat"</p>
    <ToastView payload={DEMO_TOASTS['penting']!} expanded />
  </section>
  <section>
    <p class="mb-3 text-sm">4 · Berhasil</p>
    <ToastView payload={DEMO_TOASTS['berhasil']!} />
  </section>
  <section>
    <p class="mb-3 text-sm">5 · Sedang berpikir</p>
    <ToastView payload={DEMO_TOASTS['berpikir']!} />
  </section>
  <section>
    <p class="mb-3 text-sm">6 · Butuh izin</p>
    <ToastView payload={DEMO_TOASTS['izin']!} />
  </section>
</main>
