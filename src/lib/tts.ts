import { invoke } from "@tauri-apps/api/core";
export interface TtsStatus {
  supported: boolean; installed: boolean; accepted: boolean;
  model_bytes: number; runtime_bytes: number; disk_bytes: number; partial_bytes: number;
  license: string; license_hash: string; error: string | null;
  progress: { stage: string; downloaded: number; total: number } | null;
}
export const getTtsStatus = () => invoke<TtsStatus>("tts_status");
export const installTts = (accepted: boolean, licenseHash: string, modelPath: string | null = null) => invoke<void>("install_tts", { accepted, licenseHash, modelPath });
export const cancelTts = () => invoke<void>("cancel_tts");
export const removeTts = () => invoke<void>("remove_tts");
export const previewTts = (voice: string) => invoke<string>("preview_tts", { voice });
