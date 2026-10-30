<script lang="ts">
  import ModeSwitch from '$lib/components/mode-switch.svelte';
  import * as Card from '$lib/components/ui/card';

  import type { SettingsState } from './settings.svelte';

  let { state }: { state: SettingsState } = $props();

  const keys = $derived(state.settings.hotkey.split('+'));
</script>

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
  </Card.Content>
</Card.Root>

<Card.Root class="gap-0 py-0">
  <h2 class="border-b px-5 py-3.5 font-semibold">Pintasan keyboard</h2>
  <div class="flex items-center justify-between gap-4 px-5 py-3.5">
    <div>
      <p class="font-medium">Tanya Nuenly</p>
      {#if state.settings.hotkeyStatus === 'active'}
        <p class="text-[13px] text-muted-foreground">Pilih teks di aplikasi apa pun, lalu tekan pintasan ini.</p>
      {:else}
        <p class="text-[13px] text-amber-700">
          {state.settings.hotkeyStatus === 'taken'
            ? 'Pintasan ini sedang dipakai aplikasi lain.'
            : 'Pintasan ini belum bisa dipasang.'}
          Buka Tanya Nuenly lewat klik kanan ikon Nuenly di tray.
        </p>
      {/if}
    </div>
    <span class="flex shrink-0 items-center gap-1 text-muted-foreground">
      {#each keys as key, index (key)}
        {#if index > 0}+{/if}
        <kbd class="rounded border bg-muted px-1.5 py-px font-sans text-xs font-medium text-foreground">{key}</kbd>
      {/each}
    </span>
  </div>
</Card.Root>
