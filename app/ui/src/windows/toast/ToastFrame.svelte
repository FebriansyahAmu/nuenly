<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cn } from '$lib/utils';

  import { toastLayout } from './layout.svelte';

  let { id, class: className, children }: { id: string; class?: string; children: Snippet } = $props();

  let height = $state(0);

  // Tinggi kartu menentukan tinggi jendela toast (lihat App.svelte).
  $effect(() => {
    toastLayout.set(id, height);
  });
  $effect(() => {
    const current = id;
    return () => toastLayout.delete(current);
  });
</script>

<div
  bind:offsetHeight={height}
  class={cn('relative w-[376px] rounded-lg border bg-background p-4 text-foreground shadow-lg', className)}
>
  {@render children()}
</div>
