<script lang="ts">
  import { invoke, isTauri } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { Toaster, toast } from 'svelte-sonner';

  import { toastLayout } from './layout.svelte';
  import ToastView from './ToastView.svelte';
  import type { ToastAction, ToastPayload } from './types';

  let { initial = [] }: { initial?: ToastPayload[] } = $props();

  /** Jarak kartu ke tepi jendela; jendela = kartu + jarak ini di setiap sisi. */
  const MARGIN = 12;
  const COPIED: ToastPayload = {
    id: 'copied',
    durationMs: 3000,
    kind: 'success',
    message: 'Poin disalin. Tempel dengan Ctrl V.',
  };

  function present(payload: ToastPayload, expanded = false) {
    // Setelah "Lihat", toast tidak hilang sendiri karena pengguna sedang membacanya.
    const sticky = expanded || payload.durationMs === null;
    const close = (action: ToastAction) => {
      report(payload, action);
      toast.dismiss(payload.id);
    };
    toast.custom(ToastView, {
      id: payload.id,
      duration: sticky ? Number.POSITIVE_INFINITY : (payload.durationMs ?? undefined),
      onAutoClose: () => report(payload, 'expire'),
      componentProps: {
        payload,
        expanded,
        onExpand: () => {
          report(payload, 'view');
          present(payload, true);
        },
        onClose: () => close('close'),
        onDismiss: () => close('dismiss'),
        onDislike: () => close('dislike'),
        onCopyPoints: async (text: string) => {
          await copyText(text);
          close('copy');
          present(COPIED);
        },
      },
    });
  }

  /** Tombol saran dicatat Rust sebagai umpan balik tanpa teks; toast lain tidak dilaporkan. */
  function report(payload: ToastPayload, action: ToastAction) {
    if (payload.kind === 'suggestion' && isTauri()) {
      void invoke('toast_action', { id: payload.id, action });
    }
  }

  async function copyText(text: string) {
    // Jendela toast tidak pernah fokus, jadi clipboard ditulis oleh Rust.
    if (isTauri()) {
      await invoke('clipboard_write_text', { text });
    } else {
      await navigator.clipboard.writeText(text);
    }
  }

  // Jendela mengikuti tinggi toast terdepan; 0 berarti tidak ada toast dan jendela disembunyikan.
  $effect(() => {
    const height = toastLayout.frontHeight;
    if (isTauri()) {
      void invoke('toast_layout', { height: height > 0 ? height + MARGIN * 2 : 0 });
    }
  });

  onMount(() => {
    for (const payload of initial) {
      present(payload);
    }
    if (!isTauri()) {
      return;
    }
    const stop = listen<ToastPayload>('toast:show', ({ payload }) => present(payload));
    return () => void stop.then((unlisten) => unlisten());
  });
</script>

<Toaster position="bottom-right" visibleToasts={1} offset={MARGIN} hotkey={[]} />
