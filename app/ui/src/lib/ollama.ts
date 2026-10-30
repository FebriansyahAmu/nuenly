/** Bentuknya sama dengan `OllamaStatus` di `app/src/ollama.rs`. */
export type OllamaStatus = 'running' | 'installed' | 'missing';

/** Bentuknya sama dengan `OllamaInfo` di `app/src/ollama.rs`. */
export interface OllamaInfo {
  status: OllamaStatus;
  version: string | null;
  /** Model yang sudah diunduh, urut abjad. */
  models: string[];
}
