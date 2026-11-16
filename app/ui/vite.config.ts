import { resolve } from 'node:path';

import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

// Satu entry HTML per jendela Tauri; label jendela sama dengan nama entry.
const windows = ['ask', 'toast', 'tray', 'onboarding', 'settings'];

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  resolve: {
    alias: { $lib: resolve(import.meta.dirname, 'src/lib') },
  },
  clearScreen: false,
  // Server dev hanya untuk `cargo tauri dev`; build rilis memuat aset lewat protokol Tauri.
  server: { host: '127.0.0.1', port: 1420, strictPort: true },
  build: {
    target: 'chrome120',
    rolldownOptions: {
      input: Object.fromEntries(
        windows.map((name) => [name, resolve(import.meta.dirname, `${name}.html`)]),
      ),
    },
  },
  test: {
    include: ['src/**/*.test.ts'],
  },
});
