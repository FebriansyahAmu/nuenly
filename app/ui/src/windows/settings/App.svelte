<script lang="ts">
  import { onMount } from 'svelte';

  import { cn } from '$lib/utils';

  import ConnectorPage from './ConnectorPage.svelte';
  import GeneralPage from './GeneralPage.svelte';
  import ModelPage from './ModelPage.svelte';
  import PrivacyPage from './PrivacyPage.svelte';
  import { PAGES, type SettingsState } from './settings.svelte';

  let { state }: { state: SettingsState } = $props();

  const page = $derived(PAGES.find((candidate) => candidate.id === state.page) ?? PAGES[0]);

  onMount(() => {
    void state.checkOllama();
  });
</script>

<div class="flex h-screen bg-background text-sm text-foreground">
  <nav
    aria-label="Menu pengaturan"
    class="flex w-[200px] shrink-0 flex-col border-r bg-sidebar px-2 pt-4.5 text-sidebar-foreground"
  >
    <p class="px-2 text-base font-semibold">Pengaturan</p>
    <ul class="mt-4 flex flex-col gap-0.5">
      {#each PAGES as item (item.id)}
        {@const current = item.id === state.page}
        <li>
          <button
            type="button"
            aria-current={current ? 'page' : undefined}
            class={cn(
              'flex h-9 w-full items-center rounded-lg px-2.5 text-left transition-colors outline-none',
              'focus-visible:ring-2 focus-visible:ring-sidebar-ring',
              current ? 'bg-sidebar-accent text-sidebar-accent-foreground' : 'hover:bg-sidebar-accent/60',
            )}
            onclick={() => (state.page = item.id)}
          >
            {item.label}
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <main class="min-w-0 flex-1 overflow-y-auto px-12 pt-8 pb-12">
    <div class="max-w-[640px]">
      <h1 class="text-[22px] leading-tight font-semibold tracking-[-0.01em]">{page.label}</h1>
      <p class="mt-1 text-muted-foreground">{page.subtitle}</p>
      {#if state.error}
        <p role="alert" class="mt-3 text-destructive">{state.error}</p>
      {/if}

      <div class="mt-6 flex flex-col gap-6">
        {#if state.page === 'general'}
          <GeneralPage {state} />
        {:else if state.page === 'model'}
          <ModelPage {state} />
        {:else if state.page === 'privacy'}
          <PrivacyPage {state} />
        {:else}
          <ConnectorPage />
        {/if}
      </div>
    </div>
  </main>
</div>
