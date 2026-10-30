<script lang="ts">
  import LogOut from '@lucide/svelte/icons/log-out';
  import Pause from '@lucide/svelte/icons/pause';
  import Play from '@lucide/svelte/icons/play';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
  import { onMount } from 'svelte';

  import ModeSwitch from '$lib/components/mode-switch.svelte';
  import NuenlyMark from '$lib/components/nuenly-mark.svelte';
  import { Button } from '$lib/components/ui/button';
  import { applyAccent, type StatusView } from '$lib/status';
  import { cn } from '$lib/utils';

  import type { TrayBackend } from './backend';

  let {
    backend,
    initial,
    watch,
  }: {
    backend: TrayBackend;
    initial: StatusView;
    /** Mengikuti perubahan status dari Rust; tidak ada di mode demo. */
    watch?: (onChange: (status: StatusView) => void) => Promise<() => void>;
  } = $props();

  let latest = $state<StatusView | null>(null);
  const current = $derived(latest ?? initial);
  const paused = $derived(current.pausedUntilMs !== null);

  const timeFormat = new Intl.DateTimeFormat('id-ID', { hour: '2-digit', minute: '2-digit' });

  onMount(() => {
    applyAccent(initial);
    const stop = watch?.((status) => (latest = status));
    return () => void stop?.then((unwatch) => unwatch());
  });

  async function run(action: () => Promise<StatusView>) {
    const next = await action();
    applyAccent(next);
    latest = next;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      void backend.close();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main class="flex h-screen flex-col bg-background text-foreground select-none">
  <div class="flex flex-col px-4.5 pt-5">
    <header class="flex items-center gap-3.5">
      <NuenlyMark class="size-[26px] shrink-0" />
      <div class="min-w-0 flex-1">
        <h1 class="text-[15px] leading-tight font-semibold">
          {paused ? 'Nuenly dijeda' : 'Nuenly aktif'}
        </h1>
        <p class="mt-0.5 truncate text-sm text-muted-foreground">
          {#if paused && current.pausedUntilMs !== null}
            Lanjut otomatis pukul {timeFormat.format(current.pausedUntilMs)}
          {:else}
            Siap membantu
          {/if}
        </p>
      </div>
      <span
        class={cn('size-2.5 shrink-0 rounded-full', paused ? 'bg-amber-500' : 'bg-green-500')}
        aria-hidden="true"
      ></span>
    </header>

    <ModeSwitch
      class="mt-5"
      modes={current.modes}
      active={current.activeMode}
      onselect={(id) => run(() => backend.setMode(id))}
    />

    <p class="mt-3.5 text-[13px] leading-snug text-muted-foreground">
      Mode berganti otomatis sesuai aplikasi yang Anda buka.
    </p>

    <Button
      variant="outline"
      class="mt-4 h-9 w-full text-[15px]"
      onclick={() => run(() => (paused ? backend.resume() : backend.pause()))}
    >
      {#if paused}
        <Play /> Lanjutkan sekarang
      {:else}
        <Pause /> Jeda 15 menit
      {/if}
    </Button>
  </div>

  <nav class="mt-auto flex flex-col border-t px-1.5 py-1.5" aria-label="Menu Nuenly">
    <Button variant="ghost" class="h-9 justify-start gap-2.5 px-3 text-[15px] font-normal" onclick={() => backend.openSettings()}>
      <SlidersHorizontal /> Pengaturan
    </Button>
    <Button variant="ghost" class="h-9 justify-start gap-2.5 px-3 text-[15px] font-normal" onclick={() => backend.quit()}>
      <LogOut /> Keluar
    </Button>
  </nav>
</main>
