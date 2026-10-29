/**
 * Tinggi setiap toast yang sedang terpasang, urut dari yang paling lama. Hanya satu toast
 * terlihat (yang terbaru), jadi jendela mengikuti tinggi toast terakhir.
 */
class ToastLayout {
  #entries = $state<{ id: string; height: number }[]>([]);

  get frontHeight(): number {
    return this.#entries.at(-1)?.height ?? 0;
  }

  /**
   * Mencatat tinggi toast. Tinggi 0 (belum diukur) diabaikan: bila dicatat, jendela akan
   * disembunyikan sebelum toast sempat diukur, dan WebView di jendela tersembunyi berhenti
   * merender sehingga ukurannya tidak pernah datang.
   */
  set(id: string, height: number): void {
    if (!(height > 0)) {
      return;
    }
    const entry = this.#entries.find((item) => item.id === id);
    if (entry) {
      entry.height = height;
    } else {
      this.#entries.push({ id, height });
    }
  }

  delete(id: string): void {
    this.#entries = this.#entries.filter((item) => item.id !== id);
  }
}

export const toastLayout = new ToastLayout();
