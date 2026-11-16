<script lang="ts">
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';

  import ModeSwitch from '$lib/components/mode-switch.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { DEFAULT_SHORTCUT } from '$lib/shortcut';
  import { cn } from '$lib/utils';

  import { ShortcutRecorder } from './recorder.svelte';
  import type { SettingsState } from './settings.svelte';

  let { state }: { state: SettingsState } = $props();

  const recorder = new ShortcutRecorder();

  const suggest = $derived(state.settings.suggest);
  /** Mode yang belum memberi saran, misalnya "Medical"; diambil dari mode pack, bukan ditulis tetap. */
  const silentModes = $derived(state.status.modes.filter((mode) => !mode.suggests).map((mode) => mode.label));

  const keys = $derived((recorder.held ?? state.settings.hotkey).split('+'));
  const notice = $derived.by(() => {
    if (recorder.recording) {
      return recorder.invalid
        ? { warn: true, text: 'Pakai Ctrl atau Alt bersama huruf, angka, F1–F24, atau Spasi. Esc untuk batal.' }
        : { warn: false, text: 'Tekan kombinasi baru sekarang. Esc untuk batal.' };
    }
    if (state.hotkeyError) {
      return { warn: true, text: state.hotkeyError };
    }
    if (state.settings.hotkeyStatus === 'taken') {
      return { warn: true, text: 'Pintasan ini sedang dipakai aplikasi lain. Pilih kombinasi lain lewat Ubah.' };
    }
    if (state.settings.hotkeyStatus === 'failed') {
      return { warn: true, text: 'Pintasan ini belum bisa dipasang. Pilih kombinasi lain lewat Ubah.' };
    }
    return { warn: false, text: 'Pilih teks di aplikasi apa pun, lalu tekan pintasan ini.' };
  });

  function startRecording() {
    state.hotkeyError = null;
    recorder.start();
  }

  function onKeydown(event: KeyboardEvent) {
    const shortcut = recorder.keydown(event);
    if (shortcut !== null) {
      void state.setHotkey(shortcut);
    }
  }
</script>

<!-- Perekaman berhenti bila jendela kehilangan fokus, agar tidak ada tombol yang tertangkap diam-diam. -->
<svelte:window onkeydown={onKeydown} onkeyup={(event) => recorder.keyup(event)} onblur={() => recorder.stop()} />

<Card.Root class="gap-0 [--card-spacing:--spacing(5)]">
  <Card.Content class="gap-0">
    <h2 class="font-semibold">Mode</h2>
    <p class="mt-1 text-[13px] text-muted-foreground">Umum untuk pekerjaan sehari-hari, Medical untuk aplikasi klinis.</p>
    <ModeSwitch
      class="mt-3 w-72"
      modes={state.status.modes}
      active={state.status.activeMode}
      onselect={(id) => state.setMode(id)}
    />
    <p class="mt-3 text-[13px] text-muted-foreground">
      {#if state.status.appName}
        Pilihan ini berlaku untuk {state.status.appName} dan diingat setiap kali Anda membukanya.
      {:else}
        Mode berganti otomatis sesuai aplikasi yang Anda buka.
      {/if}
    </p>
  </Card.Content>
</Card.Root>

<Card.Root class="gap-0 py-0">
  <h2 class="border-b px-5 py-3.5 font-semibold">Saran saat mengetik</h2>
  <div class="flex items-center justify-between gap-4 px-5 py-3.5">
    <div class="min-w-0">
      <p id="suggest-label" class="font-medium">Beri saran saat saya berhenti mengetik</p>
      <p id="suggest-note" class="text-[13px] text-muted-foreground">
        Saran kecil muncul di pojok kanan bawah tanpa mengganggu ketikan.
        {#if silentModes.length > 0}
          Mode {silentModes.join(' dan ')} belum memberi saran.
        {/if}
      </p>
    </div>
    <button
      type="button"
      role="switch"
      aria-checked={suggest.enabled}
      aria-labelledby="suggest-label"
      aria-describedby="suggest-note"
      class={cn(
        'inline-flex h-5 w-9 shrink-0 items-center rounded-full border-2 border-transparent shadow-xs transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50 forced-colors:border-[ButtonText]',
        suggest.enabled ? 'bg-primary' : 'bg-input',
      )}
      onclick={() => state.setSuggest(!suggest.enabled, suggest.perHour)}
    >
      <span
        class={cn(
          'pointer-events-none block size-4 rounded-full bg-background shadow-sm transition-transform',
          suggest.enabled ? 'translate-x-4' : 'translate-x-0',
        )}
        aria-hidden="true"
      ></span>
    </button>
  </div>
  {#if suggest.enabled}
    <div class="flex items-center justify-between gap-4 border-t px-5 py-3.5">
      <div class="min-w-0">
        <label for="suggest-limit" class="font-medium">Paling banyak</label>
        <p class="text-[13px] text-muted-foreground">Saran yang sering Anda abaikan makin jarang muncul.</p>
      </div>
      <div class="relative shrink-0">
        <select
          id="suggest-limit"
          class="h-8 appearance-none rounded-md border bg-background pr-8 pl-3 text-[13px] shadow-xs outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
          value={suggest.perHour}
          onchange={(event) => state.setSuggest(true, Number(event.currentTarget.value))}
        >
          {#each suggest.limits as limit (limit)}
            <option value={limit}>{limit} saran per jam</option>
          {/each}
        </select>
        <ChevronsUpDown
          class="pointer-events-none absolute top-1/2 right-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
          aria-hidden="true"
        />
      </div>
    </div>
  {/if}
</Card.Root>

<Card.Root class="gap-0 py-0">
  <h2 class="border-b px-5 py-3.5 font-semibold">Pintasan keyboard</h2>
  <div class="flex items-center justify-between gap-4 px-5 py-3.5">
    <div class="min-w-0">
      <p class="font-medium">Tanya Nuenly</p>
      <p class={cn('text-[13px]', notice.warn ? 'text-amber-700' : 'text-muted-foreground')} aria-live="polite">
        {notice.text}
      </p>
      {#if !recorder.recording && state.settings.hotkey !== DEFAULT_SHORTCUT}
        <button
          type="button"
          class="mt-1 rounded-sm text-[13px] text-muted-foreground underline underline-offset-2 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
          disabled={state.savingHotkey}
          onclick={() => state.setHotkey(null)}
        >
          Kembalikan ke {DEFAULT_SHORTCUT}
        </button>
      {/if}
    </div>
    <div class="flex shrink-0 items-center gap-3">
      {#if recorder.recording && recorder.held === null}
        <span class="rounded border border-dashed px-2 py-px text-xs text-muted-foreground">Tekan kombinasi…</span>
      {:else}
        <span class="flex items-center gap-1 text-muted-foreground" aria-label="Pintasan: {keys.join(' ')}">
          {#each keys as key, index (index)}
            {#if index > 0}+{/if}
            <kbd class="rounded border bg-muted px-1.5 py-px font-sans text-xs font-medium text-foreground">{key}</kbd>
          {/each}
        </span>
      {/if}
      {#if recorder.recording}
        <Button variant="outline" size="sm" onclick={() => recorder.stop()}>Batal</Button>
      {:else}
        <Button variant="outline" size="sm" disabled={state.savingHotkey} onclick={startRecording}>Ubah</Button>
      {/if}
    </div>
  </div>
</Card.Root>
