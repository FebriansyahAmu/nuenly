// Mode demo onboarding untuk melihat tampilan di browser saat `pnpm dev`. Hanya dimuat di build dev.

import type { OllamaStatus, OnboardingBackend } from './backend';
import { STEPS, type Step } from './flow.svelte';

/**
 * `?langkah=1|2|3` membuka langkah tertentu (bawaannya 2, seperti desain 06);
 * `?ollama=running|installed|missing|checking` mengatur hasil pemeriksaan Ollama.
 */
export function demoOnboarding(params: URLSearchParams): { backend: OnboardingBackend; step: Step } {
  const step = STEPS[Number(params.get('langkah') ?? 2) - 1] ?? 'privacy';
  const ollama = params.get('ollama') ?? 'missing';
  const checkOllama = (): Promise<OllamaStatus> =>
    ollama === 'checking'
      ? new Promise(() => {})
      : Promise.resolve(ollama === 'running' || ollama === 'installed' ? ollama : 'missing');
  return {
    step,
    backend: {
      accept: () => Promise.resolve(),
      checkOllama,
      openDownload: () => Promise.resolve(),
      finish: () => Promise.resolve(),
    },
  };
}
