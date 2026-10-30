<script lang="ts">
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import Lock from '@lucide/svelte/icons/lock';

  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { cn } from '$lib/utils';

  import Advanced from './Advanced.svelte';
  import type { ModelState, SettingsState } from './settings.svelte';

  let { state }: { state: SettingsState } = $props();

  const STATUS: Record<ModelState, string> = {
    checking: 'Memeriksa…',
    ready: 'Siap',
    'no-models': 'Belum ada model',
    'model-missing': 'Model tidak ditemukan',
    'not-running': 'Ollama belum berjalan',
    'not-installed': 'Ollama belum terpasang',
  };

  const OTHER = [
    { title: 'Server kantor', description: 'Model di jaringan internal Anda' },
    { title: 'Claude API', description: 'Pakai kunci API sendiri · tidak dipakai di Medical' },
  ];

  const ready = $derived(state.modelState === 'ready');
  const waiting = $derived(state.modelState === 'checking');

  async function onModelChange(event: Event & { currentTarget: HTMLSelectElement }) {
    const select = event.currentTarget;
    await state.chooseModel(select.value === '' ? null : select.value);
    // Bila gagal disimpan, kembalikan tampilan ke pilihan yang tersimpan.
    select.value = state.settings.model ?? '';
  }
</script>

<Card.Root class="gap-0 [--card-spacing:--spacing(5)]">
  <Card.Content class="gap-0">
    <div class="flex h-5 items-center justify-between gap-4">
      <h2 id="model-title" class="font-semibold">Sedang dipakai</h2>
      <span
        role="status"
        class={cn(
          'flex items-center gap-1.5',
          ready ? 'text-green-700' : waiting ? 'text-muted-foreground' : 'text-amber-700',
        )}
      >
        <span
          class={cn('size-1.5 rounded-full', ready ? 'bg-green-600' : waiting ? 'bg-muted-foreground' : 'bg-amber-500')}
          aria-hidden="true"
        ></span>
        {STATUS[state.modelState]}
      </span>
    </div>

    <div class="relative mt-3">
      <!-- Teks pilihan digambar ulang di atas select agar keterangan "di PC ini" bisa diredupkan. -->
      <select
        aria-labelledby="model-title"
        class="peer h-10 w-full appearance-none rounded-lg border bg-background pr-10 pl-3 text-transparent shadow-xs outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 forced-colors:text-[CanvasText]"
        value={state.settings.model ?? ''}
        onchange={onModelChange}
      >
        <option class="text-foreground" value="">Otomatis · sesuai kemampuan PC ini</option>
        {#each state.modelOptions as name (name)}
          <option class="text-foreground" value={name}>{name} · di PC ini</option>
        {/each}
      </select>
      <span
        class="pointer-events-none absolute inset-y-0 right-10 left-3 flex items-center truncate forced-colors:hidden"
        aria-hidden="true"
      >
        {state.settings.model ?? 'Otomatis'}
        <span class="ml-1 text-muted-foreground">
          · {state.settings.model === null ? 'sesuai kemampuan PC ini' : 'di PC ini'}
        </span>
      </span>
      <ChevronsUpDown
        class="pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2 text-muted-foreground"
        aria-hidden="true"
      />
    </div>

    <p class="mt-3.5 flex items-center gap-2 text-[13px] text-muted-foreground">
      <Lock class="size-3.5 shrink-0" aria-hidden="true" />
      Berjalan lewat Ollama di PC ini. Tidak ada data yang keluar.
    </p>

    {#if !ready && !waiting}
      <div class="mt-4 flex items-center justify-between gap-4 rounded-lg bg-muted px-3 py-2.5">
        <p>
          {#if state.modelState === 'not-installed'}
            Pasang Ollama agar Nuenly bisa menjawab.
          {:else if state.modelState === 'not-running'}
            Buka Ollama dari menu Start, lalu periksa lagi.
          {:else if state.modelState === 'no-models'}
            Belum ada model di Ollama. Caranya ada di Pengaturan lanjutan.
          {:else}
            Model ini tidak ada di Ollama. Pilih model lain.
          {/if}
        </p>
        {#if state.modelState === 'not-installed'}
          <Button variant="outline" size="sm" class="bg-background" onclick={() => state.openDownload()}>
            Buka halaman unduhan <ExternalLink aria-hidden="true" />
          </Button>
        {:else}
          <Button
            variant="outline"
            size="sm"
            class="bg-background"
            disabled={state.checking}
            onclick={() => state.checkOllama()}
          >
            Periksa lagi
          </Button>
        {/if}
      </div>
    {/if}
  </Card.Content>
</Card.Root>

<Card.Root class="gap-0 py-0">
  <h2 class="border-b px-5 py-3.5 font-semibold">Pilihan lain</h2>
  <ul class="divide-y">
    {#each OTHER as option (option.title)}
      <li class="flex items-center justify-between gap-4 px-5 py-3.5">
        <div>
          <p class="font-medium">{option.title}</p>
          <p class="text-[13px] text-muted-foreground">{option.description}</p>
        </div>
        <Badge variant="secondary">Segera hadir</Badge>
      </li>
    {/each}
  </ul>
</Card.Root>

<Advanced>
  <dl class="grid grid-cols-[max-content_1fr] gap-x-6 gap-y-2">
    <dt class="text-muted-foreground">Alamat Ollama</dt>
    <dd class="font-mono text-[13px]">{state.settings.ollamaAddress}</dd>
    <dt class="text-muted-foreground">Versi Ollama</dt>
    <dd>{state.ollama?.version ?? 'Tidak terdeteksi'}</dd>
    <dt class="text-muted-foreground">Unduh model</dt>
    <dd>
      Jalankan <code class="rounded bg-muted px-1.5 py-0.5 font-mono text-[13px]">ollama pull &lt;nama-model&gt;</code>
      di terminal.
    </dd>
  </dl>
</Advanced>
