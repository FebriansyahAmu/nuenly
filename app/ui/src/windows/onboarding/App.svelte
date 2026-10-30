<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Laptop from '@lucide/svelte/icons/laptop';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';
  import Lock from '@lucide/svelte/icons/lock';
  import SlidersHorizontal from '@lucide/svelte/icons/sliders-horizontal';
  import TextCursorInput from '@lucide/svelte/icons/text-cursor-input';
  import X from '@lucide/svelte/icons/x';
  import { onMount } from 'svelte';

  import NuenlyMark from '$lib/components/nuenly-mark.svelte';
  import { Button } from '$lib/components/ui/button';
  import { cn } from '$lib/utils';

  import { type OnboardingFlow, STEPS, type Step } from './flow.svelte';

  let { flow }: { flow: OnboardingFlow } = $props();

  const HEADINGS: Record<Step, { title: string; subtitle: string }> = {
    welcome: {
      title: 'Selamat datang di Nuenly',
      subtitle: 'Asisten AI untuk menulis dan memahami teks di aplikasi apa pun.',
    },
    privacy: {
      title: 'Nuenly hanya membaca yang perlu',
      subtitle: 'Anda bisa menjeda atau mengubahnya kapan saja.',
    },
    model: {
      title: 'Siapkan model AI',
      subtitle: 'Nuenly memakai Ollama untuk menjalankan model AI di PC ini.',
    },
  };

  const READS = ['Membaca field yang sedang Anda isi', 'Membaca teks yang Anda pilih saat memanggil Nuenly'];
  const NEVER = ['Tidak merekam layar atau ketikan', 'Tidak membaca password, perbankan, atau jendela privat'];

  const heading = $derived(HEADINGS[flow.step]);
  let title = $state<HTMLHeadingElement | null>(null);

  onMount(() => {
    if (flow.step === 'model' && flow.ollama === null) {
      void flow.checkOllama();
    }
  });

  // Setiap ganti langkah, fokus pindah ke judul agar pembaca layar membacakan langkah baru.
  $effect(() => {
    void flow.step;
    title?.focus();
  });
</script>

{#snippet kbd(key: string)}
  <kbd class="rounded border bg-muted px-1.5 py-px font-sans text-xs font-medium">{key}</kbd>
{/snippet}

{#snippet stepNumber(number: number)}
  <span
    class="flex size-5 shrink-0 items-center justify-center rounded-full border text-xs font-medium text-muted-foreground"
  >
    {number}
  </span>
{/snippet}

<main class="flex min-h-screen items-center justify-center bg-muted/50 p-6 text-foreground">
  <section class="w-[520px] rounded-xl border bg-card p-8 text-sm text-card-foreground shadow-sm" aria-labelledby="judul">
    <div class="flex h-4 items-center gap-2.5">
      <div class="flex gap-1.5" aria-hidden="true">
        {#each STEPS as step, index (step)}
          <span class={cn('h-1 w-6 rounded-full', index <= flow.index ? 'bg-foreground' : 'bg-border')}></span>
        {/each}
      </div>
      <p class="text-xs text-muted-foreground">
        <span class="sr-only">Langkah</span>
        {flow.index + 1} dari {STEPS.length}
      </p>
    </div>

    {#if flow.step === 'welcome'}
      <NuenlyMark class="mt-6 size-10" />
    {/if}
    <h1
      id="judul"
      bind:this={title}
      tabindex="-1"
      class={cn(
        'text-[22px] leading-tight font-semibold tracking-[-0.01em] outline-none',
        flow.step === 'welcome' ? 'mt-4' : 'mt-6',
      )}
    >
      {heading.title}
    </h1>
    <p class="mt-1 text-muted-foreground">{heading.subtitle}</p>

    {#if flow.step === 'welcome'}
      <ul class="mt-6 space-y-3">
        <li class="flex items-center gap-2">
          <TextCursorInput class="size-[18px] shrink-0 text-muted-foreground" aria-hidden="true" />
          <span>Pilih teks, lalu tekan {@render kbd('Ctrl')} + {@render kbd('Space')} untuk bertanya</span>
        </li>
        <li class="flex items-center gap-2">
          <SlidersHorizontal class="size-[18px] shrink-0 text-muted-foreground" aria-hidden="true" />
          Ganti mode atau jeda dari ikon Nuenly di tray
        </li>
        <li class="flex items-center gap-2">
          <Laptop class="size-[18px] shrink-0 text-muted-foreground" aria-hidden="true" />
          Model AI berjalan di PC ini lewat Ollama
        </li>
      </ul>
    {:else if flow.step === 'privacy'}
      <ul class="mt-6 space-y-3">
        {#each READS as item (item)}
          <li class="flex items-center gap-2">
            <Check class="size-[18px] shrink-0 text-green-600" aria-hidden="true" />
            {item}
          </li>
        {/each}
        {#each NEVER as item (item)}
          <li class="flex items-center gap-2">
            <X class="size-[18px] shrink-0 text-red-600" aria-hidden="true" />
            {item}
          </li>
        {/each}
      </ul>
      <p class="mt-6 flex items-center gap-2.5 rounded-lg bg-muted px-3 py-3">
        <Lock class="size-[18px] shrink-0" aria-hidden="true" />
        Semua diproses di PC ini. Tidak ada yang dikirim ke internet.
      </p>
    {:else}
      <p role="status" class="mt-6 flex items-center gap-2.5 rounded-lg bg-muted px-3 py-3">
        {#if flow.ollama === 'running'}
          <CircleCheck class="size-[18px] shrink-0 text-green-600" aria-hidden="true" />
          Ollama sudah berjalan di PC ini.
        {:else if flow.ollama === 'installed'}
          <CircleAlert class="size-[18px] shrink-0 text-amber-600" aria-hidden="true" />
          Ollama sudah terpasang, tetapi belum berjalan.
        {:else if flow.ollama === 'missing'}
          <CircleAlert class="size-[18px] shrink-0 text-amber-600" aria-hidden="true" />
          Ollama belum terpasang di PC ini.
        {:else}
          <LoaderCircle class="size-[18px] shrink-0 animate-spin text-muted-foreground" aria-hidden="true" />
          Memeriksa Ollama…
        {/if}
      </p>

      {#if flow.ollama === 'running'}
        <p class="mt-4 text-muted-foreground">Pilihan model ada di Pengaturan › Model AI.</p>
      {:else if flow.ollama === 'installed'}
        <ol class="mt-4 space-y-3">
          <li class="flex items-center gap-2.5">
            {@render stepNumber(1)}
            Buka Ollama dari menu Start.
          </li>
          <li class="flex items-center gap-2.5">
            {@render stepNumber(2)}
            Tunggu ikon Ollama muncul di tray, lalu klik Periksa lagi.
          </li>
        </ol>
      {:else if flow.ollama === 'missing'}
        <ol class="mt-4 space-y-3">
          <li class="flex items-center gap-2.5">
            {@render stepNumber(1)}
            <span class="flex-1">Unduh Ollama untuk Windows.</span>
            <Button variant="outline" size="sm" onclick={() => flow.openDownload()}>
              Buka halaman unduhan <ExternalLink aria-hidden="true" />
            </Button>
          </li>
          <li class="flex items-center gap-2.5">
            {@render stepNumber(2)}
            Jalankan file instalasinya sampai selesai.
          </li>
          <li class="flex items-center gap-2.5">
            {@render stepNumber(3)}
            Kembali ke sini, lalu klik Periksa lagi.
          </li>
        </ol>
      {/if}
    {/if}

    {#if flow.error}
      <p role="alert" class="mt-4 text-destructive">{flow.error}</p>
    {/if}

    <div class="mt-6 flex items-center gap-2">
      {#if flow.step !== 'welcome'}
        <Button variant="ghost" class="px-3 text-foreground/70" onclick={() => flow.back()}>Kembali</Button>
      {/if}
      <div class="ml-auto flex gap-2">
        {#if flow.step === 'welcome'}
          <Button class="px-4" onclick={() => flow.next()}>Mulai</Button>
        {:else if flow.step === 'privacy'}
          <Button class="px-4" disabled={flow.accepting} onclick={() => flow.accept()}>Saya mengerti</Button>
        {:else if flow.ollama === 'running'}
          <Button class="px-4" onclick={() => flow.finish()}>Selesai</Button>
        {:else}
          <Button variant="ghost" class="px-3 text-foreground/70" onclick={() => flow.finish()}>Lewati dulu</Button>
          <Button class="px-4" disabled={flow.ollama === 'checking'} onclick={() => flow.checkOllama()}>
            Periksa lagi
          </Button>
        {/if}
      </div>
    </div>
  </section>
</main>
