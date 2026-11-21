import { invoke } from "@tauri-apps/api/core";
export interface TtsStatus {
  model_id: string; voices: string[]; dependencies: { id: string; shared: boolean; installed: boolean; bytes: number }[];
  provider: string; language: "en" | "ru"; model_file: string; download_bytes: number;
  supported: boolean; installed: boolean; accepted: boolean;
  model_bytes: number; runtime_bytes: number; disk_bytes: number; partial_bytes: number;
  license: string; license_hash: string; error: string | null;
  progress: { stage: string; downloaded: number; total: number } | null;
}
export const getTtsStatus = (provider = "silero") => invoke<TtsStatus>("tts_status", { provider });
export const prepareTts = (provider = "silero") => invoke<void>("prepare_tts", { provider });
export const installTts = (accepted: boolean, licenseHash: string, modelPath: string | null = null, provider = "silero") => invoke<void>("install_tts", { accepted, licenseHash, modelPath, provider });
export const cancelTts = () => invoke<void>("cancel_tts");
export const removeTts = (provider = "silero") => invoke<void>("remove_tts", { provider });
/** A sample of `voice`; with `text` (≤200 characters) that phrase, prepared like a document. */
export const previewTts = (voice: string, provider = "silero", text: string | null = null, style: string | null = null) => invoke<string>("preview_tts", { voice, provider, text, style });
