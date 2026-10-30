<script lang="ts">
  import type { Snippet } from 'svelte';

  /** "Pengaturan lanjutan": tempat satu-satunya untuk istilah dan opsi teknis. Tertutup bawaannya. */
  let { children }: { children: Snippet } = $props();
</script>

<details class="group rounded-xl bg-card text-sm text-card-foreground shadow-xs ring-1 ring-foreground/10">
  <summary
    class="flex cursor-pointer list-none items-center gap-2 rounded-xl px-5 py-3.5 font-medium outline-none select-none focus-visible:ring-3 focus-visible:ring-ring/50 [&::-webkit-details-marker]:hidden"
  >
    <svg viewBox="0 0 8 8" class="size-2 transition-transform group-open:rotate-90" aria-hidden="true">
      <path d="M1 0 7 4 1 8Z" fill="currentColor" />
    </svg>
    Pengaturan lanjutan
  </summary>
  <div class="px-5 pb-4">
    {@render children()}
  </div>
</details>
