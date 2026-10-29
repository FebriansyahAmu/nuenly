<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import X from '@lucide/svelte/icons/x';
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount, tick } from 'svelte';

  import NuenlyMark from '$lib/components/nuenly-mark.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';

  import type { AskContext } from './backend';
  import type { AskSession } from './session.svelte';

  let { session }: { session: AskSession } = $props();

  const SOURCE_LABEL: Record<AskContext['source'], string> = {
    selection: 'Teks terpilih',
    field: 'Isi field',
  };

  let input = $state<HTMLInputElement>();
  let copied = $state(false);
  // Teks jawaban tetap teks biasa (tanpa HTML); baris kosong memisahkan paragraf.
  const paragraphs = $derived(session.answer.split(/\n\s*\n/));

  async function close() {
    if (isTauri()) {
      await getCurrentWindow().hide();
    }
  }

  async function copy() {
    // Lewat Rust agar jawaban tidak ikut ke riwayat clipboard dan clipboard cloud.
    if (isTauri()) {
      await invoke('clipboard_write_text', { text: session.answer });
    } else {
      await navigator.clipboard.writeText(session.answer);
    }
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      void close();
    }
  }

  function onSubmit(event: SubmitEvent) {
    event.preventDefault();
    void session.submit();
  }

  onMount(() => {
    input?.focus();
    if (!isTauri()) {
      return;
    }
    // Rust mengirim `ask:open` setiap kali jendela dibuka; sesi lama dibuang.
    const stop = listen<{ context?: AskContext | null } | null>('ask:open', async ({ payload }) => {
      session.open(payload?.context ?? null);
      copied = false;
      await tick();
      input?.focus();
    });
    return () => void stop.then((unlisten) => unlisten());
  });
</script>

<svelte:window onkeydown={onKeydown} />

<!-- Kartu di sepertiga atas layar (desain 03) agar input tidak bergeser saat jawaban memanjang. -->
<div class="fixed inset-0 flex items-start justify-center p-4 pt-[15vh]">
  <!-- Latar redup menutupi layar kerja; klik di luar kartu sama dengan Esc. -->
  <button
    type="button"
    class="absolute inset-0 cursor-default bg-black/40"
    tabindex="-1"
    aria-label="Tutup"
    onclick={close}
  ></button>

  <div
    role="dialog"
    aria-modal="true"
    aria-labelledby="ask-title"
    class="relative w-full max-w-[620px] overflow-hidden rounded-xl border bg-background shadow-2xl"
  >
    <h1 id="ask-title" class="sr-only">Tanya Nuenly</h1>

    <form class="flex items-center gap-3 border-b px-4 py-3.5" onsubmit={onSubmit}>
      <NuenlyMark class="shrink-0" />
      <input
        bind:this={input}
        bind:value={session.prompt}
        class="min-w-0 flex-1 bg-transparent text-base outline-none placeholder:text-muted-foreground"
        placeholder="Minta Nuenly merapikan, meringkas, atau menjelaskan…"
        aria-label="Perintah untuk Nuenly"
        autocomplete="off"
        spellcheck="false"
      />
    </form>

    <div class="flex flex-col gap-3 p-4">
      {#if session.context}
        <div class="flex items-center gap-2 text-sm">
          <span class="text-muted-foreground">Dari:</span>
          <Badge variant="secondary" class="h-6 gap-1.5 rounded-md px-2 text-sm font-normal">
            {SOURCE_LABEL[session.context.source]} di {session.context.appName}
            <button
              type="button"
              class="rounded-sm text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
              aria-label="Jangan pakai {SOURCE_LABEL[session.context.source].toLowerCase()} sebagai konteks"
              onclick={() => session.clearContext()}
            >
              <X class="size-3.5" />
            </button>
          </Badge>
        </div>
      {/if}

      {#if session.phase === 'idle'}
        <p class="text-sm text-muted-foreground">Tulis perintah, lalu tekan Enter.</p>
      {:else}
        <div
          class="flex max-h-[50vh] flex-col gap-2 overflow-y-auto rounded-lg border bg-muted/40 p-4 text-[15px] leading-normal"
          aria-live="polite"
          aria-busy={session.phase === 'streaming'}
        >
          {#if session.phase === 'error'}
            <p class="text-destructive">{session.error}</p>
          {:else}
            {#each paragraphs as paragraph, index (index)}
              <p class="whitespace-pre-wrap">
                {paragraph}{#if session.phase === 'streaming' && index === paragraphs.length - 1}<span
                    class="ml-0.5 inline-block h-4 w-1.5 animate-pulse rounded-sm bg-foreground/50 align-text-bottom"
                  ></span>{/if}
              </p>
            {/each}
          {/if}
        </div>
      {/if}

      <div class="flex items-center gap-2 pt-1">
        <!-- Aktif setelah Ganti teks (F-23) tersedia di Fase 2. -->
        <Button disabled>Ganti teks</Button>
        <Button variant="outline" disabled={!session.canCopy} onclick={copy}>
          {#if copied}
            <Check /> Tersalin
          {:else}
            Salin
          {/if}
        </Button>
        <Button variant="ghost" disabled={!session.canRetry} onclick={() => session.retry()}>
          Coba lagi
        </Button>
        <span class="ml-auto text-xs text-muted-foreground">Esc untuk tutup</span>
      </div>
    </div>
  </div>
</div>
