// Mode demo popup tray untuk melihat tampilan di browser saat `pnpm dev`. Hanya dimuat di build dev.

import type { StatusView } from '$lib/status';

import type { TrayBackend } from './backend';

const MODES = [
  { id: 'all', label: 'Umum', accent: '#4f46e5' },
  { id: 'medical', label: 'Medical', accent: '#0f766e' },
];

/** `?jeda` membuka dalam keadaan dijeda; bawaannya mode Medical seperti desain 04. */
export function demoTray(params: URLSearchParams): { initial: StatusView; backend: TrayBackend } {
  let status: StatusView = {
    modes: MODES,
    activeMode: 'medical',
    pausedUntilMs: params.has('jeda') ? Date.now() + 15 * 60 * 1000 : null,
  };
  const update = (change: Partial<StatusView>) => Promise.resolve((status = { ...status, ...change }));
  return {
    initial: status,
    backend: {
      setMode: (id) => update({ activeMode: id }),
      pause: () => update({ pausedUntilMs: Date.now() + 15 * 60 * 1000 }),
      resume: () => update({ pausedUntilMs: null }),
      openSettings: () => Promise.resolve(),
      quit: () => Promise.resolve(),
      close: () => Promise.resolve(),
    },
  };
}
