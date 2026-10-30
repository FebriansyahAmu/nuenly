<script lang="ts">
  import { tick } from 'svelte';

  import { nextIndex } from '$lib/mode-switch';
  import type { ModeInfo } from '$lib/status';
  import { cn } from '$lib/utils';

  /** Pilihan mode berupa radiogroup: klik, atau panah/Home/End saat terfokus. */
  let {
    modes,
    active,
    onselect,
    class: className,
  }: {
    modes: ModeInfo[];
    active: string;
    onselect: (id: string) => Promise<void>;
    class?: string;
  } = $props();

  const activeIndex = $derived(modes.findIndex((mode) => mode.id === active));
  let radios = $state<HTMLButtonElement[]>([]);

  async function onkeydown(event: KeyboardEvent) {
    const index = nextIndex(activeIndex, event.key, modes.length);
    const mode = index === null ? undefined : modes[index];
    if (index === null || !mode) {
      return;
    }
    event.preventDefault();
    await onselect(mode.id);
    await tick();
    radios[index]?.focus();
  }
</script>

<div
  role="radiogroup"
  aria-label="Mode"
  class={cn('grid rounded-lg bg-muted p-[3px]', className)}
  style:grid-template-columns="repeat({modes.length}, minmax(0, 1fr))"
>
  {#each modes as mode, index (mode.id)}
    {@const checked = mode.id === active}
    <button
      bind:this={radios[index]}
      type="button"
      role="radio"
      aria-checked={checked}
      tabindex={checked ? 0 : -1}
      class={cn(
        'flex h-7 items-center justify-center gap-2 rounded-md text-[15px] transition-colors',
        'focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
        checked ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground',
      )}
      onclick={() => onselect(mode.id)}
      {onkeydown}
    >
      {#if checked}
        <span class="size-2 rounded-full" style:background-color={mode.accent} aria-hidden="true"></span>
      {/if}
      {mode.label}
    </button>
  {/each}
</div>
