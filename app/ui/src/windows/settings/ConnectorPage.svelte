<script lang="ts">
  import Plug from '@lucide/svelte/icons/plug';

  import * as Card from '$lib/components/ui/card';
</script>

<Card.Root class="items-center gap-0 px-6 py-10 text-center">
  <Plug class="size-6 text-muted-foreground" aria-hidden="true" />
  <h2 class="mt-3 font-semibold">Belum ada connector</h2>
  <p class="mt-1 max-w-sm text-muted-foreground">
    Connector menghubungkan Nuenly ke layanan lain, misalnya kalender atau sistem rekam medis. Fitur ini hadir di
    versi berikutnya.
  </p>
</Card.Root>
