<script lang="ts">
  import { cn } from '$lib/utils';

  /** Logo Nuenly: huruf n di lingkaran gelap, dengan titik berwarna aksen mode aktif. */
  let { class: className }: { class?: string } = $props();
</script>

<svg class={cn('size-6 text-foreground', className)} viewBox="0 0 1024 1024" aria-hidden="true">
  <circle cx="480" cy="544" r="440" fill="currentColor" />
  <path
    d="M310 790 V420 M310 540 C310 430 390 370 485 370 C580 370 655 430 655 540 V790"
    fill="none"
    stroke="var(--background)"
    stroke-width="130"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
  <circle cx="850" cy="174" r="130" fill="var(--mode-accent)" stroke="var(--background)" stroke-width="40" />
</svg>
