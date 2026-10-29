<script lang="ts">
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import Shield from '@lucide/svelte/icons/shield';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import X from '@lucide/svelte/icons/x';

  import NuenlyMark from '$lib/components/nuenly-mark.svelte';
  import { Button } from '$lib/components/ui/button';

  import ToastFrame from './ToastFrame.svelte';
  import type { ToastPayload } from './types';

  let {
    payload,
    expanded = false,
    onExpand,
    onClose,
    onCopyPoints,
  }: {
    payload: ToastPayload;
    expanded?: boolean;
    onExpand?: () => void;
    onClose?: () => void;
    /** Menyalin poin; error berarti gagal dan ditampilkan di toast. */
    onCopyPoints?: (text: string) => Promise<void>;
    /** Dari svelte-sonner; tidak dipakai karena penutupan lewat `onClose`. */
    closeToast?: () => void;
  } = $props();

  let copyFailed = $state(false);

  async function copyPoints(points: string[]) {
    copyFailed = false;
    try {
      await onCopyPoints?.(points.map((point) => `- ${point}`).join('\n'));
    } catch {
      copyFailed = true;
    }
  }
</script>

<!-- Toast satu baris (berhasil, berpikir) lebih rapat, seperti di desain 02. -->
<ToastFrame
  id={payload.id}
  class={payload.kind === 'success' ? 'py-3.5' : payload.kind === 'thinking' ? 'py-3' : undefined}
>
  {#if payload.kind === 'suggestion'}
    <div class="flex gap-3.5">
      <NuenlyMark class="mt-0.5 size-[18px] shrink-0" />
      <div class="min-w-0 flex-1">
        <p class="pr-6 text-sm leading-snug font-semibold">{payload.title}</p>

        {#if expanded}
          <!-- Teks poin sejajar judul; bullet menggantung di kiri seperti di desain. -->
          <ul class="mt-2.5 list-disc space-y-1 pl-0 text-[13px] leading-relaxed">
            {#each payload.points as point, index (index)}
              <li>{point}</li>
            {/each}
          </ul>
          {#if payload.source}
            <p class="mt-2.5 text-xs text-muted-foreground">{payload.source}</p>
          {/if}
          {#if copyFailed}
            <p class="mt-2 text-xs text-destructive" role="alert">Poin tidak bisa disalin. Coba lagi.</p>
          {/if}
          <div class="mt-3 flex items-center gap-2">
            <Button size="sm" onclick={() => copyPoints(payload.points)}>Salin poin</Button>
            <Button size="sm" variant="outline" onclick={onClose}>Tutup</Button>
          </div>
        {:else}
          <p class="mt-1 text-[13px] text-muted-foreground">{payload.body}</p>
          {#if payload.warning}
            <p class="mt-2.5 flex gap-1.5 text-[13px] text-destructive">
              <TriangleAlert class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
              <span>{payload.warning}</span>
            </p>
          {/if}
          <div class="mt-3 flex items-center gap-2">
            {#if payload.points.length > 0}
              <Button size="sm" onclick={onExpand}>Lihat</Button>
            {/if}
            <Button size="sm" variant="outline" onclick={onClose}>Abaikan</Button>
            {#if payload.note}
              <span class="ml-auto text-xs text-muted-foreground">{payload.note}</span>
            {/if}
          </div>
        {/if}
      </div>
    </div>
    {#if !expanded}
      <button
        type="button"
        class="absolute top-3 right-3 rounded-sm p-0.5 text-muted-foreground hover:text-foreground"
        aria-label="Tutup saran"
        onclick={onClose}
      >
        <X class="size-4" />
      </button>
    {/if}
  {:else if payload.kind === 'success'}
    <p class="flex items-center gap-2.5 text-sm">
      <CircleCheck class="size-[18px] shrink-0 text-green-600" aria-hidden="true" />
      {payload.message}
    </p>
  {:else if payload.kind === 'thinking'}
    <div class="flex items-center gap-2.5 text-sm">
      <LoaderCircle class="size-[18px] shrink-0 animate-spin text-muted-foreground" aria-hidden="true" />
      <span class="min-w-0 flex-1 truncate">{payload.message}</span>
      <Button size="sm" variant="ghost" class="text-muted-foreground" onclick={onClose}>Batal</Button>
    </div>
  {:else if payload.kind === 'permission'}
    <div class="flex gap-3.5">
      <Shield class="mt-0.5 size-[18px] shrink-0" aria-hidden="true" />
      <div class="min-w-0 flex-1">
        <p class="text-sm leading-snug font-semibold">{payload.title}</p>
        <p class="mt-1 text-[13px] text-muted-foreground">{payload.body}</p>
        <div class="mt-3 flex items-center gap-2">
          <!-- Keputusan izin dikirim ke policy gate MCP (F-17) di Fase 2. -->
          <Button size="sm" onclick={onClose}>Izinkan</Button>
          <Button size="sm" variant="outline" onclick={onClose}>Tolak</Button>
        </div>
      </div>
    </div>
  {/if}
</ToastFrame>
