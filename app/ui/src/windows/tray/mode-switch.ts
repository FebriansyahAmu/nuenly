/**
 * Indeks pilihan berikutnya untuk tombol panah di grup radio mode (pola radiogroup WAI-ARIA):
 * kiri/atas mundur, kanan/bawah maju, berputar di ujung. `null` bila tombol tidak relevan.
 */
export function nextIndex(current: number, key: string, count: number): number | null {
  if (count === 0) {
    return null;
  }
  switch (key) {
    case 'ArrowRight':
    case 'ArrowDown':
      return (current + 1) % count;
    case 'ArrowLeft':
    case 'ArrowUp':
      return (current - 1 + count) % count;
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return null;
  }
}
