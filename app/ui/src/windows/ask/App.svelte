<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      void getCurrentWindow().hide();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main class="flex h-screen flex-col items-center justify-center gap-2 p-6">
  <h1 class="text-lg font-medium">Tanya Nuenly</h1>
  <p class="text-sm text-muted-foreground">Esc untuk tutup</p>
</main>
