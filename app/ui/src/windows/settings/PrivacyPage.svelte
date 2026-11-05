<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Lock from '@lucide/svelte/icons/lock';
  import X from '@lucide/svelte/icons/x';
  import { onMount } from 'svelte';

  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { LOCAL_ONLY, NEVER, READS } from '$lib/privacy';

  import { ACTIVITY_LIMIT, ACTIVITY_PREVIEW, describeActivity, formatWhen } from './activity';
  import Advanced from './Advanced.svelte';
  import type { SettingsState } from './settings.svelte';

  let { state }: { state: SettingsState } = $props();

  const timeFormat = new Intl.DateTimeFormat('id-ID', { hour: '2-digit', minute: '2-digit' });
  const pausedUntil = $derived(state.status.pausedUntilMs);

  let confirm: HTMLDialogElement | undefined;
  const activity = $derived(state.activity ?? []);
  const shown = $derived(state.allActivity ? activity : activity.slice(0, ACTIVITY_PREVIEW));
  /** "Hari ini" dan "Kemarin" dihitung dari saat riwayat dimuat. */
  const now = $derived.by(() => {
    void state.activity;
    return new Date();
  });

  onMount(() => {
    void state.loadActivity();
  });

  async function clear() {
    confirm?.close();
    await state.clearActivity();
  }
</script>

<Card.Root class="gap-0 [--card-spacing:--spacing(5)]">
  <Card.Content class="gap-0">
    <h2 class="font-semibold">Yang dibaca Nuenly</h2>
    <ul class="mt-3 space-y-2.5">
      <!-- Ikon sejajar baris pertama, karena butir yang panjang bisa menjadi dua baris. -->
      {#each READS as item (item)}
        <li class="flex items-start gap-2">
          <Check class="mt-0.5 size-4 shrink-0 text-green-600" aria-hidden="true" />
          {item}
        </li>
      {/each}
      {#each NEVER as item (item)}
        <li class="flex items-start gap-2">
          <X class="mt-0.5 size-4 shrink-0 text-red-600" aria-hidden="true" />
          {item}
        </li>
      {/each}
    </ul>
    <p class="mt-4 flex items-center gap-2 text-[13px] text-muted-foreground">
      <Lock class="size-3.5 shrink-0" aria-hidden="true" />
      {LOCAL_ONLY}
    </p>
  </Card.Content>
</Card.Root>

<Card.Root class="gap-0 py-0">
  <div class="flex items-center justify-between gap-4 px-5 py-3.5">
    <div>
      <h2 class="font-medium">{pausedUntil === null ? 'Jeda Nuenly' : 'Nuenly dijeda'}</h2>
      <p class="text-[13px] text-muted-foreground">
        {#if pausedUntil === null}
          Berhenti membaca selama 15 menit, lalu lanjut sendiri.
        {:else}
          Lanjut otomatis pukul {timeFormat.format(pausedUntil)}.
        {/if}
      </p>
    </div>
    <Button variant="outline" size="sm" onclick={() => state.togglePause()}>
      {pausedUntil === null ? 'Jeda 15 menit' : 'Lanjutkan sekarang'}
    </Button>
  </div>
</Card.Root>

<Card.Root class="gap-0 py-0">
  <div class="flex items-start justify-between gap-4 border-b px-5 py-3.5">
    <div>
      <h2 class="font-semibold">Riwayat aktivitas</h2>
      <p class="mt-0.5 text-[13px] text-muted-foreground">
        Tanpa isi teks. Disimpan terenkripsi di PC ini dan dihapus otomatis setelah 30 hari.
      </p>
    </div>
    {#if activity.length > 0}
      <Button variant="outline" size="sm" onclick={() => confirm?.showModal()}>Hapus riwayat</Button>
    {/if}
  </div>
  {#if state.activityFailed}
    <p class="px-5 py-6 text-center text-muted-foreground">Riwayat tidak bisa dibuka. Buka halaman ini lagi nanti.</p>
  {:else if state.activity === null}
    <p class="px-5 py-6 text-center text-muted-foreground">Memuat…</p>
  {:else if activity.length === 0}
    <p class="px-5 py-6 text-center text-muted-foreground">Belum ada aktivitas.</p>
  {:else}
    <ul class="divide-y">
      {#each shown as item, index (index)}
        {@const row = describeActivity(item, state.status.modes)}
        <li class="flex items-center justify-between gap-4 px-5 py-3">
          <div class="min-w-0">
            <p class="font-medium">{row.title}</p>
            <p class="truncate text-[13px] text-muted-foreground" title={row.detail}>{row.detail}</p>
          </div>
          <div class="shrink-0 text-right text-[13px]">
            <p class="text-muted-foreground">
              <time datetime={new Date(item.atMs).toISOString()}>{formatWhen(item.atMs, now)}</time>
            </p>
            <p class={row.failed ? 'text-amber-700' : 'text-muted-foreground'}>{row.outcome}</p>
          </div>
        </li>
      {/each}
    </ul>
    {#if activity.length > ACTIVITY_PREVIEW}
      <div class="border-t px-5 py-2.5">
        <Button variant="ghost" size="sm" class="-ml-2.5" onclick={() => (state.allActivity = !state.allActivity)}>
          {state.allActivity ? 'Tampilkan lebih sedikit' : `Tampilkan semua (${activity.length})`}
        </Button>
        {#if state.allActivity && activity.length >= ACTIVITY_LIMIT}
          <p class="mt-1 text-[13px] text-muted-foreground">Hanya {ACTIVITY_LIMIT} catatan terbaru yang ditampilkan.</p>
        {/if}
      </div>
    {/if}
  {/if}
</Card.Root>

<dialog
  bind:this={confirm}
  aria-labelledby="clear-title"
  aria-describedby="clear-description"
  class="m-auto w-[360px] max-w-[calc(100vw-32px)] rounded-xl border bg-background p-5 text-sm text-foreground shadow-lg backdrop:bg-black/40"
>
  <h2 id="clear-title" class="font-semibold">Hapus riwayat aktivitas?</h2>
  <p id="clear-description" class="mt-1.5 text-muted-foreground">
    Semua catatan di PC ini dihapus. Setelan Nuenly tidak ikut terhapus.
  </p>
  <form method="dialog" class="mt-5 flex justify-end gap-2">
    <Button type="submit" variant="outline" size="sm">Batal</Button>
    <Button type="button" variant="destructive" size="sm" onclick={clear}>Hapus</Button>
  </form>
</dialog>

<Advanced>
  <p class="text-muted-foreground">Aplikasi yang tidak pernah dibaca, di mode apa pun:</p>
  <ul class="mt-2 flex flex-wrap gap-1.5">
    {#each state.settings.blockedApps as app (app)}
      <li><code class="rounded bg-muted px-1.5 py-0.5 font-mono text-[13px]">{app}</code></li>
    {/each}
  </ul>
</Advanced>
