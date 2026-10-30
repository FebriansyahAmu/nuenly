<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Lock from '@lucide/svelte/icons/lock';
  import X from '@lucide/svelte/icons/x';

  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { LOCAL_ONLY, NEVER, READS } from '$lib/privacy';

  import Advanced from './Advanced.svelte';
  import type { SettingsState } from './settings.svelte';

  let { state }: { state: SettingsState } = $props();

  const timeFormat = new Intl.DateTimeFormat('id-ID', { hour: '2-digit', minute: '2-digit' });
  const pausedUntil = $derived(state.status.pausedUntilMs);
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

<Advanced>
  <p class="text-muted-foreground">Aplikasi yang tidak pernah dibaca, di mode apa pun:</p>
  <ul class="mt-2 flex flex-wrap gap-1.5">
    {#each state.settings.blockedApps as app (app)}
      <li><code class="rounded bg-muted px-1.5 py-0.5 font-mono text-[13px]">{app}</code></li>
    {/each}
  </ul>
</Advanced>
