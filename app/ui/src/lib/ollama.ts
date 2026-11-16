/**
 * Bentuknya sama dengan `OllamaStatus` di `app/src/ollama.rs`. `exposed`: Ollama berjalan, tetapi
 * juga bisa dihubungi dari jaringan, jadi Nuenly tidak memakainya.
 */
export type OllamaStatus = 'running' | 'exposed' | 'installed' | 'missing';

/** Kalimat pembuka yang sama di semua jendela saat Ollama terbuka ke jaringan. */
export const OLLAMA_EXPOSED = 'Ollama bisa dihubungi komputer lain di jaringan, jadi Nuenly tidak memakainya.';

/** Tempat model berjalan, dari yang tercepat; sama dengan `Placement` di `nuenly-context`. */
export type Placement = 'gpu' | 'split' | 'cpu' | 'tooLarge';

/** Bentuknya sama dengan `PlanView` di `app/src/ollama.rs`. */
export interface PlanView {
  model: string;
  /** Dipilih otomatis karena pengguna memilih "Otomatis". */
  automatic: boolean;
  placement: Placement;
  numCtx: number;
  keepAliveSecs: number;
  kvQ8Recommended: boolean;
}

/** Bentuknya sama dengan `HardwareInfo` di `app/src/hardware.rs`. */
export interface HardwareInfo {
  ramBytes: number;
  /** VRAM GPU terbesar; 0 bila tidak ada GPU. */
  vramBytes: number;
  gpu: string | null;
}

/** Bentuknya sama dengan `OllamaInfo` di `app/src/ollama.rs`. */
export interface OllamaInfo {
  status: OllamaStatus;
  version: string | null;
  /** Model yang berjalan di PC ini, urut abjad; model cloud tidak termasuk. */
  models: string[];
  /** Rencana untuk model yang dipakai; `null` bila belum ada model yang cocok. */
  plan: PlanView | null;
  hardware: HardwareInfo;
}
